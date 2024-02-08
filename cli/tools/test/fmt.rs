// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::stats::RuntimeActivity;
use deno_core::stats::RuntimeActivityDiff;
use deno_core::stats::RuntimeActivityType;
use phf::phf_map;
use std::borrow::Cow;
use std::ops::AddAssign;

use super::*;

pub fn to_relative_path_or_remote_url(cwd: &Url, path_or_url: &str) -> String {
  let Ok(url) = Url::parse(path_or_url) else {
    return "<anonymous>".to_string();
  };
  if url.scheme() == "file" {
    if let Some(mut r) = cwd.make_relative(&url) {
      if !r.starts_with("../") {
        r = format!("./{r}");
      }
      return r;
    }
  }
  path_or_url.to_string()
}

fn abbreviate_test_error(js_error: &JsError) -> JsError {
  let mut js_error = js_error.clone();
  let frames = std::mem::take(&mut js_error.frames);

  // check if there are any stack frames coming from user code
  let should_filter = frames.iter().any(|f| {
    if let Some(file_name) = &f.file_name {
      !(file_name.starts_with("[ext:") || file_name.starts_with("ext:"))
    } else {
      true
    }
  });

  if should_filter {
    let mut frames = frames
      .into_iter()
      .rev()
      .skip_while(|f| {
        if let Some(file_name) = &f.file_name {
          file_name.starts_with("[ext:") || file_name.starts_with("ext:")
        } else {
          false
        }
      })
      .collect::<Vec<_>>();
    frames.reverse();
    js_error.frames = frames;
  } else {
    js_error.frames = frames;
  }

  js_error.cause = js_error
    .cause
    .as_ref()
    .map(|e| Box::new(abbreviate_test_error(e)));
  js_error.aggregated = js_error
    .aggregated
    .as_ref()
    .map(|es| es.iter().map(abbreviate_test_error).collect());
  js_error
}

// This function prettifies `JsError` and applies some changes specifically for
// test runner purposes:
//
// - filter out stack frames:
//   - if stack trace consists of mixed user and internal code, the frames
//     below the first user code frame are filtered out
//   - if stack trace consists only of internal code it is preserved as is
pub fn format_test_error(js_error: &JsError) -> String {
  let mut js_error = abbreviate_test_error(js_error);
  js_error.exception_message = js_error
    .exception_message
    .trim_start_matches("Uncaught ")
    .to_string();
  format_js_error(&js_error)
}

pub fn format_sanitizer_diff(diff: RuntimeActivityDiff) -> Vec<String> {
  let mut output = format_sanitizer_accum(diff.appeared, true);
  output.extend(format_sanitizer_accum(diff.disappeared, false));
  output.sort();
  output
}

fn format_sanitizer_accum(
  activities: Vec<RuntimeActivity>,
  appeared: bool,
) -> Vec<String> {
  let mut accum = HashMap::new();
  for activity in activities {
    let item = format_sanitizer_accum_item(activity);
    accum.entry(item).or_insert(0).add_assign(1);
  }

  let mut output = vec![];
  for ((item_type, item_name), count) in accum.into_iter() {
    if item_type == RuntimeActivityType::Resource {
      // TODO(mmastrac): until we implement the new timers and op sanitization, these must be ignored in this path
      if item_name == "timer" {
        continue;
      }
      let (name, action1, action2) = pretty_resource_name(&item_name);
      let hint = resource_close_hint(&item_name);

      if appeared {
        output.push(format!("{name} was {action1} during the test, but not {action2} during the test. {hint}"));
      } else {
        output.push(format!("{name} was {action1} before the test started, but was {action2} during the test. \
          Do not close resources in a test that were not created during that test."));
      }
    } else {
      // TODO(mmastrac): this will be done in a later PR
      unimplemented!(
        "Unhandled diff: {appeared} {} {:?} {}",
        count,
        item_type,
        item_name
      );
    }
  }
  output
}

fn format_sanitizer_accum_item(
  activity: RuntimeActivity,
) -> (RuntimeActivityType, Cow<'static, str>) {
  let activity_type = activity.activity();
  match activity {
    RuntimeActivity::AsyncOp(_, name) => (activity_type, name.into()),
    RuntimeActivity::Interval(_) => (activity_type, "".into()),
    RuntimeActivity::Resource(_, name) => (activity_type, name.into()),
    RuntimeActivity::Timer(_) => (activity_type, "".into()),
  }
}

