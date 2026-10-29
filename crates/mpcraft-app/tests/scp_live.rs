//! The logs of a companion, read over SSH from a server that is really there.
//!
//! The unit tests in `mpcraft-app` assert the address, the selection and the safe name without a
//! server. What they cannot say is that the client opens an SFTP session, lists a directory and
//! copies the bytes. This area does that against a server named by the environment; it is skipped
//! when the environment names none, so a machine without one still runs the rest of the suite.
//!
//! ```text
//! MPCRAFT_SCP_TARGET="user:password@127.0.0.1:2222:/home/user/logs/" \
//! cargo test -p mpcraft-app --test scp_live -- --nocapture
//! ```
//!
//! The server used to prove it is a container of `atmoz/sftp` with a folder of `.bin` files; it is
//! an SSH server, not a companion computer, and that is the difference this area records.

use std::path::PathBuf;

use mpcraft_app::scp::{download_logs, list_logs, ScpTarget};

#[tokio::test]
async fn a_companion_lists_its_logs_and_gives_them_over_scp() {
    let Some(text) = std::env::var_os("MPCRAFT_SCP_TARGET") else {
        println!(
            "MPCRAFT_SCP_TARGET does not name a companion, so nothing is reached; the unit tests carry the address and the selection"
        );
        return;
    };

    let text = text.to_string_lossy();
    let target = ScpTarget::parse(&text).expect("the environment names a companion");
    let logs = list_logs(&target).await.expect("the companion answers");

    assert!(!logs.is_empty(), "the companion holds at least one log");

    for log in &logs {
        assert!(
            log.name.to_ascii_lowercase().ends_with(".bin"),
            "{} is a log the companion lists",
            log.name
        );
    }

    println!("the companion lists {} logs", logs.len());

    let folder = std::env::temp_dir().join(format!("mpcraft-scp-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("a folder for the copies");

    let names: Vec<String> = logs.iter().map(|log| log.name.clone()).collect();
    let copied = download_logs(&target, &names, &folder)
        .await
        .expect("the companion gives the logs");

    assert_eq!(copied.len(), names.len(), "every log was copied");

    for log in &logs {
        let path: PathBuf = folder.join(&log.name);
        let bytes = std::fs::read(&path).expect("the copy is on the disk");

        assert_eq!(
            bytes.len() as u64,
            log.size,
            "{} arrived with the size the companion said",
            log.name
        );
    }

    println!(
        "{} logs copied to {}; the sizes agree with the listing",
        copied.len(),
        folder.display()
    );
}
