#!/usr/bin/env bash
set -euo pipefail
declare anchor kr port site
mkdir -p "$site/next"
cp "$site/src/image.png" "$site/next/image.png"
cp "$ROOT/tests/fixtures/bands-640x180.png" "$site/next/preview.png"
sed 's/"1.0.0"/"1.1.0"/' "$site/src/manifest.json" >"$site/next/manifest.json"
(cd "$site" && ctl pack build next -o dawn-next.wpak >/dev/null && ctl repo add repo dawn-next.wpak >/dev/null &&
  ctl repo publish repo --key online.key >/dev/null)
repo=$(ctl remote add "http://127.0.0.1:$port/$anchor" --wait --json | jq_ "r['output']['repo_id']")
fetched() { grep -c 'GET /previews/' "$WORK/http.log" || true; }
old=$(ctl pkg preview "${repo:0:8}/org.example.dawn@1.0.0")
new=$(ctl pkg preview "${repo:0:8}/org.example.dawn")
before=$(fetched)
if [ "$(ctl pkg preview "${repo:0:8}/org.example.dawn@1.0.0")" != "$old" ] || [ "$old" = "$new" ]; then
  fail "the preview of an older version was not cached apart"
fi
[ "$(fetched)" = "$before" ] || fail "the preview of an older version was downloaded again"
state=$XDG_DATA_HOME/wallpaper-rs/repos/$repo/state.json
rm "$state"
[ "$(ctl remote ls --json | jq_ "[r['broken'] is not None for r in r['repos']]")" = "[True]" ] ||
  fail "a repository without its state was not reported: $(ctl remote ls --json)"
ctl remote ls | grep -q "broken, refresh or remove it" || fail "remote ls hides the broken repository"
ctl pkg search sky >/dev/null || fail "a broken repository stopped browsing"
if ctl remote refresh "${repo:0:8}" --wait >/dev/null 2>&1; then fail "a repository without an address was refreshed"; fi
ctl remote add "http://127.0.0.1:$port/$anchor" --wait >/dev/null
[ "$(ctl remote ls --json | jq_ "[(r['broken'], r['last_error'], r['packages']) for r in r['repos']]")" = "[(None, None, 2)]" ] ||
  fail "adding a broken repository again did not repair it: $(ctl remote ls --json)"
ctl remote rm "${repo:0:8}"
if [ -e "$old" ] || [ -e "$new" ]; then fail "previews of every version go with their repository"; fi
echo "broken repository and preview cache checks passed"
dport=$(python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', 0)); print(s.getsockname()[1])")
cat >"$WORK/dav.py" <<'DAV'
import base64, http.server, sys
files = {"a.png": open(sys.argv[2], "rb").read(), "b.png": open(sys.argv[3], "rb").read(), "big.png": b"x" * (17 << 20)}
auth = "Basic " + base64.b64encode(b"me:hunter2").decode()
class Dav(http.server.BaseHTTPRequestHandler):
    def denied(self):
        if self.headers.get("Authorization") == auth:
            return False
        self.send_response(401)
        self.send_header("Content-Length", "0")
        self.end_headers()
        return True
    def do_PROPFIND(self):
        if self.denied():
            return
        rows = "".join(f"<d:response><d:href>/dav/{n}</d:href><d:propstat><d:prop><d:resourcetype/></d:prop></d:propstat></d:response>" for n in files)
        body = f'<?xml version="1.0"?><d:multistatus xmlns:d="DAV:"><d:response><d:href>/dav/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response>{rows}</d:multistatus>'.encode()
        self.send_response(207)
        self.send_header("Content-Type", "application/xml")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)
    def do_GET(self):
        name = self.path.rsplit("/", 1)[-1]
        if self.denied() or name not in files:
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(files[name])))
        self.end_headers()
        try:
            self.wfile.write(files[name])
        except OSError:
            print("cut", name, flush=True)
http.server.ThreadingHTTPServer(("127.0.0.1", int(sys.argv[1])), Dav).serve_forever()
DAV
python3 "$WORK/dav.py" "$dport" "$ROOT/tests/fixtures/green-64x32.png" "$ROOT/tests/fixtures/bands-640x180.png" >"$WORK/dav.log" 2>&1 & pids+=($!)
wait_for 10 python3 -c "import socket; socket.create_connection(('127.0.0.1', $dport))" || fail "the WebDAV server did not start"
unlock
dav() { printf 'hunter2\n' | ctl provider add dav webdav "url=http://127.0.0.1:$dport/dav/" user=me refresh=0s cache_mb=16 "$@"; }
dav --secret-stdin >/dev/null
report=$(ctl provider sync dav --wait --json | jq_ "[(p['listed'], p['downloaded'], p['skipped'], p['failed']) for p in r['output']['reports']]")
[ "$report" = "[(3, 2, 1, 0)]" ] || fail "the WebDAV sync reported $report"
wait_for 10 grep -q "cut big.png" "$WORK/dav.log" || fail "the file over the size limit was downloaded whole"
provided=$XDG_CACHE_HOME/wallpaper-rs/providers/dav
[ "$(find "$provided" -name '*.png' | wc -l)" = 2 ] || fail "provider files: $(ls -a "$provided")"
synced() { ctl provider ls --json | jq_ "[(p['last_sync'] is not None, p['orphan_secret']) for p in r['providers'] if p['id'] == 'dav']"; }
[ "$(synced)" = "[(True, False)]" ] || fail "the provider state after a sync: $(synced)"
ctl provider rm dav
[ ! -e "$provided/.credits.json" ] || fail "removing a provider kept its sync state"
dav --secret-stdin >/dev/null
[ "$(synced)" = "[(False, False)]" ] || fail "a provider added again kept the old sync time: $(synced)"
kill "$kr"
wait "$kr" 2>/dev/null || true
ctl provider rm dav
dav --no-secret >/dev/null
[ "$(synced)" = "[(False, True)]" ] || fail "the orphaned secret was not reported: $(synced)"
unlock
swept() { [ "$(synced)" = "[(False, False)]" ]; }
wait_for 30 swept || fail "the orphaned secret was kept after the keyring came back"
[ "$(secretly dav)" = "[('needs_secret', False)]" ] || fail "an orphaned secret reached the provider: $(secretly dav)"
ctl provider rm dav --purge
kill "$kr"
wait "$kr" 2>/dev/null || true
echo "online provider, size limit and orphaned secret checks passed"
