// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use flaky_test::flaky_test;
use std::io::BufRead;
use tempfile::TempDir;
use test_util as util;

// Helper function to skip watcher output that contains "Restarting"
// phrase.
fn skip_restarting_line(
  mut stderr_lines: impl Iterator<Item = String>,
) -> String {
  loop {
    let msg = stderr_lines.next().unwrap();
    if !msg.contains("Restarting") {
      return msg;
    }
  }
}

/// Helper function to skip watcher output that doesn't contain
/// "{job_name} finished" phrase.
fn wait_for_process_finished(
  job_name: &str,
  stderr_lines: &mut impl Iterator<Item = String>,
) {
  let phrase = format!("{} finished", job_name);
  loop {
    let msg = stderr_lines.next().unwrap();
    if msg.contains(&phrase) {
      break;
    }
  }
}

/// Helper function to skip watcher output that doesn't contain
/// "{job_name} failed" phrase.
fn wait_for_process_failed(
  job_name: &str,
  stderr_lines: &mut impl Iterator<Item = String>,
) {
  let phrase = format!("{} failed", job_name);
  loop {
    let msg = stderr_lines.next().unwrap();
    if msg.contains(&phrase) {
      break;
    }
  }
}

#[test]
fn fmt_watch_test() {
  let t = TempDir::new().expect("tempdir fail");
  let fixed = util::testdata_path().join("badly_formatted_fixed.js");
  let badly_formatted_original =
    util::testdata_path().join("badly_formatted.mjs");
  let badly_formatted = t.path().join("badly_formatted.js");
  std::fs::copy(&badly_formatted_original, &badly_formatted)
    .expect("Failed to copy file");

  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("fmt")
    .arg(&badly_formatted)
    .arg("--watch")
    .arg("--unstable")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("Failed to spawn script");
  let stderr = child.stderr.as_mut().unwrap();
  let stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  // TODO(lucacasonato): remove this timeout. It seems to be needed on Linux.
  std::thread::sleep(std::time::Duration::from_secs(1));

  assert!(skip_restarting_line(stderr_lines).contains("badly_formatted.js"));

  let expected = std::fs::read_to_string(fixed.clone()).unwrap();
  let actual = std::fs::read_to_string(badly_formatted.clone()).unwrap();
  assert_eq!(expected, actual);

  // Change content of the file again to be badly formatted
  std::fs::copy(&badly_formatted_original, &badly_formatted)
    .expect("Failed to copy file");
  std::thread::sleep(std::time::Duration::from_secs(1));

  // Check if file has been automatically formatted by watcher
  let expected = std::fs::read_to_string(fixed).unwrap();
  let actual = std::fs::read_to_string(badly_formatted).unwrap();
  assert_eq!(expected, actual);

  // the watcher process is still alive
  assert!(child.try_wait().unwrap().is_none());

  child.kill().unwrap();
  drop(t);
}

#[test]
fn bundle_js_watch() {
  use std::path::PathBuf;
  // Test strategy extends this of test bundle_js by adding watcher
  let t = TempDir::new().expect("tempdir fail");
  let file_to_watch = t.path().join("file_to_watch.js");
  std::fs::write(&file_to_watch, "console.log('Hello world');")
    .expect("error writing file");
  assert!(file_to_watch.is_file());
  let t = TempDir::new().expect("tempdir fail");
  let bundle = t.path().join("mod6.bundle.js");
  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(&file_to_watch)
    .arg(&bundle)
    .arg("--watch")
    .arg("--unstable")
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stderr = deno.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("file_to_watch.js"));
  assert!(stderr_lines.next().unwrap().contains("mod6.bundle.js"));
  let file = PathBuf::from(&bundle);
  assert!(file.is_file());
  wait_for_process_finished("Bundle", &mut stderr_lines);

  std::fs::write(&file_to_watch, "console.log('Hello world2');")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines
    .next()
    .unwrap()
    .contains("File change detected!"));
  assert!(stderr_lines.next().unwrap().contains("file_to_watch.js"));
  assert!(stderr_lines.next().unwrap().contains("mod6.bundle.js"));
  let file = PathBuf::from(&bundle);
  assert!(file.is_file());
  wait_for_process_finished("Bundle", &mut stderr_lines);

  // Confirm that the watcher keeps on working even if the file is updated and has invalid syntax
  std::fs::write(&file_to_watch, "syntax error ^^")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines
    .next()
    .unwrap()
    .contains("File change detected!"));
  assert!(stderr_lines.next().unwrap().contains("error: "));
  wait_for_process_failed("Bundle", &mut stderr_lines);

  // the watcher process is still alive
  assert!(deno.try_wait().unwrap().is_none());

  deno.kill().unwrap();
  drop(t);
}