fn pretty_resource_name(
  name: &str,
) -> (Cow<'static, str>, &'static str, &'static str) {
  let (name, action1, action2) = match name {
    "fsFile" => ("A file", "opened", "closed"),
    "fetchRequest" => ("A fetch request", "started", "finished"),
    "fetchRequestBody" => ("A fetch request body", "created", "closed"),
    "fetchResponse" => ("A fetch response body", "created", "consumed"),
    "httpClient" => ("An HTTP client", "created", "closed"),
    "dynamicLibrary" => ("A dynamic library", "loaded", "unloaded"),
    "httpConn" => ("An inbound HTTP connection", "accepted", "closed"),
    "httpStream" => ("An inbound HTTP request", "accepted", "closed"),
    "tcpStream" => ("A TCP connection", "opened/accepted", "closed"),
    "unixStream" => ("A Unix connection", "opened/accepted", "closed"),
    "tlsStream" => ("A TLS connection", "opened/accepted", "closed"),
    "tlsListener" => ("A TLS listener", "opened", "closed"),
    "unixListener" => ("A Unix listener", "opened", "closed"),
    "unixDatagram" => ("A Unix datagram", "opened", "closed"),
    "tcpListener" => ("A TCP listener", "opened", "closed"),
    "udpSocket" => ("A UDP socket", "opened", "closed"),
    "timer" => ("A timer", "started", "fired/cleared"),
    "textDecoder" => ("A text decoder", "created", "finished"),
    "messagePort" => ("A message port", "created", "closed"),
    "webSocketStream" => ("A WebSocket", "opened", "closed"),
    "fsEvents" => ("A file system watcher", "created", "closed"),
    "childStdin" => ("A child process stdin", "opened", "closed"),
    "childStdout" => ("A child process stdout", "opened", "closed"),
    "childStderr" => ("A child process stderr", "opened", "closed"),
    "child" => ("A child process", "started", "closed"),
    "signal" => ("A signal listener", "created", "fired/cleared"),
    "stdin" => ("The stdin pipe", "opened", "closed"),
    "stdout" => ("The stdout pipe", "opened", "closed"),
    "stderr" => ("The stderr pipe", "opened", "closed"),
    "compression" => ("A CompressionStream", "created", "closed"),
    _ => return (format!("\"{name}\"").into(), "created", "cleaned up"),
  };
  (name.into(), action1, action2)
}

fn resource_close_hint(name: &str) -> &'static str {
  match name {
    "fsFile" => "Close the file handle by calling `file.close()`.",
    "fetchRequest" => "Await the promise returned from `fetch()` or abort the fetch with an abort signal.",
    "fetchRequestBody" => "Terminate the request body `ReadableStream` by closing or erroring it.",
    "fetchResponse" => "Consume or close the response body `ReadableStream`, e.g `await resp.text()` or `await resp.body.cancel()`.",
    "httpClient" => "Close the HTTP client by calling `httpClient.close()`.",
    "dynamicLibrary" => "Unload the dynamic library by calling `dynamicLibrary.close()`.",
    "httpConn" => "Close the inbound HTTP connection by calling `httpConn.close()`.",
    "httpStream" => "Close the inbound HTTP request by responding with `e.respondWith()` or closing the HTTP connection.",
    "tcpStream" => "Close the TCP connection by calling `tcpConn.close()`.",
    "unixStream" => "Close the Unix socket connection by calling `unixConn.close()`.",
    "tlsStream" => "Close the TLS connection by calling `tlsConn.close()`.",
    "tlsListener" => "Close the TLS listener by calling `tlsListener.close()`.",
    "unixListener" => "Close the Unix socket listener by calling `unixListener.close()`.",
    "unixDatagram" => "Close the Unix datagram socket by calling `unixDatagram.close()`.",
    "tcpListener" => "Close the TCP listener by calling `tcpListener.close()`.",
    "udpSocket" => "Close the UDP socket by calling `udpSocket.close()`.",
    "timer" => "Clear the timer by calling `clearInterval` or `clearTimeout`.",
    "textDecoder" => "Close the text decoder by calling `textDecoder.decode('')` or `await textDecoderStream.readable.cancel()`.",
    "messagePort" => "Close the message port by calling `messagePort.close()`.",
    "webSocketStream" => "Close the WebSocket by calling `webSocket.close()`.",
    "fsEvents" => "Close the file system watcher by calling `watcher.close()`.",
    "childStdin" => "Close the child process stdin by calling `proc.stdin.close()`.",
    "childStdout" => "Close the child process stdout by calling `proc.stdout.close()` or `await child.stdout.cancel()`.",
    "childStderr" => "Close the child process stderr by calling `proc.stderr.close()` or `await child.stderr.cancel()`.",
    "child" => "Close the child process by calling `proc.kill()` or `proc.close()`.",
    "signal" => "Clear the signal listener by calling `Deno.removeSignalListener`.",
    "stdin" => "Close the stdin pipe by calling `Deno.stdin.close()`.",
    "stdout" => "Close the stdout pipe by calling `Deno.stdout.close()`.",
    "stderr" => "Close the stderr pipe by calling `Deno.stderr.close()`.",
    "compression" => "Close the compression stream by calling `await stream.writable.close()`.",
    _ => "Close the resource before the end of the test.",
  }
}

