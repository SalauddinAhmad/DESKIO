# DESKIO — Organize. Clean. Simplify.

A native cross-platform application uninstaller, developer cache sweeper, and workspace optimizer built with **Tauri 2**, **React 19**, and **Rust**. Free, open-source, and 100% safe.

---

## 📥 Download DESKIO Latest Release

Click your platform button below to download the latest installer from GitHub Releases:

[![Download for macOS](https://img.shields.io/badge/Download-macOS%20(.dmg)-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/SalauddinAhmad/DESKIO/releases/latest)
[![Download for Windows](https://img.shields.io/badge/Download-Windows%20(.msi)-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/SalauddinAhmad/DESKIO/releases/latest)
[![Download for Linux](https://img.shields.io/badge/Download-Linux%20(.deb%2F.rpm)-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://github.com/SalauddinAhmad/DESKIO/releases/latest)

> 💡 Or visit the [**GitHub Releases Page**](https://github.com/SalauddinAhmad/DESKIO/releases/latest) to choose all available build formats.

---

## ✨ Features & Highlights

- 🚀 **Complete Application Uninstaller**: Detects every installed app along with its leftover support files, caches, containers, launch agents, and preferences.
- 💻 **Developer Build Cleaner**: Deep scans for heavy `node_modules`, Cargo `target` build caches, Xcode `DerivedData`, and package manager temp files.
- 📊 **System Overview Dashboard**: Visual storage breakdown, health metrics, and 1-click system cleanup.
- 🛡️ **100% Reversible Safety**: Nothing is deleted permanently outright. Everything moves safely to system Trash / Recycle Bin, backed by itemized review sheets.
- ⚡ **Native Engine Performance**: Rust-powered file discovery engine ensuring lightning-fast scanning across macOS, Windows, and Linux.

---

## 🛠️ Architecture

DESKIO is structured as a Rust workspace with a React frontend:

- `crates/dc-core`: Core engine for application discovery, leftover path matching, blocklist enforcement, and trash execution.
- `crates/dc-cli`: Headless CLI runner (`deskio`) for command-line inspections and automation.
- `app/`: Modern React 19 + TypeScript frontend with a high-tech glassmorphism theme.
- `app/src-tauri`: Tauri 2 desktop app wrapper and IPC bindings.

---

## 🚀 Building & Running Locally

### Prerequisites
- [Rust](https://www.rust-lang.org/) (stable)
- [Node.js](https://nodejs.org/) (v20+)

### Quick Start

1. Install frontend dependencies:
   ```bash
   cd app
   npm install
   ```

2. Run in frontend preview mode:
   ```bash
   npm run dev
   ```

3. Launch as desktop application via Tauri:
   ```bash
   npm run tauri dev
   ```

4. Build production desktop installer:
   ```bash
   npm run tauri build
   ```

---

## 📄 License

Distributed under the MIT License.
