// Copyright 2018-2026 the Deno authors. MIT license.
//! This mod provides DenoError to unify errors across Deno.
use std::borrow::Cow;
use std::fmt::Write as _;
use std::sync::LazyLock;

use color_print::cformat;
use color_print::cstr;
use deno_core::error::JsError;
use deno_core::error::format_frame;
use deno_core::url::Url;
use deno_terminal::colors;

use crate::source_highlight::syntax_highlight_source_line;

#[derive(Debug, Clone)]
struct ErrorReference<'a> {
  from: &'a JsError,
  to: &'a JsError,
}

#[derive(Debug, Clone)]
struct IndexedErrorReference<'a> {
  reference: ErrorReference<'a>,
  index: usize,
}

#[derive(Debug)]
enum FixSuggestionKind {
  Info,
  Hint,
  Docs,
}

#[derive(Debug)]
enum FixSuggestionMessage<'a> {
  Single(&'a str),
  Multiline(&'a [&'a str]),
}

#[derive(Debug)]
pub struct FixSuggestion<'a> {
  kind: FixSuggestionKind,
  message: FixSuggestionMessage<'a>,
}

impl<'a> FixSuggestion<'a> {
  pub fn info(message: &'a str) -> Self {
    Self {
      kind: FixSuggestionKind::Info,
      message: FixSuggestionMessage::Single(message),
    }
  }

  pub fn info_multiline(messages: &'a [&'a str]) -> Self {
    Self {
      kind: FixSuggestionKind::Info,
      message: FixSuggestionMessage::Multiline(messages),
    }
  }

  pub fn hint(message: &'a str) -> Self {
    Self {
      kind: FixSuggestionKind::Hint,
      message: FixSuggestionMessage::Single(message),
    }
  }

  pub fn hint_multiline(messages: &'a [&'a str]) -> Self {
    Self {
      kind: FixSuggestionKind::Hint,
      message: FixSuggestionMessage::Multiline(messages),
    }
  }

  pub fn docs(url: &'a str) -> Self {
    Self {
      kind: FixSuggestionKind::Docs,
      message: FixSuggestionMessage::Single(url),
    }
  }
}

struct AnsiColors;

impl deno_core::error::ErrorFormat for AnsiColors {
  fn fmt_element(
    element: deno_core::error::ErrorElement,
    in_extension_code: bool,
    s: &str,
  ) -> std::borrow::Cow<'_, str> {
    if in_extension_code {
      return colors::dimmed_gray(s).to_string().into();
    }
    use deno_core::error::ErrorElement::*;
    match element {
      Anonymous | NativeFrame | FileName | EvalOrigin => {
        colors::cyan(s).to_string().into()
      }
      LineNumber | ColumnNumber => colors::yellow(s).to_string().into(),
      FunctionName | PromiseAll => colors::italic_bold(s).to_string().into(),
      WorkingDirPath => colors::dimmed_gray(s).to_string().into(),
      PlainText => s.into(),
    }
  }
}

/// Take an optional source line and associated information to format it into
/// a pretty printed version of that line.
fn format_maybe_source_line(
  source_line: Option<&str>,
  column_number: Option<i64>,
  line_number: Option<i64>,
  is_error: bool,
  level: usize,
) -> String {
  if source_line.is_none() || column_number.is_none() {
    return "".to_string();
  }

  let source_line = source_line.unwrap();
  // sometimes source_line gets set with an empty string, which then outputs
  // an empty source line when displayed, so need just short circuit here.
  if source_line.is_empty() {
    return "".to_string();
  }
  if source_line.contains("Couldn't format source line: ") {
    return format!("\n{source_line}");
  }

  let mut s = String::new();
  let column_number = column_number.unwrap();

  if column_number as usize > source_line.len() {
    return format!(
      "\n{} Couldn't format source line: Column {} is out of bounds (source may have changed at runtime)",
      colors::yellow("Warning"),
      column_number,
    );
  }

  // Build the line number gutter prefix: "  42 | "
  let (line_prefix, gutter_width) = if let Some(ln) = line_number {
    let ln_str = ln.to_string();
    let prefix = if colors::use_color() {
      format!("{} {} ", colors::gray(&ln_str), colors::gray("|"))
    } else {
      format!("{} | ", ln_str)
    };
    // Width of the gutter in visible characters (for the caret alignment)
    let width = ln_str.len() + 3; // "42 | " = digits + " | "
    (prefix, width)
  } else {
    (String::new(), 0)
  };

  // Build caret padding (accounting for gutter width)
  for _ in 0..gutter_width {
    s.push(' ');
  }
  for _i in 0..(column_number - 1) {
    if source_line.chars().nth(_i as usize).unwrap() == '\t' {
      s.push('\t');
    } else {
      s.push(' ');
    }
  }
  s.push('^');
  let color_underline = if is_error {
    colors::red_bold(&s).to_string()
  } else {
    colors::cyan(&s).to_string()
  };

  let indent = format!("{:indent$}", "", indent = level);
  let highlighted_source =
    syntax_highlight_source_line(source_line, colors::use_color());

  format!(
    "\n{indent}{line_prefix}{highlighted_source}\n{indent}{color_underline}"
  )
}

