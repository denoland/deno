// Copyright 2018-2026 the Deno authors. MIT license.

#![cfg(unix)]

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use test_util as util;
use test_util::TestContextBuilder;
use test_util::assert_contains;
use test_util::test;

/// `deno bundle` in these tests downloads esbuild from the test npm registry, so
/// this bound has to survive a cold, loaded CI runner. It exists only to turn a
/// regression that blocks forever on a drained FIFO into a failure instead of a
/// hung job.
const BUNDLE_TIMEOUT: Duration = Duration::from_secs(180);

fn mkfifo(path: &Path) {
  use nix::sys::stat::Mode;
  nix::unistd::mkfifo(path, Mode::S_IRUSR | Mode::S_IWUSR)
    .unwrap_or_else(|err| panic!("mkfifo {} failed: {err}", path.display()));
}

/// Feeds a source string into a FIFO from a background thread.
///
/// Opening a FIFO for writing blocks until a reader opens it, so the writer
/// rendezvous with deno's first (and, after the fix, only) read of the
/// entrypoint; writing then closing gives that reader EOF.
struct FifoWriter {
  fifo_path: PathBuf,
  done_rx: std::sync::mpsc::Receiver<()>,
  handle: std::thread::JoinHandle<()>,
}

impl FifoWriter {
  fn spawn(fifo_path: PathBuf, source: &str) -> Self {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let path = fifo_path.clone();
    let source = source.to_string();
    let handle = std::thread::spawn(move || {
      let _ = std::fs::write(&path, source);
      let _ = done_tx.send(());
    });
    Self {
      fifo_path,
      done_rx,
      handle,
    }
  }

  /// Waits for the writer and joins it, so neither the thread nor its fd leaks
  /// into the rest of the test run.
  ///
  /// On a regression path deno can exit without ever opening the FIFO, leaving
  /// the writer parked in `open()` forever; opening the read end here (with
  /// `O_NONBLOCK`, which never blocks on a FIFO) unparks it. The fd is held
  /// until the writer is done so its `write` doesn't hit `EPIPE`.
  fn finish(self) {
    use nix::fcntl::OFlag;
    use nix::sys::stat::Mode;

    let timeout = Duration::from_secs(10);
    let mut done = self.done_rx.recv_timeout(timeout).is_ok();
    if !done {
      let _read_end = nix::fcntl::open(
        &self.fifo_path,
        OFlag::O_RDONLY | OFlag::O_NONBLOCK,
        Mode::empty(),
      );
      done = self.done_rx.recv_timeout(timeout).is_ok();
    }
    if done {
      let _ = self.handle.join();
    }
    // If it is still wedged, don't block the suite on it: the caller's
    // assertions report the real failure.
  }
}

/// Set up a named pipe (FIFO) `source.ts` with a symlink `alias.ts -> source.ts`
/// in the test's temp cwd, feed `source` into the pipe from a single background
/// writer, and run `deno bundle <args...>` with a bounded timeout. When `outdir`
/// is set, `--outdir` is passed too and the emitted `.js` files are read back
/// (before the temp dir is deleted) and returned by file name.
///
/// This exercises denoland/deno#36162: a symlink to a non-regular file drives
/// the `canonicalize`-would-*rewrite* path (unlike `/dev/stdin`, where on Linux
/// `canonicalize` errors instead of rewriting). If `resolve_url_or_path_absolute`
/// canonicalized `alias.ts` to `source.ts`, the shared in-memory `File` would be
/// keyed by the rewritten URL while the `--check` graph's `collect_specifiers`
/// keeps `alias.ts`; the check graph would then miss the memory entry and reopen
/// the already-drained FIFO, hanging forever. Since the writer feeds the pipe
/// exactly once, any second read blocks and the bounded timeout turns that
/// regression into a fast failure instead of a CI hang.
///
/// Uses `for_npm()` (http server + npm env) so `deno bundle` can fetch the
/// esbuild binary from the test registry, matching the bundle spec tests.
#[derive(Default)]
struct FifoBundle<'a> {
  /// Fed into the FIFO by the background writer, exactly once.
  source: &'a str,
  /// Args after `deno bundle`.
  args: &'a [&'a str],
  /// Pass `--outdir` and read the emitted `.js` files back.
  outdir: bool,
  /// Written as `deno.json` in the temp cwd. `{alias_url}` and `{source_url}`
  /// are replaced with the `file:` URLs of the symlink and the FIFO.
  deno_json: Option<&'a str>,
}

impl FifoBundle<'_> {
  fn run(self) -> (std::process::Output, BTreeMap<String, String>) {
    use std::os::unix::fs::symlink;

    let context = TestContextBuilder::for_npm().use_temp_cwd().build();
    let temp_dir = context.temp_dir();
    let fifo = temp_dir.path().join("source.ts");
    let alias = temp_dir.path().join("alias.ts");
    let outdir_path = temp_dir.path().join("out").to_path_buf();

    mkfifo(fifo.as_path());
    symlink(fifo.as_path(), alias.as_path()).unwrap();

    if let Some(deno_json) = self.deno_json {
      temp_dir.write(
        "deno.json",
        deno_json
          .replace("{alias_url}", alias.url_file().as_str())
          .replace("{source_url}", fifo.url_file().as_str()),
      );
    }

    let writer = FifoWriter::spawn(fifo.to_path_buf(), self.source);

    let mut command = context.new_command().arg("bundle");
    if self.outdir {
      command =
        command.arg(format!("--outdir={}", outdir_path.to_string_lossy()));
    }
    let child = command
      .args_vec(self.args.iter().copied())
      .stdout_piped()
      .stderr_piped()
      .spawn()
      .unwrap();

    let output = child.wait_with_output_and_timeout(BUNDLE_TIMEOUT).expect(
      "`deno bundle` hung reopening the drained FIFO \
       (regression of denoland/deno#36162)",
    );

    writer.finish();

    // Read the emitted bundles while the temp dir is still alive (it is deleted
    // when `context`/`temp_dir` drop at the end of this function).
    let mut bundles = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(&outdir_path) {
      for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("js") {
          bundles.insert(
            path.file_name().unwrap().to_string_lossy().into_owned(),
            std::fs::read_to_string(&path).unwrap(),
          );
        }
      }
    }

    (output, bundles)
  }
}