/// Confirm that the watcher continues to work even if module resolution fails at the *first* attempt
#[test]
fn bundle_watch_not_exit() {
  let t = TempDir::new().expect("tempdir fail");
  let file_to_watch = t.path().join("file_to_watch.js");
  std::fs::write(&file_to_watch, "syntax error ^^")
    .expect("error writing file");
  let target_file = t.path().join("target.js");

  let mut deno = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("bundle")
    .arg(&file_to_watch)
    .arg(&target_file)
    .arg("--watch")
    .arg("--unstable")
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stderr = deno.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("error:"));
  assert!(stderr_lines.next().unwrap().contains("Bundle failed"));
  // the target file hasn't been created yet
  assert!(!target_file.is_file());

  // Make sure the watcher actually restarts and works fine with the proper syntax
  std::fs::write(&file_to_watch, "console.log(42);")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines
    .next()
    .unwrap()
    .contains("File change detected!"));
  assert!(stderr_lines.next().unwrap().contains("file_to_watch.js"));
  assert!(stderr_lines.next().unwrap().contains("target.js"));
  wait_for_process_finished("Bundle", &mut stderr_lines);
  // bundled file is created
  assert!(target_file.is_file());

  // the watcher process is still alive
  assert!(deno.try_wait().unwrap().is_none());

  deno.kill().unwrap();
  drop(t);
}

#[flaky_test::flaky_test]
fn run_watch() {
  let t = TempDir::new().expect("tempdir fail");
  let file_to_watch = t.path().join("file_to_watch.js");
  std::fs::write(&file_to_watch, "console.log('Hello world');")
    .expect("error writing file");

  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--watch")
    .arg("--unstable")
    .arg(&file_to_watch)
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  assert!(stdout_lines.next().unwrap().contains("Hello world"));
  wait_for_process_finished("Process", &mut stderr_lines);

  // TODO(lucacasonato): remove this timeout. It seems to be needed on Linux.
  std::thread::sleep(std::time::Duration::from_secs(1));

  // Change content of the file
  std::fs::write(&file_to_watch, "console.log('Hello world2');")
    .expect("error writing file");
  // Events from the file watcher is "debounced", so we need to wait for the next execution to start
  std::thread::sleep(std::time::Duration::from_secs(1));

  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stdout_lines.next().unwrap().contains("Hello world2"));
  wait_for_process_finished("Process", &mut stderr_lines);

  // Add dependency
  let another_file = t.path().join("another_file.js");
  std::fs::write(&another_file, "export const foo = 0;")
    .expect("error writing file");
  std::fs::write(
    &file_to_watch,
    "import { foo } from './another_file.js'; console.log(foo);",
  )
  .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stdout_lines.next().unwrap().contains('0'));
  wait_for_process_finished("Process", &mut stderr_lines);

  // Confirm that restarting occurs when a new file is updated
  std::fs::write(&another_file, "export const foo = 42;")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stdout_lines.next().unwrap().contains("42"));
  wait_for_process_finished("Process", &mut stderr_lines);

  // Confirm that the watcher keeps on working even if the file is updated and has invalid syntax
  std::fs::write(&file_to_watch, "syntax error ^^")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stderr_lines.next().unwrap().contains("error:"));
  wait_for_process_failed("Process", &mut stderr_lines);

  // Then restore the file
  std::fs::write(
    &file_to_watch,
    "import { foo } from './another_file.js'; console.log(foo);",
  )
  .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stdout_lines.next().unwrap().contains("42"));
  wait_for_process_finished("Process", &mut stderr_lines);

  // Update the content of the imported file with invalid syntax
  std::fs::write(&another_file, "syntax error ^^").expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stderr_lines.next().unwrap().contains("error:"));
  wait_for_process_failed("Process", &mut stderr_lines);

  // Modify the imported file and make sure that restarting occurs
  std::fs::write(&another_file, "export const foo = 'modified!';")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stdout_lines.next().unwrap().contains("modified!"));
  wait_for_process_finished("Process", &mut stderr_lines);

  // the watcher process is still alive
  assert!(child.try_wait().unwrap().is_none());

  child.kill().unwrap();
  drop(t);
}