fn find_recursive_cause(js_error: &JsError) -> Option<ErrorReference<'_>> {
  let mut history = Vec::<&JsError>::new();

  let mut current_error: &JsError = js_error;

  while let Some(cause) = &current_error.cause {
    history.push(current_error);

    if let Some(seen) = history.iter().find(|&el| cause.is_same_error(el)) {
      return Some(ErrorReference {
        from: current_error,
        to: seen,
      });
    } else {
      current_error = cause;
    }
  }

  None
}

fn format_aggregated_error(
  aggregated_errors: &Vec<JsError>,
  circular_reference_index: usize,
  initial_cwd: Option<&Url>,
  filter_frames: bool,
) -> String {
  let mut s = String::new();
  let mut nested_circular_reference_index = circular_reference_index;

  for js_error in aggregated_errors {
    let aggregated_circular = find_recursive_cause(js_error);
    if aggregated_circular.is_some() {
      nested_circular_reference_index += 1;
    }
    let error_string = format_js_error_inner(
      js_error,
      aggregated_circular.map(|reference| IndexedErrorReference {
        reference,
        index: nested_circular_reference_index,
      }),
      false,
      filter_frames,
      vec![],
      initial_cwd,
    );

    for line in error_string.trim_start_matches("Uncaught ").lines() {
      write!(s, "\n    {line}").unwrap();
    }
  }

  s
}

fn stack_frame_is_ext(frame: &deno_core::error::JsStackFrame) -> bool {
  frame
    .file_name
    .as_ref()
    .map(|file_name| {
      file_name.starts_with("ext:") || file_name.starts_with("node:")
    })
    .unwrap_or(false)
}

/// Colorize an exception message like "Uncaught (in promise) TypeError: msg".
///
/// - "(in promise)" is grayed out
/// - The error class name (e.g. "TypeError") is colored red
fn colorize_exception_message(msg: &str) -> String {
  if !colors::use_color() {
    return msg.to_string();
  }

  let mut remaining = msg;
  let mut result = String::with_capacity(msg.len() + 40);

  // Strip "Uncaught " prefix (will be re-added plain)
  let uncaught = remaining.starts_with("Uncaught ");
  if uncaught {
    result.push_str("Uncaught ");
    remaining = &remaining["Uncaught ".len()..];
  }

  // Handle optional "(in promise) "
  if remaining.starts_with("(in promise) ") {
    result.push_str(&colors::gray("(in promise)").to_string());
    result.push(' ');
    remaining = &remaining["(in promise) ".len()..];
  }

  // Find the error class name — everything up to the first ": "
  if let Some(colon_pos) = remaining.find(": ") {
    let class_name = &remaining[..colon_pos];
    // Only colorize if it looks like an error class name
    // (starts with uppercase, contains only alphanumeric chars)
    if class_name
      .chars()
      .next()
      .is_some_and(|c| c.is_ascii_uppercase())
      && class_name.chars().all(|c| c.is_ascii_alphanumeric())
    {
      result.push_str(&colors::red_bold(class_name).to_string());
      result.push_str(&remaining[colon_pos..]);
    } else {
      result.push_str(remaining);
    }
  } else {
    result.push_str(remaining);
  }

  result
}

