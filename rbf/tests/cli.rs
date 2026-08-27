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
fn prints_bulk_constant_output() {
    assert_eq!(run_program("[-].[-]+.", b""), b"\0\x01");
}

#[test]
fn wraps_cells_at_256() {
    assert_eq!(run_program(&format!("[-]{}.", "+".repeat(255)), b""), [255]);
}

#[test]
fn runs_optimized_multiply_loop() {
    assert_eq!(run_program("+++++[>+++++++++++++<-]>.", b""), b"A");
}

#[test]
fn runs_input_dependent_multiply_loop() {
    assert_eq!(run_program(",[>+>->++<<<-]>.>.>.", &[5]), [5, 251, 10]);
}

fn emit(source: &str, mode: &str) -> String {
    let path = write_program(source);
    let output = Command::new(env!("CARGO_BIN_EXE_rbf"))
        .arg("-e")
        .arg(mode)
        .arg(&path)
        .output()
        .expect("failed to run rbf");
    let _ = fs::remove_file(&path);

    assert!(
        output.status.success(),
        "rbf failed with status {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("emitted output is not utf-8")
}

#[test]
fn emits_disassembly() {
    let disassembly = emit("+++++[>+++++++++++++<-]>.", "asm");
    let (listing, summary) = disassembly
        .rsplit_once("\n\n")
        .expect("disassembly has no summary");
    let lines: Vec<&str> = listing.lines().collect();

    assert!(!lines.is_empty(), "disassembly is empty");
    assert!(
        !listing.contains("(bad"),
        "disassembly contains undecodable bytes:\n{disassembly}"
    );

    let mut next_offset = 0;
    for line in &lines {
        let mut columns = line.split_whitespace();
        let offset = columns.next().expect("missing offset");
        let bytes = columns.next().expect("missing bytes");
        let mnemonic = columns.next().expect("missing mnemonic");

        assert!(
            offset.len() >= 4 && offset.chars().all(|c| c.is_ascii_hexdigit()),
            "bad offset in {line:?}"
        );
        assert!(
            !bytes.is_empty()
                && bytes.len().is_multiple_of(2)
                && bytes.chars().all(|c| c.is_ascii_hexdigit()),
            "bad bytes in {line:?}"
        );
        assert!(!mnemonic.is_empty(), "missing mnemonic in {line:?}");

        assert_eq!(
            usize::from_str_radix(offset, 16).expect("offset is not hexadecimal"),
            next_offset,
            "offset does not follow the previous instruction in {line:?}"
        );
        next_offset += bytes.len() / 2;
    }

    let epilogue = lines.last().expect("disassembly is empty");
    assert!(
        epilogue
            .split_whitespace()
            .nth(2)
            .is_some_and(|mnemonic| mnemonic.starts_with("ret")),
        "disassembly does not end in a return: {epilogue:?}"
    );

    assert_eq!(
        summary.trim_end(),
        format!("; {} instructions, {next_offset} bytes", lines.len())
    );
}

#[test]
fn annotates_helpers_and_literals() {
    let disassembly = emit(",.[-].[-]+.", "asm");

    for helper in ["memzero", "getchar", "putchar", "putbytes"] {
        assert!(
            disassembly.contains(&format!("; {helper}")),
            "disassembly does not name {helper}:\n{disassembly}"
        );
    }

    assert!(
        disassembly.contains(r#"; "\x00\x01""#),
        "disassembly does not show the written bytes:\n{disassembly}"
    );
}

#[test]
fn disassembly_does_not_run_the_program() {
    let disassembly = emit(&format!("[-]{}.", "+".repeat(b'Z' as usize)), "asm");

    assert!(
        !disassembly.contains('Z'),
        "program ran while emitting its machine code:\n{disassembly}"
    );
}