/// Confirm that the watcher continues to work even if module resolution fails at the *first* attempt
#[test]
fn run_watch_not_exit() {
  let t = TempDir::new().expect("tempdir fail");
  let file_to_watch = t.path().join("file_to_watch.js");
  std::fs::write(&file_to_watch, "syntax error ^^")
    .expect("error writing file");

  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--watch")
    .arg("--unstable")
    .arg(&file_to_watch)
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("error:"));
  assert!(stderr_lines.next().unwrap().contains("Process failed"));

  // Make sure the watcher actually restarts and works fine with the proper syntax
  std::fs::write(&file_to_watch, "console.log(42);")
    .expect("error writing file");
  std::thread::sleep(std::time::Duration::from_secs(1));
  assert!(stderr_lines.next().unwrap().contains("Restarting"));
  assert!(stdout_lines.next().unwrap().contains("42"));
  wait_for_process_finished("Process", &mut stderr_lines);

  // the watcher process is still alive
  assert!(child.try_wait().unwrap().is_none());

  child.kill().unwrap();
  drop(t);
}

#[test]
fn run_watch_with_import_map_and_relative_paths() {
  fn create_relative_tmp_file(
    directory: &TempDir,
    filename: &'static str,
    filecontent: &'static str,
  ) -> std::path::PathBuf {
    let absolute_path = directory.path().join(filename);
    std::fs::write(&absolute_path, filecontent).expect("error writing file");
    let relative_path = absolute_path
      .strip_prefix(util::testdata_path())
      .expect("unable to create relative temporary file")
      .to_owned();
    assert!(relative_path.is_relative());
    relative_path
  }
  let temp_directory =
    TempDir::new_in(util::testdata_path()).expect("tempdir fail");
  let file_to_watch = create_relative_tmp_file(
    &temp_directory,
    "file_to_watch.js",
    "console.log('Hello world');",
  );
  let import_map_path = create_relative_tmp_file(
    &temp_directory,
    "import_map.json",
    "{\"imports\": {}}",
  );

  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("run")
    .arg("--unstable")
    .arg("--watch")
    .arg("--import-map")
    .arg(&import_map_path)
    .arg(&file_to_watch)
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  assert!(stderr_lines.next().unwrap().contains("Process finished"));
  assert!(stdout_lines.next().unwrap().contains("Hello world"));

  child.kill().unwrap();

  drop(file_to_watch);
  drop(import_map_path);
  temp_directory.close().unwrap();
}

