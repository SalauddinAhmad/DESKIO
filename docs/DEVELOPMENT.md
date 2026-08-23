# Working on BHUninstaller

Notes for anyone building or extending this — including the platform work that
is still outstanding.

## Getting set up

```bash
git clone https://github.com/wpexpertinbd/BHUninstaller.git
cd BHUninstaller

cargo build --release        # engine + CLI
cargo test                   # 48 tests, no platform setup needed

cd app && npm ci
npm run tauri dev            # the app
npm run tauri build          # installers for this platform
```

**Windows** additionally needs the WebView2 runtime (present on Windows 11) and
the MSVC build tools. **Linux** needs the Tauri v2 dependencies:

```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev patchelf \
  build-essential curl wget file libxdo-dev libssl-dev
```

Start with the CLI — it exercises the whole engine with no UI in the way, and
`list`, `plan`, `orphans`, `cleanup` and `access` change nothing at all:

```bash
./target/release/bhu list
./target/release/bhu plan "Some App"     # a dry run; nothing moves
```

## Where things live

Everything platform-specific is in an adapter. The model, the safety rules, the
matching, planning, removal, the undo journal and the entire interface are
shared, so adding or fixing a platform means touching two or three files:

```
crates/bhu-core/src/
  discovery/{macos,windows,linux}.rs   what is installed
  leftovers/{macos,windows,linux}.rs   what was left behind
  startup/{macos,windows,linux}.rs     what runs at login
  extensions/{macos,windows,linux}.rs  browser and system add-ons
  cleaner/{macos,windows,linux}.rs     reclaimable junk
  safety.rs      the blocklist — read this before changing removal
  removal.rs     planning and execution
  elevate.rs     the privileged path
```

You can type-check another platform without leaving yours:

```bash
rustup target add x86_64-pc-windows-msvc
cargo check -p bhu-core --no-default-features --target x86_64-pc-windows-msvc
```

`--no-default-features` drops the update check, which is the only part that
needs a TLS stack — and therefore a C cross-compiler. CI does the same on real
runners, which is where the foreign adapters actually get compiled.

## What is not finished

**Linux has never been run by anyone.** It compiles and produces a `.deb` and
`.rpm` in CI. Nothing more is known about it.

**Windows is being tested but is young.** These paths in particular are written
and have not been confirmed working:

- `elevate.rs` — the elevated quarantine under `%LOCALAPPDATA%\BHUninstaller\Quarantine`
- `removal.rs::run_delegated` — running an uninstaller with administrator rights
- **registry leftovers** — scanning, exporting and deleting keys, and the
  elevated batch for `HKLM`. Read the safety rules in `safety.rs`
  (`check_registry_removable`) before changing anything here: only
  `HK{CU,LM}\Software\<Vendor>…` is ever eligible, shared subtrees are refused
  outright, and a key is never deleted unless its `.reg` export was written
  first — that export is the only undo a registry key has.

## Things that cost time to find

- **`qlmanage -t` renders SVGs onto a white card.** Anything rasterised that way
  carries a white background. `scripts/make-icons.py` draws the artwork directly.
- **Windows composites PNG-in-`.ico` onto white** at 16/32/48 in the taskbar and
  title bar. Small sizes must be uncompressed BMP with an AND mask.
- **Uninstaller exit codes mean nothing.** Inno Setup's `unins000.exe` returns 1
  both when it hands off to an elevated copy of itself and when the user answers
  "No". Check whether the install directory is gone instead.
- **An item that is already gone is a success, not a failure** — the app's own
  uninstaller usually removed it a second earlier.
- **`launchctl print-disabled` wording changed**: older macOS prints `=> true`,
  macOS 26/27 prints `=> disabled`. Parse both.
- **macOS ships bash 3.2** — no `mapfile` in CI scripts.
- **`cargo fmt` rewraps code**, so a scripted find-and-replace written against
  unformatted text silently matches nothing. Verify edits landed.

## Releasing

Tag `vX.Y.Z` and push. CI builds every platform, attaches the installers, and
**deletes all older releases** — only the newest is ever kept. The in-app
updater reads `releases/latest`, so every release needs an asset per platform.

The version lives in three files and they must agree: `Cargo.toml`,
`app/package.json`, `app/src-tauri/tauri.conf.json`.

macOS bundling sets `APPLE_SIGNING_IDENTITY=-` so the app is ad-hoc signed
during bundling — signing after the disk image is built is too late, and an
unsigned build reads as "damaged" rather than merely unverified.
