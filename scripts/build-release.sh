#!/usr/bin/env bash
# Build, sign and verify the macOS release, then prune everything that is not
# the finished product.
#
# Only the current build is kept. Intermediates (the debug profile, the
# cross-compile check artefacts, the disk image scratch files) are removed
# afterwards — they are regenerated on demand and otherwise grow to several
# gigabytes without anyone noticing.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "==> Engine"
cargo build --release
cargo test --workspace --exclude bhuninstaller

echo "==> App"
(cd app && npm run tauri build)

APP="target/release/bundle/macos/BHUninstaller.app"

# The ad-hoc signature must be applied LAST. Tauri edits the bundle after
# signing it, which leaves the signature stale — and a stale signature reads as
# "damaged" on Apple silicon once the app has been downloaded.
echo "==> Signing"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"
echo "    signature valid"

# The finished product is a few megabytes; the compiler cache behind it grows
# to several gigabytes. Keep the first, discard the second.
echo "==> Keeping the build, discarding the cache"
rm -rf dist
mkdir -p dist
cp target/release/bundle/dmg/*.dmg dist/
cp -R "$APP" dist/
cp target/release/bhu dist/ 2>/dev/null || true

cargo clean

echo
echo "Latest build in dist/ — and nothing else:"
du -sh dist/* | sed 's/^/  /'
echo "Compiler cache cleared; the next build repopulates it."