// An async operation to $0 was started in this test, but never completed. This is often caused by not $1.
// An async operation to $0 was started in this test, but never completed. Async operations should not complete in a test if they were not started in that test.
// deno-fmt-ignore
pub const OP_DETAILS: phf::Map<&'static str, [&'static str; 2]> = phf_map! {
  "op_blob_read_part" => ["read from a Blob or File", "awaiting the result of a Blob or File read"],
  "op_broadcast_recv" => ["receive a message from a BroadcastChannel", "closing the BroadcastChannel"],
  "op_broadcast_send" => ["send a message to a BroadcastChannel", "closing the BroadcastChannel"],
  "op_chmod_async" => ["change the permissions of a file", "awaiting the result of a `Deno.chmod` call"],
  "op_chown_async" => ["change the owner of a file", "awaiting the result of a `Deno.chown` call"],
  "op_copy_file_async" => ["copy a file", "awaiting the result of a `Deno.copyFile` call"],
  "op_crypto_decrypt" => ["decrypt data", "awaiting the result of a `crypto.subtle.decrypt` call"],
  "op_crypto_derive_bits" => ["derive bits from a key", "awaiting the result of a `crypto.subtle.deriveBits` call"],
  "op_crypto_encrypt" => ["encrypt data", "awaiting the result of a `crypto.subtle.encrypt` call"],
  "op_crypto_generate_key" => ["generate a key", "awaiting the result of a `crypto.subtle.generateKey` call"],
  "op_crypto_sign_key" => ["sign data", "awaiting the result of a `crypto.subtle.sign` call"],
  "op_crypto_subtle_digest" => ["digest data", "awaiting the result of a `crypto.subtle.digest` call"],
  "op_crypto_verify_key" => ["verify data", "awaiting the result of a `crypto.subtle.verify` call"],
  "op_net_recv_udp" => ["receive a datagram message via UDP", "awaiting the result of `Deno.DatagramConn#receive` call, or not breaking out of a for await loop looping over a `Deno.DatagramConn`"],
  "op_net_recv_unixpacket" => ["receive a datagram message via Unixpacket", "awaiting the result of `Deno.DatagramConn#receive` call, or not breaking out of a for await loop looping over a `Deno.DatagramConn`"],
  "op_net_send_udp" => ["send a datagram message via UDP", "awaiting the result of `Deno.DatagramConn#send` call"],
  "op_net_send_unixpacket" => ["send a datagram message via Unixpacket", "awaiting the result of `Deno.DatagramConn#send` call"],
  "op_dns_resolve" => ["resolve a DNS name", "awaiting the result of a `Deno.resolveDns` call"],
  "op_fdatasync_async" => ["flush pending data operations for a file to disk", "awaiting the result of a `file.fdatasync` call"],
  "op_fetch_send" => ["send a HTTP request", "awaiting the result of a `fetch` call"],
  "op_ffi_call_nonblocking" => ["do a non blocking ffi call", "awaiting the returned promise"],
  "op_ffi_call_ptr_nonblocking" => ["do a non blocking ffi call", "awaiting the returned promise"],
  "op_flock_async" => ["lock a file", "awaiting the result of a `Deno.flock` call"],
  "op_fs_events_poll" => ["get the next file system event", "breaking out of a for await loop looping over `Deno.FsEvents`"],
  "op_fstat_async" => ["get file metadata", "awaiting the result of a `Deno.File#fstat` call"],
  "op_fsync_async" => ["flush pending data operations for a file to disk", "awaiting the result of a `file.fsync` call"],
  "op_ftruncate_async" => ["truncate a file", "awaiting the result of a `Deno.ftruncate` call"],
  "op_funlock_async" => ["unlock a file", "awaiting the result of a `Deno.funlock` call"],
  "op_futime_async" => ["change file timestamps", "awaiting the result of a `Deno.futime` call"],
  "op_http_accept" => ["accept a HTTP request", "closing a `Deno.HttpConn`"],
  "op_http_shutdown" => ["shutdown a HTTP connection", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_http_upgrade_websocket" => ["upgrade a HTTP connection to a WebSocket", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_http_write_headers" => ["write HTTP response headers", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_http_write" => ["write HTTP response body", "awaiting `Deno.HttpEvent#respondWith`"],
  "op_link_async" => ["create a hard link", "awaiting the result of a `Deno.link` call"],
  "op_make_temp_dir_async" => ["create a temporary directory", "awaiting the result of a `Deno.makeTempDir` call"],
  "op_make_temp_file_async" => ["create a temporary file", "awaiting the result of a `Deno.makeTempFile` call"],
  "op_message_port_recv_message" => ["receive a message from a MessagePort", "awaiting the result of not closing a `MessagePort`"],
  "op_mkdir_async" => ["create a directory", "awaiting the result of a `Deno.mkdir` call"],
  "op_net_accept_tcp" => ["accept a TCP stream", "closing a `Deno.Listener`"],
  "op_net_accept_unix" => ["accept a Unix stream", "closing a `Deno.Listener`"],
  "op_net_connect_tcp" => ["connect to a TCP server", "awaiting a `Deno.connect` call"],
  "op_net_connect_unix" => ["connect to a Unix server", "awaiting a `Deno.connect` call"],
  "op_open_async" => ["open a file", "awaiting the result of a `Deno.open` call"],
  "op_read_dir_async" => ["read a directory", "collecting all items in the async iterable returned from a `Deno.readDir` call"],
  "op_read_link_async" => ["read a symlink", "awaiting the result of a `Deno.readLink` call"],
  "op_realpath_async" => ["resolve a path", "awaiting the result of a `Deno.realpath` call"],
  "op_remove_async" => ["remove a file or directory", "awaiting the result of a `Deno.remove` call"],
  "op_rename_async" => ["rename a file or directory", "awaiting the result of a `Deno.rename` call"],
  "op_run_status" => ["get the status of a subprocess", "awaiting the result of a `Deno.Process#status` call"],
  "op_seek_async" => ["seek in a file", "awaiting the result of a `Deno.File#seek` call"],
  "op_signal_poll" => ["get the next signal", "un-registering a OS signal handler"],
  "op_sleep" => ["sleep for a duration", "cancelling a `setTimeout` or `setInterval` call"],
  "op_stat_async" => ["get file metadata", "awaiting the result of a `Deno.stat` call"],
  "op_symlink_async" => ["create a symlink", "awaiting the result of a `Deno.symlink` call"],
  "op_net_accept_tls" => ["accept a TLS stream", "closing a `Deno.TlsListener`"],
  "op_net_connect_tls" => ["connect to a TLS server", "awaiting a `Deno.connectTls` call"],
  "op_tls_handshake" => ["perform a TLS handshake", "awaiting a `Deno.TlsConn#handshake` call"],
  "op_tls_start" => ["start a TLS connection", "awaiting a `Deno.startTls` call"],
  "op_truncate_async" => ["truncate a file", "awaiting the result of a `Deno.truncate` call"],
  "op_utime_async" => ["change file timestamps", "awaiting the result of a `Deno.utime` call"],
  "op_host_recv_message" => ["receive a message from a web worker", "terminating a `Worker`"],
  "op_host_recv_ctrl" => ["receive a message from a web worker", "terminating a `Worker`"],
  "op_webgpu_buffer_get_map_async" => ["map a WebGPU buffer", "awaiting the result of a `GPUBuffer#mapAsync` call"],
  "op_webgpu_request_adapter" => ["request a WebGPU adapter", "awaiting the result of a `navigator.gpu.requestAdapter` call"],
  "op_webgpu_request_device" => ["request a WebGPU device", "awaiting the result of a `GPUAdapter#requestDevice` call"],
  "op_ws_close" => ["close a WebSocket", "awaiting until the `close` event is emitted on a `WebSocket`, or the `WebSocketStream#closed` promise resolves"],
  "op_ws_create" => ["create a WebSocket", "awaiting until the `open` event is emitted on a `WebSocket`, or the result of a `WebSocketStream#connection` promise"],
  "op_ws_next_event" => ["receive the next message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
  "op_ws_send_text" => ["send a message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
  "op_ws_send_binary" => ["send a message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
  "op_ws_send_binary_ab" => ["send a message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
  "op_ws_send_ping" => ["send a message on a WebSocket", "closing a `WebSocket` or `WebSocketStream`"],
  "op_spawn_wait" => ["wait for a subprocess to exit", "awaiting the result of a `Deno.Process#status` call"],
};