fn format_js_error_inner(
  js_error: &JsError,
  circular: Option<IndexedErrorReference>,
  include_source_code: bool,
  filter_frames: bool,
  suggestions: Vec<FixSuggestion>,
  initial_cwd: Option<&Url>,
) -> String {
  let mut s = String::new();

  s.push_str(&colorize_exception_message(&js_error.exception_message));

  if let Some(circular) = &circular
    && js_error.is_same_error(circular.reference.to)
  {
    write!(s, " {}", colors::cyan(format!("<ref *{}>", circular.index)))
      .unwrap();
  }

  if let Some(aggregated) = &js_error.aggregated {
    let aggregated_message = format_aggregated_error(
      aggregated,
      circular
        .as_ref()
        .map(|circular| circular.index)
        .unwrap_or(0),
      initial_cwd,
      filter_frames,
    );
    s.push_str(&aggregated_message);
  }

  let source_frame = js_error
    .source_line_frame_index
    .and_then(|i| js_error.frames.get(i));
  let column_number = source_frame.and_then(|f| f.column_number);
  let line_number = source_frame.and_then(|f| f.line_number);
  s.push_str(&format_maybe_source_line(
    if include_source_code {
      js_error.source_line.as_deref()
    } else {
      None
    },
    column_number,
    line_number,
    true,
    0,
  ));

  let at_dimmed = Cow::Owned(colors::dimmed_gray("at ").to_string());
  let at_normal = Cow::Borrowed("at ");
  for frame in &js_error.frames {
    let is_ext = stack_frame_is_ext(frame);
    if filter_frames
      && is_ext
      && let Some(fn_name) = &frame.function_name
      && (fn_name.starts_with("__node_internal_")
        || fn_name == "eventLoopTick"
        || fn_name == "denoErrorToNodeError"
        || fn_name == "__drainNextTickAndMacrotasks")
    {
      continue;
    }
    write!(
      s,
      "\n    {}{}",
      if is_ext { &at_dimmed } else { &at_normal },
      format_frame::<AnsiColors>(frame, initial_cwd)
    )
    .unwrap();
  }
  if let Some(cause) = &js_error.cause {
    let is_caused_by_circular = circular
      .as_ref()
      .map(|circular| js_error.is_same_error(circular.reference.from))
      .unwrap_or(false);

    let error_string = if is_caused_by_circular {
      colors::cyan(format!("[Circular *{}]", circular.unwrap().index))
        .to_string()
    } else {
      format_js_error_inner(cause, circular, false, false, vec![], initial_cwd)
    };

    write!(
      s,
      "\nCaused by: {}",
      error_string.trim_start_matches("Uncaught ")
    )
    .unwrap();
  }
  if !suggestions.is_empty() {
    write!(s, "\n\n").unwrap();
    for (index, suggestion) in suggestions.iter().enumerate() {
      write!(s, "    ").unwrap();
      match suggestion.kind {
        FixSuggestionKind::Hint => {
          write!(s, "{} ", colors::cyan("hint:")).unwrap()
        }
        FixSuggestionKind::Info => {
          write!(s, "{} ", colors::yellow("info:")).unwrap()
        }
        FixSuggestionKind::Docs => {
          write!(s, "{} ", colors::green("docs:")).unwrap()
        }
      };
      match suggestion.message {
        FixSuggestionMessage::Single(msg) => {
          if matches!(suggestion.kind, FixSuggestionKind::Docs) {
            write!(s, "{}", cformat!("<u>{}</>", msg)).unwrap();
          } else {
            write!(s, "{}", msg).unwrap();
          }
        }
        FixSuggestionMessage::Multiline(messages) => {
          for (idx, message) in messages.iter().enumerate() {
            if idx != 0 {
              writeln!(s).unwrap();
              write!(s, "          ").unwrap();
            }
            write!(s, "{}", message).unwrap();
          }
        }
      }

      if index != (suggestions.len() - 1) {
        writeln!(s).unwrap();
      }
    }
  }

  s
}