#[flaky_test]
fn test_watch() {
  macro_rules! assert_contains {
        ($string:expr, $($test:expr),+) => {
          let string = $string; // This might be a function call or something
          if !($(string.contains($test))||+) {
            panic!("{:?} does not contain any of {:?}", string, [$($test),+]);
          }
        }
      }

  let t = TempDir::new().expect("tempdir fail");

  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--watch")
    .arg("--unstable")
    .arg("--no-check")
    .arg(&t.path())
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  assert_eq!(stdout_lines.next().unwrap(), "");
  assert_contains!(
    stdout_lines.next().unwrap(),
    "0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
  );
  wait_for_process_finished("Test", &mut stderr_lines);

  let foo_file = t.path().join("foo.js");
  let bar_file = t.path().join("bar.js");
  let foo_test = t.path().join("foo_test.js");
  let bar_test = t.path().join("bar_test.js");
  std::fs::write(&foo_file, "export default function foo() { 1 + 1 }")
    .expect("error writing file");
  std::fs::write(&bar_file, "export default function bar() { 2 + 2 }")
    .expect("error writing file");
  std::fs::write(
    &foo_test,
    "import foo from './foo.js'; Deno.test('foo', foo);",
  )
  .expect("error writing file");
  std::fs::write(
    &bar_test,
    "import bar from './bar.js'; Deno.test('bar', bar);",
  )
  .expect("error writing file");

  assert_eq!(stdout_lines.next().unwrap(), "");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "foo", "bar");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "foo", "bar");
  stdout_lines.next();
  stdout_lines.next();
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Change content of the file
  std::fs::write(
    &foo_test,
    "import foo from './foo.js'; Deno.test('foobar', foo);",
  )
  .expect("error writing file");

  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "foobar");
  stdout_lines.next();
  stdout_lines.next();
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Add test
  let another_test = t.path().join("new_test.js");
  std::fs::write(&another_test, "Deno.test('another one', () => 3 + 3)")
    .expect("error writing file");
  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "another one");
  stdout_lines.next();
  stdout_lines.next();
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Confirm that restarting occurs when a new file is updated
  std::fs::write(&another_test, "Deno.test('another one', () => 3 + 3); Deno.test('another another one', () => 4 + 4)")
    .expect("error writing file");
  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stdout_lines.next().unwrap(), "running 2 tests");
  assert_contains!(stdout_lines.next().unwrap(), "another one");
  assert_contains!(stdout_lines.next().unwrap(), "another another one");
  stdout_lines.next();
  stdout_lines.next();
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Confirm that the watcher keeps on working even if the file is updated and has invalid syntax
  std::fs::write(&another_test, "syntax error ^^").expect("error writing file");
  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stderr_lines.next().unwrap(), "error:");
  assert_contains!(stderr_lines.next().unwrap(), "Test failed");

  // Then restore the file
  std::fs::write(&another_test, "Deno.test('another one', () => 3 + 3)")
    .expect("error writing file");
  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "another one");
  stdout_lines.next();
  stdout_lines.next();
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Confirm that the watcher keeps on working even if the file is updated and the test fails
  // This also confirms that it restarts when dependencies change
  std::fs::write(
    &foo_file,
    "export default function foo() { throw new Error('Whoops!'); }",
  )
  .expect("error writing file");
  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "FAILED");
  while !stdout_lines.next().unwrap().contains("test result") {}
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Then restore the file
  std::fs::write(&foo_file, "export default function foo() { 1 + 1 }")
    .expect("error writing file");
  assert_contains!(stderr_lines.next().unwrap(), "Restarting");
  assert_contains!(stdout_lines.next().unwrap(), "running 1 test");
  assert_contains!(stdout_lines.next().unwrap(), "foo");
  stdout_lines.next();
  stdout_lines.next();
  stdout_lines.next();
  wait_for_process_finished("Test", &mut stderr_lines);

  // Test that circular dependencies work fine
  std::fs::write(
    &foo_file,
    "import './bar.js'; export default function foo() { 1 + 1 }",
  )
  .expect("error writing file");
  std::fs::write(
    &bar_file,
    "import './foo.js'; export default function bar() { 2 + 2 }",
  )
  .expect("error writing file");

  // the watcher process is still alive
  assert!(child.try_wait().unwrap().is_none());

  child.kill().unwrap();
  drop(t);
}

#[flaky_test]
fn test_watch_doc() {
  macro_rules! assert_contains {
        ($string:expr, $($test:expr),+) => {
          let string = $string; // This might be a function call or something
          if !($(string.contains($test))||+) {
            panic!("{:?} does not contain any of {:?}", string, [$($test),+]);
          }
        }
      }

  let t = TempDir::new().expect("tempdir fail");

  let mut child = util::deno_cmd()
    .current_dir(util::testdata_path())
    .arg("test")
    .arg("--watch")
    .arg("--doc")
    .arg("--unstable")
    .arg(&t.path())
    .env("NO_COLOR", "1")
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stdout = child.stdout.as_mut().unwrap();
  let mut stdout_lines =
    std::io::BufReader::new(stdout).lines().map(|r| r.unwrap());
  let stderr = child.stderr.as_mut().unwrap();
  let mut stderr_lines =
    std::io::BufReader::new(stderr).lines().map(|r| r.unwrap());

  assert_eq!(stdout_lines.next().unwrap(), "");
  assert_contains!(
    stdout_lines.next().unwrap(),
    "0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
  );
  wait_for_process_finished("Test", &mut stderr_lines);

  let foo_file = t.path().join("foo.ts");
  std::fs::write(
    &foo_file,
    r#"
    export default function foo() {}
  "#,
  )
  .expect("error writing file");

  std::fs::write(
    &foo_file,
    r#"
    /**
     * ```ts
     * import foo from "./foo.ts";
     * ```
     */
    export default function foo() {}
  "#,
  )
  .expect("error writing file");

  // We only need to scan for a Check file://.../foo.ts$3-6 line that
  // corresponds to the documentation block being type-checked.
  assert_contains!(skip_restarting_line(stderr_lines), "foo.ts$3-6");

  assert!(child.try_wait().unwrap().is_none());
  child.kill().unwrap();

  drop(t);
}
