use super::Staged;

pub(super) fn install(staged: Staged) {
    // GPUI polls quit futures after clearing windows. Wait here because its
    // shutdown timeout must not interrupt replacement of the application.
    let worker = std::thread::Builder::new()
        .name("datalith-update-install".into())
        .spawn(move || {
            #[cfg(unix)]
            let executable = std::env::var_os("APPIMAGE")
                .map(std::path::PathBuf::from)
                .map_or_else(std::env::current_exe, Ok);
            if staged.update.install(&staged.bytes).is_err() {
                eprintln!("The update couldn't be installed");
                return;
            }
            #[cfg(unix)]
            if let Err(error) = executable.and_then(relaunch) {
                eprintln!("Couldn't restart Datalith: {error}");
            }
        });
    match worker {
        Ok(worker) => {
            if worker.join().is_err() {
                eprintln!("The update installer panicked");
            }
        }
        Err(error) => eprintln!("Couldn't start the update installer: {error}"),
    }
}

#[cfg(unix)]
fn relaunch(executable: std::path::PathBuf) -> std::io::Result<()> {
    use std::os::unix::process::CommandExt as _;
    use std::process::{Command, Stdio};

    Command::new("/bin/sh")
        .args([
            "-c",
            "while kill -0 \"$1\" 2>/dev/null; do sleep 1; done; exec \"$2\"",
            "datalith-restart",
        ])
        .arg(std::process::id().to_string())
        .arg(executable)
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .current_dir("/")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(|_| ())
}
