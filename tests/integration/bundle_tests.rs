// Copyright 2018-2026 the Deno authors. MIT license.

#![cfg(unix)]

use std::time::Duration;

use test_util as util;
use test_util::TestContextBuilder;
use test_util::assert_contains;
use test_util::test;

/// Set up a named pipe (FIFO) `source.ts` with a symlink `alias.ts -> source.ts`
/// in the test's temp cwd, feed `source` into the pipe from a background writer,
/// and run `deno bundle <check_arg> alias.ts` with a bounded timeout.
///
/// `check_arg` is the single check flag to pass (e.g. `--check` or
/// `--check=all`); clap rejects `--check` given more than once, so callers must
/// supply exactly one.
///
/// This exercises denoland/deno#36162: a symlink to a non-regular file drives
/// the `canonicalize`-would-*rewrite* path (unlike `/dev/stdin`, where on Linux
/// `canonicalize` errors instead of rewriting). If `resolve_url_or_path_absolute`
/// canonicalized `alias.ts` to `source.ts`, the shared in-memory `File` would be
/// keyed by the rewritten URL while the `--check` graph's `collect_specifiers`
/// keeps `alias.ts`; the check graph would then miss the memory entry and reopen
/// the already-drained FIFO, hanging forever. The bounded timeout turns that
/// regression into a fast failure instead of a CI hang.
///
/// Uses `for_npm()` (http server + npm env) so `deno bundle` can fetch the
/// esbuild binary from the test registry, matching the bundle spec tests.
fn run_bundle_check_symlink_to_fifo(
  source: &str,
  check_arg: &str,
) -> std::process::Output {
  use std::os::unix::fs::symlink;

  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let fifo_path = temp_dir.path().join("source.ts");
  let alias_path = temp_dir.path().join("alias.ts");

  // Create the FIFO. std has no mkfifo, so shell out to the coreutils binary.
  let status = std::process::Command::new("mkfifo")
    .arg(fifo_path.to_string())
    .status()
    .expect("failed to spawn mkfifo");
  assert!(status.success(), "mkfifo failed");

  symlink(fifo_path.as_path(), alias_path.as_path()).unwrap();

  // Background writer: opening a FIFO for writing blocks until a reader opens
  // it, so this rendezvous with deno's first (and, after the fix, only) read of
  // the entrypoint. Writing then closing gives that reader EOF. `deno bundle`
  // reads a non-regular entrypoint exactly once into memory, so the writer
  // always unblocks and finishes even on the regression path (where the check
  // graph's *second* reopen is what hangs).
  let fifo_for_writer = fifo_path.to_string();
  let source = source.to_string();
  let (writer_done_tx, writer_done_rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let _ = std::fs::write(&fifo_for_writer, source);
    let _ = writer_done_tx.send(());
  });

  let child = context
    .new_command()
    .arg("bundle")
    .arg(check_arg)
    .arg("alias.ts")
    .stdout_piped()
    .stderr_piped()
    .spawn()
    .unwrap();

  let output = child
    .wait_with_output_and_timeout(Duration::from_secs(30))
    .expect(
      "`deno bundle --check alias.ts` hung reopening the drained FIFO \
       (regression of denoland/deno#36162)",
    );

  // Wait (bounded) for the writer so the temp dir can be cleaned up. If deno
  // ever exited without opening the FIFO for reading, the writer stays parked
  // in `open()` forever; don't block the suite on it — the output assertions
  // below turn that into a normal failure instead of a hang.
  let _ = writer_done_rx.recv_timeout(Duration::from_secs(5));

  output
}

