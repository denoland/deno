// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::io::BufRead;
use std::io::BufReader;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;
use test_util as util;

util::unit_test_factory!(
  node_unit_test,
  "tests/unit_node",
  "**/*_test.ts",
  [
    _fs_DIR__fs_access_test,
    _fs_DIR__fs_appendFile_test,
    _fs_DIR__fs_chmod_test,
    _fs_DIR__fs_chown_test,
    _fs_DIR__fs_close_test,
    _fs_DIR__fs_copy_test,
    _fs_DIR__fs_dir_test,
    _fs_DIR__fs_exists_test,
    _fs_DIR__fs_fdatasync_test,
    _fs_DIR__fs_fstat_test,
    _fs_DIR__fs_fsync_test,
    _fs_DIR__fs_ftruncate_test,
    _fs_DIR__fs_futimes_test,
    _fs_DIR__fs_link_test,
    _fs_DIR__fs_lstat_test,
    _fs_DIR__fs_mkdir_test,
    _fs_DIR__fs_mkdtemp_test,
    _fs_DIR__fs_opendir_test,
    _fs_DIR__fs_readFile_test,
    _fs_DIR__fs_readdir_test,
    _fs_DIR__fs_readlink_test,
    _fs_DIR__fs_realpath_test,
    _fs_DIR__fs_rename_test,
    _fs_DIR__fs_rm_test,
    _fs_DIR__fs_rmdir_test,
    _fs_DIR__fs_stat_test,
    _fs_DIR__fs_symlink_test,
    _fs_DIR__fs_truncate_test,
    _fs_DIR__fs_unlink_test,
    _fs_DIR__fs_utimes_test,
    _fs_DIR__fs_watch_test,
    _fs_DIR__fs_write_test,
    async_hooks_test,
    child_process_test,
    crypto_cipher_test,
    crypto_hash_test,
    crypto_key_test,
    crypto_sign_test,
    fs_test,
    http_test,
    internal_DIR__randomBytes_test,
    internal_DIR__randomFill_test,
    internal_DIR__randomInt_test,
    internal_DIR_pbkdf2_test,
    internal_DIR_scrypt_test,
    module_test,
    process_test,
    querystring_test,
    readline_test,
    string_decoder_test,
    timers_test,
    tls_test,
    tty_test,
    util_test,
    v8_test,
    worker_threads_test
  ]
);

fn node_unit_test(test: String) {
  let _g = util::http_server();

  // Note that the unit tests are not safe for concurrency and must be run with a concurrency limit
  // of one because there are some chdir tests in there.
  // TODO(caspervonb) split these tests into two groups: parallel and serial.
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--unstable")
    // TODO(kt3k): This option is required to pass tls_test.ts,
    // but this shouldn't be necessary. tls.connect currently doesn't
    // pass hostname option correctly and it causes cert errors.
    .arg("--unsafely-ignore-certificate-errors")
    .arg("-A")
    .arg(
      util::tests_path()
        .join("unit_node")
        .join(format!("{test}.ts")),
    )
    .stderr(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let now = Instant::now();
  let stdout = deno.stdout.take().unwrap();
  let test_name = test.clone();
  let stdout = std::thread::spawn(move || {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
      if let Ok(line) = line {
        println!("[{test_name} {:0>6.2}] {line}", now.elapsed().as_secs_f32());
      } else {
        break;
      }
    }
  });

  let now = Instant::now();
  let stderr = deno.stderr.take().unwrap();
  let test_name = test.clone();
  let stderr = std::thread::spawn(move || {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
      if let Ok(line) = line {
        eprintln!("[{test_name} {:0>6.2}] {line}", now.elapsed().as_secs_f32());
      } else {
        break;
      }
    }
  });

  const PER_TEST_TIMEOUT: Duration = Duration::from_secs(5 * 60);

  let now = Instant::now();
  let status = loop {
    if now.elapsed() > PER_TEST_TIMEOUT {
      // Last-ditch kill
      _ = deno.kill();
      panic!("Test failed to complete in time");
    }
    if let Some(status) = deno
      .try_wait()
      .expect("failed to wait for the child process")
    {
      break status;
    }
  };

  #[cfg(unix)]
  assert_eq!(
    std::os::unix::process::ExitStatusExt::signal(&status),
    None,
    "Deno should not have died with a signal"
  );
  assert_eq!(Some(0), status.code(), "Deno should have exited cleanly");

  stdout.join().unwrap();
  stderr.join().unwrap();

  assert!(status.success());
}
