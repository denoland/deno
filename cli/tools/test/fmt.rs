// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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
