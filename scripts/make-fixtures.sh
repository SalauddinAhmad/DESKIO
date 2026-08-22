#!/usr/bin/env bash
# Capture this machine's real scan results so the UI can be developed in a
# browser (`npm run dev`) without the native window. Never commit the output:
# it lists everything installed here, with paths.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
python3 - <<'PY' > app/src/fixtures.json
import json, subprocess, sys
BIN = "./target/release/bhu"
def run(*a):
    out = subprocess.run([BIN, *a, "--json"], capture_output=True, text=True)
    try: return json.loads(out.stdout)
    except Exception: return []
apps = run("list")
json.dump({
    "apps": apps,
    "orphans": run("orphans"),
    "plans": {a["id"]: p for a in apps if (p := run("plan", a["id"]))},
    "startup": run("startup"),
    "extensions": run("extensions"),
    "updates": run("updates"),
    "junk": run("cleanup"),
    "history": run("history"),
}, sys.stdout)
PY
echo "wrote app/src/fixtures.json"