fn get_suggestions_for_terminal_errors(e: &JsError) -> Vec<FixSuggestion<'_>> {
  if let Some(msg) = &e.message {
    if msg.contains("module is not defined")
      || msg.contains("exports is not defined")
      || msg.contains("require is not defined")
    {
      if let Some(file_name) =
        e.frames.first().and_then(|f| f.file_name.as_ref())
        && (file_name.ends_with(".mjs") || file_name.ends_with(".mts"))
      {
        return vec![];
      }
      return vec![
        FixSuggestion::info_multiline(&[
          cstr!(
            "Deno supports CommonJS modules in <u>.cjs</> files, or when the closest"
          ),
          cstr!(
            "<u>package.json</> has a <i>\"type\": \"commonjs\"</> option."
          ),
        ]),
        FixSuggestion::hint_multiline(&[
          "Rewrite this module to ESM,",
          cstr!("or change the file extension to <u>.cjs</u>,"),
          cstr!(
            "or add <u>package.json</> next to the file with <i>\"type\": \"commonjs\"</> option,"
          ),
          cstr!(
            "or pass <i>--unstable-detect-cjs</> flag to detect CommonJS when loading."
          ),
        ]),
        FixSuggestion::docs("https://docs.deno.com/go/commonjs"),
      ];
    } else if msg.contains("__filename is not defined") {
      return vec![
        FixSuggestion::info(cstr!(
          "<u>__filename</> global is not available in ES modules."
        )),
        FixSuggestion::hint(cstr!("Use <u>import.meta.filename</> instead.")),
      ];
    } else if msg.contains("__dirname is not defined") {
      return vec![
        FixSuggestion::info(cstr!(
          "<u>__dirname</> global is not available in ES modules."
        )),
        FixSuggestion::hint(cstr!("Use <u>import.meta.dirname</> instead.")),
      ];
    } else if msg.contains("openKv is not a function") {
      return vec![
        FixSuggestion::info("Deno.openKv() is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-kv` flag to enable this API.",
        ),
      ];
    } else if msg.contains("bundle is not a function") {
      return vec![
        FixSuggestion::info("Deno.bundle() is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-bundle` flag to enable this API.",
        ),
      ];
    } else if msg.contains("cron is not a function") {
      return vec![
        FixSuggestion::info("Deno.cron() is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-cron` flag to enable this API.",
        ),
      ];
    } else if msg.contains("WebSocketStream is not defined") {
      return vec![
        FixSuggestion::info("new WebSocketStream() is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-net` flag to enable this API.",
        ),
      ];
    } else if msg.contains("window is not defined") {
      return vec![
        FixSuggestion::info("window global is not available in Deno 2."),
        FixSuggestion::hint("Replace `window` with `globalThis`."),
      ];
    } else if msg.contains("UnsafeWindowSurface is not a constructor") {
      return vec![
        FixSuggestion::info("Deno.UnsafeWindowSurface is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-webgpu` flag to enable this API.",
        ),
      ];
    } else if msg.contains("QuicEndpoint is not a constructor") {
      return vec![
        FixSuggestion::info("listenQuic is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-net` flag to enable this API.",
        ),
      ];
    } else if msg.contains("connectQuic is not a function") {
      return vec![
        FixSuggestion::info("connectQuic is an unstable API."),
        FixSuggestion::hint(
          "Run again with `--unstable-net` flag to enable this API.",
        ),
      ];
    } else if msg.contains("client error (Connect): invalid peer certificate") {
      return vec![FixSuggestion::hint(
        "Run again with the `--unsafely-ignore-certificate-errors` flag to bypass certificate errors.",
      )];
    // Try to capture errors like:
    // ```
    // Uncaught Error: Cannot find module '../build/Release/canvas.node'
    // Require stack:
    // - /.../deno/npm/registry.npmjs.org/canvas/2.11.2/lib/bindings.js
    // - /.../.cache/deno/npm/registry.npmjs.org/canvas/2.11.2/lib/canvas.js
    // ```
    } else if msg.contains("Cannot find module")
      && msg.contains("Require stack")
      && msg.contains(".node'")
    {
      return vec![
        FixSuggestion::info_multiline(&[
          "Trying to execute an npm package using Node-API addons,",
          "these packages require local `node_modules` directory to be present.",
        ]),
        FixSuggestion::hint_multiline(&[
          "Add `\"nodeModulesDir\": \"auto\" option to `deno.json`, and then run",
          "`deno install --allow-scripts=npm:<package> --entrypoint <script>` to setup `node_modules` directory.",
        ]),
      ];
    } else if msg.contains("document is not defined") {
      return vec![
        FixSuggestion::info(cstr!(
          "<u>document</> global is not available in Deno."
        )),
        FixSuggestion::hint_multiline(&[
          cstr!(
            "Use a library like <u>happy-dom</>, <u>deno_dom</>, <u>linkedom</> or <u>JSDom</>"
          ),
          cstr!(
            "and setup the <u>document</> global according to the library documentation."
          ),
        ]),
      ];
    }
  }

  vec![]
}