/// Like `run_bundle_check_symlink_to_fifo`, but passes the SAME symlinked FIFO
/// entrypoint TWICE and writes the bundle to `--outdir`. The single background
/// writer feeds the pipe exactly once, so this proves the non-regular entrypoint
/// is read at most once even when duplicated: after the fix
/// `read_nonregular_entrypoints` dedupes by resolved URL and reads the pipe once
/// (rendezvous with the one writer → EOF), so deno finishes. A regression that
/// re-reads the drained FIFO on the second occurrence blocks on the second
/// `open()` (the writer is already gone) and the bounded timeout fails fast
/// instead of hanging CI. Returns (output, concatenated bundled `.js` content
/// written to `--outdir`). The bundle is read here, before the test context's
/// temp dir is dropped (which would delete it).
fn run_bundle_check_dup_symlink_to_fifo(
  source: &str,
) -> (std::process::Output, String) {
  use std::os::unix::fs::symlink;

  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  let fifo_path = temp_dir.path().join("source.ts");
  let alias_path = temp_dir.path().join("alias.ts");
  let outdir = temp_dir.path().join("out").to_path_buf();

  let status = std::process::Command::new("mkfifo")
    .arg(fifo_path.to_string())
    .status()
    .expect("failed to spawn mkfifo");
  assert!(status.success(), "mkfifo failed");

  symlink(fifo_path.as_path(), alias_path.as_path()).unwrap();

  // Writer feeds the pipe exactly once. After the fix only one read happens, so
  // this rendezvous with it and gives EOF. A regression opens the drained FIFO a
  // second time (no writer left) and blocks, tripping the timeout below.
  let fifo_for_writer = fifo_path.to_string();
  let source = source.to_string();
  let (writer_done_tx, writer_done_rx) = std::sync::mpsc::channel();
  std::thread::spawn(move || {
    let _ = std::fs::write(&fifo_for_writer, source);
    let _ = writer_done_tx.send(());
  });

  let child = context
    .new_command()
    .arg("bundle")
    .arg("--check")
    .arg(format!("--outdir={}", outdir.to_string_lossy()))
    // The same non-regular entrypoint passed twice — valid input that must not
    // read the pipe twice (denoland/deno#36162 review follow-up).
    .arg("alias.ts")
    .arg("alias.ts")
    .stdout_piped()
    .stderr_piped()
    .spawn()
    .unwrap();

  let output = child
    .wait_with_output_and_timeout(Duration::from_secs(30))
    .expect(
      "`deno bundle --check alias.ts alias.ts` hung reopening the drained \
       FIFO for the duplicate entrypoint (regression of denoland/deno#36162)",
    );

  let _ = writer_done_rx.recv_timeout(Duration::from_secs(5));

  // Read the emitted bundle while the temp dir is still alive (it is deleted
  // when `context`/`temp_dir` drop at the end of this function).
  let mut bundled = String::new();
  if let Ok(entries) = std::fs::read_dir(&outdir) {
    for entry in entries.flatten() {
      let path = entry.path();
      if path.extension().and_then(|e| e.to_str()) == Some("js") {
        bundled = std::fs::read_to_string(&path).unwrap();
        break;
      }
    }
  }

  (output, bundled)
}

// Regression test for the denoland/deno#36162 review follow-up: passing the same
// non-regular entrypoint twice must read the pipe only once (dedup by resolved
// URL) and still produce a non-empty bundle, not a 0-byte file or a hang.
#[test]
fn bundle_check_dup_symlink_to_fifo_read_once() {
  let (output, bundled) =
    run_bundle_check_dup_symlink_to_fifo("const x = 5;\nconsole.log(x);\n");

  let stdout =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap());
  let stderr =
    util::strip_ansi_codes(std::str::from_utf8(&output.stderr).unwrap());

  assert!(
    output.status.success(),
    "expected success, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    output.status.code()
  );

  // The bundled output must contain the piped source (not a 0-byte file from a
  // second EOF read overwriting the first).
  assert!(
    !bundled.is_empty(),
    "expected a non-empty bundled .js\nstdout:\n{stdout}\nstderr:\n{stderr}",
  );
  assert_contains!(bundled, "console.log(x)");
}

// Regression test for denoland/deno#36162: `bundle --check` of a symlink to a
// FIFO must produce the bundle (from content read once) instead of hanging.
#[test]
fn bundle_check_symlink_to_fifo_no_hang() {
  let output = run_bundle_check_symlink_to_fifo(
    "const x: number = 5;\nconsole.log(x);\n",
    "--check",
  );

  let stdout =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap());
  let stderr =
    util::strip_ansi_codes(std::str::from_utf8(&output.stderr).unwrap());

  assert!(
    output.status.success(),
    "expected success, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    output.status.code()
  );
  // The bundled entrypoint content, proving the pipe was read (not EOF/empty).
  assert_contains!(stdout, "console.log(x)");
  // The type-check graph actually ran on `alias.ts`.
  assert_contains!(stderr, "Check");
}

// Proves the `--check` graph reads the piped content, not just esbuild: a type
// error only surfaces if the type-checker sees the real source. Because the
// entrypoint keeps its `.ts` extension (the fix does not canonicalize it away
// to the extensionless FIFO target), it is type-checked; esbuild alone would
// strip the annotation and succeed, so an `exitCode: 1` TS2322 here can only
// come from the check graph having read the piped bytes (an EOF/empty module
// would pass type-checking).
#[test]
fn bundle_check_symlink_to_fifo_type_error_from_check() {
  let output = run_bundle_check_symlink_to_fifo(
    "const x: number = \"str\";\nconsole.log(x);\n",
    "--check=all",
  );

  let stdout =
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap());
  let stderr =
    util::strip_ansi_codes(std::str::from_utf8(&output.stderr).unwrap());

  assert!(
    !output.status.success(),
    "expected type-check failure, got success\nstdout:\n{stdout}\nstderr:\n{stderr}",
  );
  assert_contains!(stderr, "TS2322");
  assert_contains!(stderr, "Type 'string' is not assignable to type 'number'");
}
