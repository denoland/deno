// Copyright 2018-2025 the Deno authors. MIT license.

use std::io::BufRead;
use std::io::BufReader;
use std::time::Duration;
use std::time::Instant;

use test_util as util;

util::unit_test_factory!(
  js_unit_test,
  "../tests/unit",
  "*.ts",
  [
    abort_controller_test,
    blob_test,
    body_test,
    broadcast_channel_test,
    build_test,
    cache_api_test,
    chmod_test,
    chown_test,
    command_test,
    console_test,
    copy_file_test,
    custom_event_test,
    cron_test,
    dir_test,
    dom_exception_test,
    error_stack_test,
    error_test,
    esnext_test,
    event_source_test,
    event_target_test,
    event_test,
    fetch_test,
    ffi_test,
    file_test,
    filereader_test,
    files_test,
    fs_events_test,
    get_random_values_test,
    globals_test,
    headers_test,
    http_test,
    image_bitmap_test,
    image_data_test,
    internals_test,
    intl_test,
    jupyter_test,
    kv_test,
    kv_queue_test_no_db_close,
    kv_queue_test,
    kv_queue_undelivered_test,
    link_test,
    lint_selectors_test,
    lint_plugin_test,
    make_temp_test,
    message_channel_test,
    mkdir_test,
    navigator_test,
    net_test,
    network_interfaces_test,
    os_test,
    ops_test,
    path_from_url_test,
    performance_test,
    permissions_test,
    process_test,
    progressevent_test,
    promise_hooks_test,
    quic_test,
    read_dir_test,
    read_file_test,
    read_link_test,
    read_text_file_test,
    real_path_test,
    ref_unref_test,
    remove_test,
    rename_test,
    request_test,
    response_test,
    serve_test,
    signal_test,
    stat_test,
    stdio_test,
    streams_test,
    structured_clone_test,
    symbol_test,
    symlink_test,
    test_util,
    testing_test,
    text_encoding_test,
    timers_test,
    tls_test,
    tls_sni_test,
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
    webgpu_test,
    websocket_test,
    webstorage_test,
    worker_permissions_test,
    worker_test,
    write_file_test,
    write_text_file_test,
  ]
);

fn js_unit_test(test: String) {
  let _g = util::http_server();

  let mut deno = util::deno_cmd()
    .current_dir(util::root_path())
    .arg("test")
    .arg("--config")
    .arg(util::deno_config_path())
    .arg("--no-lock")
    // TODO(bartlomieju): would be better if we could apply this unstable
    // flag to particular files, but there's many of them that rely on unstable
    // net APIs (`reusePort` in `listen` and `listenTls`; `listenDatagram`)
    .arg("--unstable-net")
    .arg("--unstable-vsock")
    .arg("--location=http://127.0.0.1:4545/")
    .arg("--no-prompt");

  if test == "broadcast_channel_test" {
    deno = deno.arg("--unstable-broadcast-channel");
  }

  if test == "cron_test" {
    deno = deno.arg("--unstable-cron");
  }

  if test.contains("kv_") {
    deno = deno.arg("--unstable-kv");
  }

  if test == "worker_permissions_test" || test == "worker_test" {
    deno = deno.arg("--unstable-worker-options");
  }

  // Some tests require the root CA cert file to be loaded.
  if test == "websocket_test" {
    deno = deno.arg(format!(
      "--cert={}",
      util::testdata_path()
        .join("tls")
        .join("RootCA.pem")
        .to_string_lossy()
    ));
  };

  if test == "tls_sni_test" {
    // TODO(lucacasonato): fix the SNI in the certs so that this is not needed
    deno = deno.arg("--unsafely-ignore-certificate-errors");
  }

  let mut deno = deno
    .arg("-A")
    .arg(util::tests_path().join("unit").join(format!("{test}.ts")));

  // update the snapshots if when `UPDATE=1`
  if std::env::var_os("UPDATE") == Some("1".into()) {
    deno = deno.arg("--").arg("--update");
  }

  let mut deno = deno.piped_output().spawn().expect("failed to spawn script");

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

  const PER_TEST_TIMEOUT: Duration = Duration::from_secs(3 * 60);

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