static SHOULD_FILTER_FRAMES: LazyLock<bool> =
  LazyLock::new(|| std::env::var("DENO_NO_FILTER_FRAMES").is_err());

/// Format a [`JsError`] for terminal output.
pub fn format_js_error(
  js_error: &JsError,
  initial_cwd: Option<&Url>,
) -> String {
  let circular =
    find_recursive_cause(js_error).map(|reference| IndexedErrorReference {
      reference,
      index: 1,
    });
  let suggestions = get_suggestions_for_terminal_errors(js_error);
  format_js_error_inner(
    js_error,
    circular,
    true,
    *SHOULD_FILTER_FRAMES,
    suggestions,
    initial_cwd,
  )
}

#[cfg(test)]
mod tests {
  use test_util::strip_ansi_codes;

  use super::*;

  #[test]
  fn test_format_none_source_line() {
    let actual = format_maybe_source_line(None, None, None, false, 0);
    assert_eq!(actual, "");
  }

  #[test]
  fn test_format_some_source_line() {
    let actual = format_maybe_source_line(
      Some("console.log('foo');"),
      Some(9),
      None,
      true,
      0,
    );
    assert_eq!(
      strip_ansi_codes(&actual),
      "\nconsole.log(\'foo\');\n        ^"
    );
  }

  #[test]
  fn test_format_source_line_with_line_number() {
    let actual = format_maybe_source_line(
      Some("console.log('foo');"),
      Some(9),
      Some(42),
      true,
      0,
    );
    let stripped = strip_ansi_codes(&actual);
    assert_eq!(stripped, "\n42 | console.log(\'foo\');\n             ^");
  }

  #[test]
  fn test_colorize_exception_message_no_color() {
    colors::set_use_color(false);
    let msg = "Uncaught (in promise) TypeError: foo";
    assert_eq!(colorize_exception_message(msg), msg);
    colors::set_use_color(true);
  }

  #[test]
  fn test_colorize_exception_message_basic() {
    colors::set_use_color(true);
    let result =
      colorize_exception_message("Uncaught TypeError: something failed");
    let stripped = strip_ansi_codes(&result);
    assert_eq!(stripped, "Uncaught TypeError: something failed");
    // "TypeError" should be red+bold
    assert!(result.contains("TypeError"), "result: {result}");
    // Should NOT contain plain "Uncaught TypeError" (TypeError must be styled)
    assert!(!result.contains("Uncaught TypeError:"), "result: {result}");
  }

  #[test]
  fn test_colorize_exception_message_in_promise() {
    colors::set_use_color(true);
    let result = colorize_exception_message(
      "Uncaught (in promise) Error: something failed",
    );
    let stripped = strip_ansi_codes(&result);
    assert_eq!(stripped, "Uncaught (in promise) Error: something failed");
    // "(in promise)" should be styled (gray)
    assert!(
      !result.contains("Uncaught (in promise) Error"),
      "result: {result}"
    );
  }

  #[test]
  fn test_colorize_exception_message_no_colon() {
    colors::set_use_color(true);
    // No ": " in message — should pass through unchanged
    let result = colorize_exception_message("Uncaught something");
    let stripped = strip_ansi_codes(&result);
    assert_eq!(stripped, "Uncaught something");
  }

  #[test]
  fn test_colorize_exception_message_not_class_name() {
    colors::set_use_color(true);
    // lowercase after "Uncaught " — not an error class name
    let result = colorize_exception_message("Uncaught error: something failed");
    let stripped = strip_ansi_codes(&result);
    assert_eq!(stripped, "Uncaught error: something failed");
  }
}
