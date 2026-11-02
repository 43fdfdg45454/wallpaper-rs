#!/usr/bin/env bash
set -euo pipefail
declare log
gpu() { ctl gpu --json | python3 -c "import json, sys; g = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
used() { gpu "[a['name'] for a in g['adapters'] if a['used']][0]"; }
[ "$(gpu "len([a for a in g['adapters'] if a['used']])")" = 1 ] || fail "not one card in use: $(ctl gpu --json)"
[ "$(gpu "g['gpu'], g['warning'], g['restart']")" = "('auto', None, False)" ] || fail "the default choice: $(ctl gpu)"
first=$(used)
first_kind=$(gpu "[a['kind'] for a in g['adapters'] if a['used']][0]")
ctl gpu "no such card 42" >/dev/null
[ "$(gpu "g['gpu'], g['restart']")" = "('no such card 42', True)" ] || fail "a new choice is not kept: $(ctl gpu)"
ctl gpu | grep -q "next time the daemon starts" || fail "wprs-ctl gpu does not say when the choice takes effect"
ctl gpu auto >/dev/null
[ "$(gpu "g['restart']")" = False ] || fail "going back to auto: $(ctl gpu)"
if ctl config render.gpu "" 2>/dev/null; then fail "an empty card name was accepted"; fi
started() {
  kill "$daemon"
  wait "$daemon" 2>/dev/null || true
  printf '[render]\ngpu = "%s"\n' "$1" >"$WORK/gpu.toml"
  RUST_LOG=$log "$BIN/wprsd" --config "$WORK/gpu.toml" >"$WORK/gpu-$2.log" 2>&1 & daemon=$!; pids+=("$daemon")
  wait_for 30 ctl state || fail "the daemon did not start with render.gpu = $1"
  read -r _ _ _ serial < <(ctl stream TEST-G 160 90)
  ctl set "$ROOT/tests/fixtures/green-64x32.png" -o TEST-G --fit fit
  retry 10 sh -c "'$BIN/probe' $serial 3 | grep -q 'center bgra \[0, 255, 0, 255\]'" ||
    fail "no frame with render.gpu = $1: $("$BIN/probe" "$serial" 3 2>&1)"
}
started "no such card 42" missing
[ "$(used)" = "$first" ] || fail "a missing card did not fall back to the automatic choice: $(ctl gpu)"
gpu "g['warning']" | grep -q "no such card 42" || fail "no warning for a missing card: $(ctl gpu --json)"
[ "$(gpu "g['restart']")" = False ] || fail "a missing card asks for a restart"
started "$(tr '[:lower:]' '[:upper:]' <<<"${first:0:12}")" named
[ "$(used), $(gpu "g['warning']")" = "$first, None" ] || fail "the card chosen by name: $(ctl gpu)"
case $first_kind in
  integrated | dedicated | software)
    started "$first_kind" kind
    [ "$(gpu "[a['kind'] for a in g['adapters'] if a['used']][0]")" = "$first_kind" ] ||
      fail "the card chosen by kind $first_kind: $(ctl gpu)"
    ;;
esac
echo "graphics card choice checks passed ($first, $first_kind)"
