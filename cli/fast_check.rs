// Copyright 2018-2026 the Deno authors. MIT license.

use std::sync::Arc;

/// Removes decorator-only lines from fast-check emitted output.
///
/// Fast-check output is declaration-like TypeScript, so decorators are never
/// valid there. A simple line filter is sufficient for the emitted shape.
pub fn strip_decorators(text: &str) -> String {
  let mut sanitized = String::with_capacity(text.len());
  let mut changed = false;

  for line in text.split_inclusive('\n') {
    let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
    if line_without_newline.trim_start().starts_with('@') {
      changed = true;
      continue;
    }
    sanitized.push_str(line);
  }

  if changed { sanitized } else { text.to_owned() }
}

pub fn strip_decorators_arc(text: &Arc<str>) -> Arc<str> {
  let sanitized = strip_decorators(text);
  if sanitized.as_str() == text.as_ref() {
    text.clone()
  } else {
    sanitized.into()
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn strips_decorator_lines() {
    let text = r#"export class Auth {
  @Inject("jwt")
  declare private readonly options?: any;
}
"#;

    assert_eq!(
      strip_decorators(text),
      r#"export class Auth {
  declare private readonly options?: any;
}
"#
    );
  }
}
