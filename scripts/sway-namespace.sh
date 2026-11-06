#!/usr/bin/env bash
set -euo pipefail
declare WORK BIN log wprsd magenta
key=surfaces.layer_namespace
announced() { grep -o "new layer surface: namespace [^ ]* layer [0-9]*" | sort -u || true; }
saw() {
  local seen; seen=$(tail -n "+$1" "$WORK/sway.log" | announced)
  [ -z "$seen" ] || [ "$seen" = "new layer surface: namespace $2 layer 0" ] || fail "the compositor saw: $seen"
}
expect "s['layer_shell']" "{'namespace': 'wallpaper', 'from': 'default', 'restart': False, 'ignored': None}"
saw 1 wallpaper
[ -n "$(announced <"$WORK/sway.log")" ] || fail "the compositor log does not show layer surface namespaces"
ctl config "$key" | grep -x "$key = \"auto\"" >/dev/null || fail "config does not read back the automatic namespace"
ctl config "$key" wprs-check >"$WORK/namespace.out"
grep -q "next time the daemon starts" "$WORK/namespace.out" || fail "no restart notice: $(cat "$WORK/namespace.out")"
grep -qx "in use: wallpaper (from default)" "$WORK/namespace.out" || fail "no name in use: $(cat "$WORK/namespace.out")"
expect "s['layer_shell']['restart'], s['options']['$key']" "(True, 'wprs-check')"
ctl config "$key" | grep -x "$key = \"wprs-check\"" >/dev/null || fail "config does not read back the namespace"
if ctl config "$key" "two words" 2>/dev/null; then fail "an invalid namespace was accepted"; fi
for bad in "a/b" "" "   "; do
  if timeout 10 "$BIN/wprsd" --layer-namespace "$bad" 2>"$WORK/option.err"; then fail "the daemon took the namespace '$bad'"; fi
  grep -q "layer-namespace" "$WORK/option.err" || fail "unclear refusal: $(cat "$WORK/option.err")"
done
mark=$(($(wc -l <"$WORK/sway.log") + 1))
kill "$wprsd"
wait "$wprsd" || true
WPRS_LAYER_NAMESPACE="not valid" RUST_LOG=$log "$BIN/wprsd" --backend layer-shell >>"$WORK/wprsd.log" 2>&1 &
wprsd=$!; pids+=("$wprsd")
wait_for 30 ctl state || fail "the daemon did not start again"
expect "[s['layer_shell'][k] for k in ('namespace', 'from', 'restart')]" "['wprs-check', 'config', False]"
expect "'environment' in (s['layer_shell']['ignored'] or '')" True
check HEADLESS-1 "0.5,0.5,$magenta"
saw "$mark" wprs-check
[ -n "$(tail -n "+$mark" "$WORK/sway.log" | announced)" ] || fail "the compositor did not log the new surfaces"
echo "the compositor read the namespace before and after the restart"
ctl config "$key" "" >/dev/null
expect "s['layer_shell']['restart'], s['options']['$key']" "(True, 'auto')"
echo "layer namespace checks passed"
