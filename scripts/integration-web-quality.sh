#!/usr/bin/env bash
set -euo pipefail
declare tint
read -r _ _ _ _ < <(ctl stream TEST-W2 320 180)
ctl lib apply "$tint" -o TEST-W2 2>/dev/null
pair() { field "s['outputs']['TEST-W'].get('helper'), s['outputs']['TEST-W2'].get('helper')"; }
same() { local p; p=$(pair); [[ $p != *None* ]] && python3 -c "import sys; a, b = eval(sys.argv[1]); sys.exit(a != b)" "$p"; }
apart() { local p; p=$(pair); [[ $p != *None* ]] && python3 -c "import sys; a, b = eval(sys.argv[1]); sys.exit(a == b)" "$p"; }
wait_for 20 same || fail "two outputs that share a quality did not share one page: $(pair)"
ctl quality q0 --output TEST-W2 >/dev/null
wait_for 20 apart || fail "an output with its own quality shared the page of another: $(pair)"
ctl quality none --output TEST-W2 >/dev/null
wait_for 20 same || fail "the page was not shared again once the quality was the same: $(pair)"
ctl release TEST-W2
echo "pages take the quality of the output that shows them"
