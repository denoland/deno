// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use std::time::Instant;
use test_util as util;
use test_util::itest;
use util::deno_config_path;
use util::env_vars_for_npm_tests;

util::unit_test_factory!(
  node_unit_test,
  "../tests/unit_node",
  "**/*_test.ts",
  [
    _fs_access_test = _fs / _fs_access_test,
    _fs_appendFile_test = _fs / _fs_appendFile_test,
    _fs_chmod_test = _fs / _fs_chmod_test,
    _fs_chown_test = _fs / _fs_chown_test,
    _fs_close_test = _fs / _fs_close_test,
    _fs_copy_test = _fs / _fs_copy_test,
    _fs_dir_test = _fs / _fs_dir_test,
    _fs_dirent_test = _fs / _fs_dirent_test,
    _fs_open_test = _fs / _fs_open_test,
    _fs_read_test = _fs / _fs_read_test,
    _fs_exists_test = _fs / _fs_exists_test,
    _fs_fdatasync_test = _fs / _fs_fdatasync_test,
    _fs_fstat_test = _fs / _fs_fstat_test,
    _fs_fsync_test = _fs / _fs_fsync_test,
    _fs_ftruncate_test = _fs / _fs_ftruncate_test,
    _fs_futimes_test = _fs / _fs_futimes_test,
    _fs_handle_test = _fs / _fs_handle_test,
    _fs_link_test = _fs / _fs_link_test,
    _fs_lstat_test = _fs / _fs_lstat_test,
    _fs_mkdir_test = _fs / _fs_mkdir_test,
    _fs_mkdtemp_test = _fs / _fs_mkdtemp_test,
    _fs_opendir_test = _fs / _fs_opendir_test,
    _fs_readFile_test = _fs / _fs_readFile_test,
    _fs_readdir_test = _fs / _fs_readdir_test,
    _fs_readlink_test = _fs / _fs_readlink_test,
    _fs_realpath_test = _fs / _fs_realpath_test,
    _fs_rename_test = _fs / _fs_rename_test,
    _fs_rm_test = _fs / _fs_rm_test,
    _fs_rmdir_test = _fs / _fs_rmdir_test,
    _fs_stat_test = _fs / _fs_stat_test,
    _fs_statfs_test = _fs / _fs_statfs_test,
    _fs_symlink_test = _fs / _fs_symlink_test,
    _fs_truncate_test = _fs / _fs_truncate_test,
    _fs_unlink_test = _fs / _fs_unlink_test,
    _fs_utimes_test = _fs / _fs_utimes_test,
    _fs_watch_test = _fs / _fs_watch_test,
    _fs_writeFile_test = _fs / _fs_writeFile_test,
    _fs_write_test = _fs / _fs_write_test,
    async_hooks_test,
    assert_test,
    assertion_error_test,
    buffer_test,
    child_process_test,
    console_test,
    crypto_cipher_gcm_test = crypto / crypto_cipher_gcm_test,
    crypto_cipher_test = crypto / crypto_cipher_test,
    crypto_hash_test = crypto / crypto_hash_test,
    crypto_hkdf_test = crypto / crypto_hkdf_test,
    crypto_key_test = crypto / crypto_key_test,
    crypto_misc_test = crypto / crypto_misc_test,
    crypto_pbkdf2_test = crypto / crypto_pbkdf2_test,
    crypto_scrypt_test = crypto / crypto_scrypt_test,
    crypto_sign_test = crypto / crypto_sign_test,
    events_test,
    dgram_test,
    domain_test,
    fs_test,
    fetch_test,
    http_test,
    http2_test,
    _randomBytes_test = internal / _randomBytes_test,
    _randomFill_test = internal / _randomFill_test,
    _randomInt_test = internal / _randomInt_test,
    module_test,
    net_test,
    os_test,
    path_test,
    perf_hooks_test,
    process_test,
    punycode_test,
    querystring_test,
    readline_test,
    repl_test,
    stream_test,
    string_decoder_test,
    timers_test,
    tls_test,
    tty_test,
    util_test,
    v8_test,
    vm_test,
    worker_threads_test,
    zlib_test
  ]
);

fn node_unit_test(test: String) {
  let _g = util::http_server();

  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--config")
    .arg(deno_config_path())
    .arg("--no-lock")
    .arg("--unstable")
    // TODO(kt3k): This option is required to pass tls_test.ts,
    // but this shouldn't be necessary. tls.connect currently doesn't
    // pass hostname option correctly and it causes cert errors.
    .arg("--unsafely-ignore-certificate-errors")
    .arg("-A");
  // Parallel tests for crypto
  if test.starts_with("crypto/") {
    deno = deno.arg("--parallel");
  }
  let mut deno = deno
    .arg(
      util::tests_path()
        .join("unit_node")
        .join(format!("{test}.ts")),
    )
    .envs(env_vars_for_npm_tests())
    .piped_output()
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
      panic!("Test {test} failed to complete in time");
    }
    if let Some(status) = deno
      .try_wait()
      .expect("failed to wait for the child process")
    {
      break status;
    }
    std::thread::sleep(Duration::from_millis(100));
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

// Regression test for https://github.com/denoland/deno/issues/16928
itest!(unhandled_rejection_web {
  args: "run -A node/unhandled_rejection_web.ts",
  output: "node/unhandled_rejection_web.ts.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
});

// Ensure that Web `onunhandledrejection` is fired before
// Node's `process.on('unhandledRejection')`.
itest!(unhandled_rejection_web_process {
  args: "run -A node/unhandled_rejection_web_process.ts",
  output: "node/unhandled_rejection_web_process.ts.out",
  envs: env_vars_for_npm_tests(),
  http_server: true,
});
