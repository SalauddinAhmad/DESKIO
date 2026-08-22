; Branded intro for the Windows installer, mirroring the first screen of the
; BHServe setup.
;
; Tauri includes this file near the top of its NSIS template — before the
; welcome page macro is inserted — which is what lets these MUI defines take
; effect. The hook macros themselves are guarded with !ifmacrodef upstream, so
; none need to be defined here.

!define MUI_WELCOMEPAGE_TITLE "Welcome to BHUninstaller"

!define MUI_WELCOMEPAGE_TEXT "Uninstall apps properly, and clean up what they leave behind - a free alternative to Revo Uninstaller and App Cleaner.$\r$\n$\r$\n\
This installs BHUninstaller on your PC. It can:$\r$\n$\r$\n\
      -   Uninstall any app, and sweep up the files it leaves behind$\r$\n\
      -   Find leftovers from apps you removed long ago$\r$\n\
      -   Manage what runs at startup, and your browser and system extensions$\r$\n\
      -   Clear caches, logs, crash reports and build junk$\r$\n\
      -   Show which of your apps have a newer version available$\r$\n\
      -   Put any removal back - nothing is ever deleted, only moved to the Recycle Bin$\r$\n$\r$\n\
Every removal is shown to you first, with the full path of each file and the reason it was matched.$\r$\n$\r$\n\
Windows support is new and still being tested - please report anything that looks wrong.$\r$\n$\r$\n\
Completely free & open-source - built with love by BiswasHost."
