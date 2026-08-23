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

# Keep the build output out of Spotlight. Cargo recreates target/ on every
# build, so the marker has to be recreated with it — otherwise the bundled .app
# sitting there gets indexed and turns up in Finder searches and app pickers as
# though it were installed.
mkdir -p target && touch target/.metadata_never_index

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
# `.noindex` is the suffix macOS honours unconditionally — it is what Apple's
# own tools use for their caches. Without it Spotlight indexes the built app and
# it turns up in Finder searches and app pickers as though it were installed,
# which is confusing on a machine where it is not.
rm -rf dist dist.noindex
mkdir -p dist.noindex
touch dist.noindex/.metadata_never_index
cp target/release/bundle/dmg/*.dmg dist.noindex/
cp -R "$APP" dist.noindex/
cp target/release/bhu dist.noindex/ 2>/dev/null || true

cargo clean

echo
echo "Latest build in dist.noindex/ — and nothing else:"
du -sh dist.noindex/* | sed 's/^/  /'
echo "Compiler cache cleared; the next build repopulates it."
