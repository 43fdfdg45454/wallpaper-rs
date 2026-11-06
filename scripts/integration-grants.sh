#!/usr/bin/env bash
set -euo pipefail
declare WORK
sea=org.example.sea
inputs() { ctl input status --json | python3 -c "import json, sys; r = json.load(sys.stdin); print(eval(sys.argv[1]))" "$1"; }
[ "$(inputs "r['grants'], r['capturing'], r['tracking']")" = "({}, False, False)" ] || fail "inputs start on"
if ctl input grant "$sea" audio 2>"$WORK/grant.err"; then fail "an input was granted to a scene that is not shown"; fi
grep -q "is not shown" "$WORK/grant.err" || fail "grant before showing: $(cat "$WORK/grant.err")"
ctl input pointer 10 10 && ctl input pointer -10 -10
ctl input status | grep -x "audio capture: off" >/dev/null || fail "status: $(ctl input status)"
refused "is not audio, pointer, clicks, network, rgb, media or media_network" Inputs1 Grant ss "$sea" camera
refused "is not a scene package id" Inputs1 Grant ss "Not A Scene" audio
refused "Signature mismatch" Inputs1 SetPointer dd 1 1
denied "only the KWin script of wprsd and wprs-ctl report the pointer" Inputs1.SetPointer int32:1 int32:1
echo "input permission checks passed"
