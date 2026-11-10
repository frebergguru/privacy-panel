//! privacy-panel-helper: makes system-wide changes for Privacy Panel. Runs as root via
//! pkexec. Reads one JSON request on stdin and writes one JSON response on stdout.
//! Every operation is checked against the fixed table in privacy_core::admin; anything
//! else is refused. It never takes a file path or a command from the request.

use std::io::Read;
use std::path::PathBuf;

use privacy_core::admin::{handle_request, AdminRequest};
use privacy_core::Env;

fn main() {
    // New policy files and folders must be readable by browsers running as the user,
    // whatever umask pkexec passed on: folders 0755, files 0644.
    unsafe {
        libc::umask(0o022);
    }
    let dry = std::env::args().any(|a| a == "--dry-run");
    let mut input = String::new();
    if std::io::stdin().take(1 << 20).read_to_string(&mut input).is_err() {
        eprintln!("could not read the request");
        std::process::exit(2);
    }
    let mut req: AdminRequest = match serde_json::from_str(&input) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("bad request: {e}");
            std::process::exit(2);
        }
    };
    if dry {
        req.dry_run = true;
    }
    // Only system paths are used; no user environment is read.
    let env = Env {
        home: PathBuf::from("/nonexistent"),
        config: PathBuf::from("/nonexistent"),
        data: PathBuf::from("/nonexistent"),
        cache: PathBuf::from("/nonexistent"),
        state: PathBuf::from("/nonexistent"),
        root: PathBuf::from("/"),
        live: true,
        processes: None,
        desktop: String::new(),
    };
    let resp = handle_request(&env, &req);
    println!("{}", serde_json::to_string(&resp).unwrap());
    let all_ok = resp.results.iter().all(|r| r.ok);
    std::process::exit(if all_ok { 0 } else { 1 });
}
