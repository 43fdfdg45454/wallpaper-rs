#!/usr/bin/env bash
set -euo pipefail
declare pkg URL
mkdir -p "$pkg/noon"
cp "$pkg/src/"*.png "$pkg/noon/"
sed 's/org.example.dusk/org.example.noon/; s/"Dusk"/"Noon"/' "$pkg/src/manifest.json" >"$pkg/noon/manifest.json"
a pack build "$pkg/noon" -o "$pkg/noon.wpak" >/dev/null
a server upload "$pkg/noon.wpak" --unlisted --wait --json | jq_ "r['output']['state']" | grep -qx pending_review ||
  fail "the unlisted upload did not wait for review"
case_id=$(review "$URL/api/v1/mod/queue" | jq_ "r['items'][0]['id']")
review -d '{"decision": "approve", "statement": "Looks fine to publish."}' "$URL/api/v1/mod/cases/$case_id/decision" >/dev/null
api -X POST "$URL/api/v1/admin/repo/publish" >/dev/null
a server packages | grep -q 'org.example.noon 1.0.0 published unlisted' || fail "own packages: $(a server packages)"
[ "$(curl -fsS "$URL/api/v1/catalog?q=noon" | jq_ "len(r['items'])")" = 0 ] || fail "the catalog lists an unlisted package"
a remote refresh --wait >/dev/null
[ "$(a pkg search noon --json | jq_ "len(r['packages'])")" = 0 ] || fail "a client browses an unlisted package"
a pkg install org.example.noon --wait >/dev/null || fail "the link of an unlisted package does not install"
a pkg uninstall org.example.noon
if a server visibility org.example.nope public 2>/dev/null; then fail "another package changed visibility"; fi
a server visibility org.example.noon public
api -X POST "$URL/api/v1/admin/repo/publish" >/dev/null
[ "$(curl -fsS "$URL/api/v1/catalog?q=noon" | jq_ "len(r['items'])")" = 1 ] || fail "a public package is not listed"
if a remote vote org.example.noon sideways 2>/dev/null; then fail "a vote other than up, down or none was sent"; fi
a remote vote org.example.noon up | grep -q '"up":1' || fail "the vote was not counted"
a remote vote org.example.noon down | grep -q '"down":1,.*"up":0' || fail "a changed vote was counted twice"
a remote votes org.example.noon | grep -q '"down":1,"mine":"down","up":0' || fail "counts: $(a remote votes org.example.noon)"
curl -fsS "$URL/api/v1/catalog?q=noon" | grep -q '"votes":{"down":1,"up":0}' || fail "the catalog has no counts"
server_repo=$(a remote votes org.example.noon | jq_ "r['repo_id']")
other=$(printf '%064d' 7)
a remote votes "$server_repo/org.example.noon" | jq_ "r['items']['$server_repo/org.example.noon']['down']" | grep -qx 1 ||
  fail "counts of the server's repository were not given by repository and id"
[ "$(a remote votes "$other/org.example.noon" | jq_ "(r['shown'], len(r['items']))")" = "(False, 0)" ] ||
  fail "ids of another repository were sent to the server"
if a remote vote "$other/org.example.noon" up 2>/dev/null; then fail "a vote for another repository was sent"; fi
mapfile -t rows < <(for i in $(seq 1 150); do echo "$other/org.example.row$i"; done)
[ "$(a remote votes "${rows[@]}" "$server_repo/org.example.noon" | jq_ "r['items']['$server_repo/org.example.noon']['down']")" = 1 ] ||
  fail "a package of the server after 150 rows of another repository got no counts"
a remote votes org.example.noon | grep -q '"down":1,"mine":"down","up":0' || fail "a refused vote changed the counts"
a config repos.vote_counts false
a remote votes org.example.noon | grep -q '"shown":false' || fail "hidden counts were fetched"
a config repos.vote_counts true
echo "unlisted upload and vote checks passed"
