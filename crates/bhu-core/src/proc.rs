//! Running other programs.

use std::ffi::OsStr;
use std::process::Command;

/// A command that does not flash a console window on Windows.
///
/// Spawning a console program from a windowed application makes Windows create
/// a console for it, which appears as a black rectangle for as long as the
/// program runs. Reading the installed applications means running PowerShell
/// once at startup, so that flash is the first thing a user sees — it looks
/// like something has gone wrong, and on a slow machine it lingers.
///
/// `CREATE_NO_WINDOW` suppresses only that console. It has no effect on a
/// program that draws its own window, so an uninstaller's interface and the
/// system's own permission prompts still appear exactly as they should.
pub fn command(program: impl AsRef<OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
