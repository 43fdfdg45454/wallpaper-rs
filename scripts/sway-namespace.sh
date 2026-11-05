#!/usr/bin/env bash
set -euo pipefail
declare WORK BIN
key=surfaces.layer_namespace
expect "s['layer_shell']" "{'namespace': 'wprs-check', 'from': 'config', 'restart': False}"
announced=$(grep -o "new layer surface: namespace [^ ]* layer [0-9]*" "$WORK/sway.log" | sort -u || true)
if [ -n "$announced" ]; then
  [ "$announced" = "new layer surface: namespace wprs-check layer 0" ] || fail "the compositor saw: $announced"
  echo "the compositor read the namespace"
else
  echo "the compositor log does not show namespaces, only the daemon state was checked"
fi
ctl config "$key" | grep -qx "$key = \"wprs-check\"" || fail "config does not read back the namespace"
ctl config "$key" auto | grep -q "next time the daemon starts" || fail "no restart notice"
expect "s['layer_shell']['restart'], s['options']['$key']" "(True, 'auto')"
if ctl config "$key" "two words" 2>/dev/null; then fail "an invalid namespace was accepted"; fi
ctl config "$key" wprs-check >/dev/null
expect "s['layer_shell']['restart']" False
if "$BIN/wprsd" --layer-namespace "a/b" 2>"$WORK/option.err"; then fail "the daemon took an invalid namespace"; fi
grep -q "layer-namespace" "$WORK/option.err" || fail "unclear refusal: $(cat "$WORK/option.err")"
echo "layer namespace checks passed"
