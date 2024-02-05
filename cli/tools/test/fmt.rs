// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::stats::RuntimeActivity;
use deno_core::stats::RuntimeActivityDiff;
use deno_core::stats::RuntimeActivityType;
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
