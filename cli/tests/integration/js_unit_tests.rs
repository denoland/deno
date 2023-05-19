// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
use std::io::BufRead;
use std::io::BufReader;
use std::process::Stdio;
use test_util as util;

#[test]
fn js_unit_tests_lint() {
  let status = util::deno_cmd()
    .arg("lint")
    .arg("--unstable")
    .arg(util::tests_path().join("unit"))
    .spawn()
    .unwrap()
    .wait()
    .unwrap();
  assert!(status.success());
}

util::unit_test_factory!(
  js_unit_test,
  "cli/tests/unit/*.ts",
  [
    abort_controller_test,
    blob_test,
    body_test,
    broadcast_channel_test,
    buffer_test,
    build_test,
    cache_api_test,
    chmod_test,
    chown_test,
    command_test,
    console_test,
    copy_file_test,
    custom_event_test,
    dir_test,
    dom_exception_test,
    error_stack_test,
    error_test,
    esnext_test,
    event_target_test,
    event_test,
    fetch_test,
    ffi_test,
    file_test,
    filereader_test,
    files_test,
    flock_test,
    fs_events_test,
    get_random_values_test,
    globals_test,
    headers_test,
    http_test,
    internals_test,
    intl_test,
    io_test,
    kv_test,
    link_test,
    make_temp_test,
    message_channel_test,
    metrics_test,
    mkdir_test,
    navigator_test,
    net_test,
    network_interfaces_test,
    opcall_test,
    os_test,
    path_from_url_test,
    performance_test,
    permissions_test,
    process_test,
    progressevent_test,
    promise_hooks_test,
    read_dir_test,
    read_file_test,
    read_link_test,
    read_text_file_test,
    real_path_test,
    ref_unref_test,
    remove_test,
    rename_test,
    request_test,
    resources_test,
    response_test,
    serve_test,
    signal_test,
    stat_test,
    stdio_test,
    structured_clone_test,
    symlink_test,
    sync_test,
    test_util,
    testing_test,
    text_encoding_test,
    timers_test,
    tls_test,
    truncate_test,
    tty_color_test,
    tty_test,
    umask_test,
    url_search_params_test,
    url_test,
    urlpattern_test,
    utime_test,
    version_test,
    wasm_test,
    webcrypto_test,
    websocket_test,
    webstorage_test,
    worker_permissions_test,
    worker_types,
    write_file_test,
    write_text_file_test,
  ]
);

fn js_unit_test(test: &'static str) {
  let _g = util::http_server();

  // Note that the unit tests are not safe for concurrency and must be run with a concurrency limit
  // of one because there are some chdir tests in there.
  // TODO(caspervonb) split these tests into two groups: parallel and serial.
  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--unstable")
    .arg("--location=http://js-unit-tests/foo/bar")
    .arg("--no-prompt")
    .arg("-A")
    .arg(util::tests_path().join("unit").join(format!("{test}.ts")))
    .stderr(Stdio::piped())
    .stdout(Stdio::piped())
    .spawn()
    .expect("failed to spawn script");

  let stdout = deno.stdout.take().unwrap();
  let stdout = std::thread::spawn(move || {
    let reader = BufReader::new(stdout);
    for line in reader.lines() {
      if let Ok(line) = line {
        eprintln!("[{test}] {line}");
      } else {
        break;
      }
    }
  });

  let stderr = deno.stderr.take().unwrap();
  let stderr = std::thread::spawn(move || {
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
      if let Ok(line) = line {
        eprintln!("[{test}] {line}");
      } else {
        break;
      }
    }
  });

  let status = deno.wait().expect("failed to wait for the child process");
  #[cfg(unix)]
  assert_eq!(
    std::os::unix::process::ExitStatusExt::signal(&status),
    None,
    "Deno should not have died with a signal"
  );
  assert_eq!(Some(0), status.code(), "Deno should have exited cleanly");
  assert!(status.success());
}
