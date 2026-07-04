use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_PROGRAM_ID: AtomicU64 = AtomicU64::new(0);

fn run_program(source: &str, input: &[u8]) -> Vec<u8> {
    let path = write_program(source);
    let mut child = Command::new(env!("CARGO_BIN_EXE_rbf"))
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to run rbf");

    if !input.is_empty() {
        child
            .stdin
            .take()
            .expect("failed to open stdin")
            .write_all(input)
            .expect("failed to write stdin");
    }

    let output = child.wait_with_output().expect("failed to wait for rbf");
    let _ = fs::remove_file(&path);

    assert!(
        output.status.success(),
        "rbf failed with status {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    output.stdout
}

fn write_program(source: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX epoch")
        .as_nanos();
    let id = NEXT_PROGRAM_ID.fetch_add(1, Ordering::Relaxed);
    path.push(format!("rbf-{}-{unique}-{id}.bf", std::process::id()));
    fs::write(&path, source).expect("failed to write test program");
    path
}

#[test]
fn prints_output() {
    assert_eq!(
        run_program(
            "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++++.",
            b""
        ),
        b"A"
    );
}

#[test]
fn reads_input() {
    assert_eq!(run_program(",.", b"Z"), b"Z");
}

#[test]
fn runs_optimized_multiply_loop() {
    assert_eq!(run_program("+++++[>+++++++++++++<-]>.", b""), b"A");
}
