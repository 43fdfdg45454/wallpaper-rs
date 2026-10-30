#!/bin/bash
set -euo pipefail
results=${1:-android/app/build/outputs/androidTest-results}
python3 - "$results" <<'EOF'
import glob, sys, xml.etree.ElementTree as ET

found = 0
for path in sorted(glob.glob(f"{sys.argv[1]}/**/*.xml", recursive=True)):
    for case in ET.parse(path).getroot().iter("testcase"):
        for bad in [*case.iter("failure"), *case.iter("error")]:
            found += 1
            print(f"::error title=Instrumentation test failed::{case.get('classname')}.{case.get('name')}")
            print("\n".join((bad.text or bad.get("message") or "").splitlines()[:40]))
print(f"{found} failing instrumentation tests in {sys.argv[1]}")
EOF
