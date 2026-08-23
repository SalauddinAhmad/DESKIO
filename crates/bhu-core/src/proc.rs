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

/// Create a scratch directory that nothing else could have prepared first.
///
/// Several of the things this app writes to a temporary directory are then
/// *executed*, some of them with administrator rights: the vendor's uninstaller
/// is wrapped in a `.cmd` file, the elevated move and the registry sweep are
/// PowerShell scripts, and a downloaded update is an installer the user will
/// approve. Anything able to substitute one of those files between writing it
/// and running it gets its own code run with those rights.
///
/// A fixed name — even one carrying the process id, which is small, reused, and
/// on Windows a multiple of four — can be created in advance by whoever gets
/// there first, as a directory to write into later or as a symlink pointing
/// somewhere else entirely. So the name is unpredictable, and `create_dir`
/// refuses a path that already exists at all, symlink included, rather than
/// following it. The result is verified to be the directory we just made.
///
/// The caller owns the directory and should remove it when finished.
pub fn private_temp_dir(prefix: &str) -> Result<std::path::PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let base = std::env::temp_dir();
    let mut last = String::new();
    // A collision means something is racing us; a few attempts is enough to
    // out-run that, and failing is the correct outcome if it is not.
    for attempt in 0..8u32 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let dir = base.join(format!(
            "{prefix}-{}-{:08x}{:x}",
            std::process::id(),
            nanos,
            attempt
        ));
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                let meta = std::fs::symlink_metadata(&dir)
                    .map_err(|e| format!("could not verify {}: {e}", dir.display()))?;
                if meta.is_symlink() || !meta.is_dir() {
                    return Err("the scratch folder was not what we created — refusing".into());
                }
                return Ok(dir);
            }
            Err(e) => last = e.to_string(),
        }
    }
    Err(format!("could not create a scratch folder: {last}"))
}