fn stdout_stderr(output: &std::process::Output) -> (String, String) {
  (
    util::strip_ansi_codes(std::str::from_utf8(&output.stdout).unwrap())
      .into_owned(),
    util::strip_ansi_codes(std::str::from_utf8(&output.stderr).unwrap())
      .into_owned(),
  )
}

// Regression test for denoland/deno#36162: `bundle --check` of a symlink to a
// FIFO must produce the bundle (from content read once) instead of hanging.
#[test]
fn bundle_check_symlink_to_fifo_no_hang() {
  let (output, _) = FifoBundle {
    source: "const x: number = 5;\nconsole.log(x);\n",
    args: &["--check", "alias.ts"],
    ..Default::default()
  }
  .run();
  let (stdout, stderr) = stdout_stderr(&output);

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
// strip the annotation and succeed, so a failing exit code with TS2322 here can
// only come from the check graph having read the piped bytes (an EOF/empty
// module would pass type-checking).
#[test]
fn bundle_check_symlink_to_fifo_type_error_from_check() {
  let (output, _) = FifoBundle {
    source: "const x: number = \"str\";\nconsole.log(x);\n",
    args: &["--check=all", "alias.ts"],
    ..Default::default()
  }
  .run();
  let (stdout, stderr) = stdout_stderr(&output);

  assert!(
    !output.status.success(),
    "expected type-check failure, got success\nstdout:\n{stdout}\nstderr:\n{stderr}",
  );
  assert_contains!(stderr, "TS2322");
  assert_contains!(stderr, "Type 'string' is not assignable to type 'number'");
}

// Regression test for the denoland/deno#36162 review follow-up: passing the same
// non-regular entrypoint twice must read the pipe only once and still produce a
// non-empty bundle, not a 0-byte file or a hang.
#[test]
fn bundle_check_dup_symlink_to_fifo_read_once() {
  let (output, bundles) = FifoBundle {
    source: "const x = 5;\nconsole.log(x);\n",
    args: &["--check", "alias.ts", "alias.ts"],
    outdir: true,
    ..Default::default()
  }
  .run();
  let (stdout, stderr) = stdout_stderr(&output);

  assert!(
    output.status.success(),
    "expected success, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    output.status.code()
  );
  // The bundled output must contain the piped source (not a 0-byte file from a
  // second EOF read overwriting the first).
  assert!(
    !bundles.is_empty(),
    "expected a bundled .js\nstdout:\n{stdout}\nstderr:\n{stderr}",
  );
  for (name, bundled) in &bundles {
    assert_contains!(bundled, "console.log(x)");
    assert!(!bundled.is_empty(), "{name} is empty");
  }
}

// Two *different* entrypoints that alias the same pipe (the symlink and its
// target) must also read it only once — deduping by resolved URL alone would
// miss this and the second `std::fs::read` would block on the drained FIFO
// (denoland/deno#36162 review follow-up).
#[test]
fn bundle_check_symlink_and_target_same_fifo_read_once() {
  let (output, bundles) = FifoBundle {
    source: "const x = 5;\nconsole.log(x);\n",
    args: &["--check", "alias.ts", "source.ts"],
    outdir: true,
    ..Default::default()
  }
  .run();
  let (stdout, stderr) = stdout_stderr(&output);

  assert!(
    output.status.success(),
    "expected success, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    output.status.code()
  );
  assert!(
    !bundles.is_empty(),
    "expected a bundled .js\nstdout:\n{stdout}\nstderr:\n{stderr}",
  );
  // Both aliases get the content that was read once, so neither bundle is the
  // empty result of a second (EOF) read.
  for (name, bundled) in &bundles {
    assert!(
      bundled.contains("console.log(x)"),
      "{name} is missing the piped source:\n{bundled}"
    );
  }
}

// The `--check` graph keys the pre-read pipe by the entrypoint as written,
// while the bundle graph keys it by `resolver.resolve()`'s output. An import map
// that remaps the entrypoint onto the same file (here the FIFO the symlink
// points at) must still hit the in-memory content — otherwise esbuild's loader
// reopens the drained FIFO and blocks (denoland/deno#36162 review follow-up).
#[test]
fn bundle_check_fifo_entrypoint_remapped_by_import_map() {
  let (output, _) = FifoBundle {
    source: "const x = 5;\nconsole.log(x);\n",
    args: &["--check", "alias.ts"],
    deno_json: Some(r#"{ "imports": { "{alias_url}": "{source_url}" } }"#),
    ..Default::default()
  }
  .run();
  let (stdout, stderr) = stdout_stderr(&output);

  assert!(
    output.status.success(),
    "expected success, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
    output.status.code()
  );
  assert_contains!(stdout, "console.log(x)");
}
