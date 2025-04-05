// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::HashSet;

use deno_ast::swc::common::comments::CommentKind;
use deno_ast::MediaType;
use deno_ast::TextLines;
use deno_core::url::Url;

static COVERAGE_IGNORE_START_DIRECTIVE: &str = "deno-coverage-ignore-start";
static COVERAGE_IGNORE_STOP_DIRECTIVE: &str = "deno-coverage-ignore-stop";
static COVERAGE_IGNORE_NEXT_DIRECTIVE: &str = "deno-coverage-ignore";
static COVERAGE_IGNORE_FILE_DIRECTIVE: &str = "deno-coverage-ignore-file";

pub struct RangeIgnoreDirective {
  pub start_line_index: usize,
  pub stop_line_index: usize,
}

pub struct CoverageComment {
  pub kind: CommentKind,
  pub text: deno_ast::swc::atoms::Atom,
  pub range: std::ops::Range<usize>,
}

pub struct CoverageComments {
  pub comments: Vec<CoverageComment>,
  pub first_token: Option<std::ops::Range<usize>>,
}

pub fn lex_comments(source: &str, media_type: MediaType) -> CoverageComments {
  let mut first_token = None;
  let mut comments = Vec::new();
  for token in deno_ast::lex(source, media_type) {
    match token.inner {
      deno_ast::TokenOrComment::Token(inner) => {
        if first_token.is_none()
          && !matches!(inner, deno_ast::swc::parser::token::Token::Shebang(..))
        {
          first_token = Some(token.range);
        }
      }
      deno_ast::TokenOrComment::Comment { kind, text } => {
        comments.push(CoverageComment {
          kind,
          text,
          range: token.range,
        })
      }
    }
  }

  CoverageComments {
    first_token,
    comments,
  }
}

pub fn parse_range_ignore_directives(
  script_module_specifier: &Url,
  comments: &[CoverageComment],
  text_lines: &TextLines,
) -> Vec<RangeIgnoreDirective> {
  let mut depth: usize = 0;
  let mut directives = Vec::<RangeIgnoreDirective>::new();
  let mut current_range: Option<std::ops::Range<usize>> = None;

  for comment in comments {
    if comment.kind != CommentKind::Line {
      continue;
    }

    let comment_text = comment.text.trim();

    if let Some(prefix) = comment_text.split_whitespace().next() {
      if prefix == COVERAGE_IGNORE_START_DIRECTIVE {
        if log::log_enabled!(log::Level::Warn) && depth > 0 {
          let unterminated_loc = text_lines
            .line_and_column_display(current_range.as_ref().unwrap().start);
          let loc = text_lines.line_and_column_display(comment.range.start);
          log::warn!(
            "WARNING: Nested {} comment at {}:{}:{}. A previous {} comment at {}:{}:{} is unterminated.",
            COVERAGE_IGNORE_START_DIRECTIVE,
            script_module_specifier,
            loc.line_number,
            loc.column_number,
            COVERAGE_IGNORE_START_DIRECTIVE,
            script_module_specifier,
            unterminated_loc.line_number,
            unterminated_loc.column_number,
          );
        }
        depth += 1;
        if current_range.is_none() {
          current_range = Some(comment.range.clone());
        }
      } else if depth > 0 && prefix == COVERAGE_IGNORE_STOP_DIRECTIVE {
        depth -= 1;
        if depth == 0 {
          let start_line_index =
            text_lines.line_index(current_range.take().unwrap().start);
          let stop_line_index = text_lines.line_index(comment.range.end);
          directives.push(RangeIgnoreDirective {
            start_line_index,
            stop_line_index,
          });
          current_range = None;
        }
      } else if log::log_enabled!(log::Level::Warn)
        && depth == 0
        && prefix == COVERAGE_IGNORE_STOP_DIRECTIVE
      {
        let loc = text_lines.line_and_column_display(comment.range.start);
        log::warn!(
          "WARNING: {} comment with no corresponding {} comment at {}:{}:{} will be ignored.",
          COVERAGE_IGNORE_STOP_DIRECTIVE,
          COVERAGE_IGNORE_START_DIRECTIVE,
          script_module_specifier,
          loc.line_number,
          loc.column_number,
        );
      }
    }
  }

  // If the coverage ignore start directive has no corresponding close directive
  // then log a warning and ignore the directive.
  if let Some(range) = current_range.take() {
    if log::log_enabled!(log::Level::Warn) {
      let loc = text_lines.line_and_column_display(range.start);
      log::warn!(
        "WARNING: Unterminated {} comment at {}:{}:{} will be ignored.",
        COVERAGE_IGNORE_START_DIRECTIVE,
        script_module_specifier,
        loc.line_number,
        loc.column_number,
      );
    }
  }

  directives
}

pub fn parse_next_ignore_directives(
  comments: &[CoverageComment],
  text_lines: &TextLines,
) -> HashSet<usize> {
  comments
    .iter()
    .filter_map(|comment| {
      if is_ignore_comment(COVERAGE_IGNORE_NEXT_DIRECTIVE, comment) {
        Some(text_lines.line_index(comment.range.start))
      } else {
        None
      }
    })
    .collect()
}

pub fn has_file_ignore_directive(comments: &CoverageComments) -> bool {
  // We want to find the files first comment before the code starts. There are
  // three cases:
  // 1. No comments. There are no comments in the file, and therefore no
  //    coverage directives.
  // 2. No code. There is at least one comment in the file, but no code. We can
  //    try to parse this as a file ignore directive.
  // 3. Comments and code. There are comments and code in the file. We need to
  //    check if the first comment comes before the first line of code. If it
  //    does, we can try and parse it as a file ignore directive. Otherwise,
  //    there is no valid file ignore directive.

  let first_comment = comments.comments.first();
  let first_module_item = &comments.first_token;

  match (first_comment, first_module_item) {
    (None, _) => false,
    (Some(first_comment), None) => {
      is_ignore_comment(COVERAGE_IGNORE_FILE_DIRECTIVE, first_comment)
    }
    (Some(first_comment), Some(first_module_item)) => {
      if first_comment.range.end <= first_module_item.start {
        is_ignore_comment(COVERAGE_IGNORE_FILE_DIRECTIVE, first_comment)
      } else {
        false
      }
    }
  }
}

fn is_ignore_comment(
  ignore_diagnostic_directive: &str,
  comment: &CoverageComment,
) -> bool {
  if comment.kind != CommentKind::Line {
    return false;
  }

  let comment_text = comment.text.trim();

  if let Some(prefix) = comment_text.split_whitespace().next() {
    if prefix == ignore_diagnostic_directive {
      return true;
    }
  }

  false
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use deno_ast::MediaType;
  use deno_ast::TextLines;

  use super::*;

  const TEST_FILE_NAME: &str = "file:///coverage_test.ts";

  fn parse(source_code: &str) -> CoverageComments {
    lex_comments(source_code, MediaType::TypeScript)
  }

  fn parse_with_text_lines(source_code: &str) -> (CoverageComments, TextLines) {
    let comments = parse(source_code);
    let text_lines = TextLines::new(source_code);
    (comments, text_lines)
  }

  mod coverage_ignore_range {
    use super::*;

    #[test]
    fn test_parse_range_ignore_comments() {
      let source_code = r#"
        // deno-coverage-ignore-start
        function foo(): any {}
        // deno-coverage-ignore-stop

        function bar(): any {
          // deno-coverage-ignore-start
          foo();
          // deno-coverage-ignore-stop
        }
      "#;
      let (comments, text_line) = parse_with_text_lines(source_code);
      let range_directives = parse_range_ignore_directives(
        &Url::from_str(TEST_FILE_NAME).unwrap(),
        &comments.comments,
        &text_line,
      );
      assert_eq!(range_directives.len(), 2);
      assert_eq!(range_directives[0].start_line_index, 1);
      assert_eq!(range_directives[0].stop_line_index, 3);
      assert_eq!(range_directives[1].start_line_index, 6);
      assert_eq!(range_directives[1].stop_line_index, 8);
    }

    #[test]
    fn test_parse_range_ignore_comments_unterminated() {
      let source_code = r#"
        // deno-coverage-ignore-start
        function foo(): any {}

        function bar(): any {
          foo();
        }
      "#;
      let (comments, text_lines) = parse_with_text_lines(source_code);
      let range_directives = parse_range_ignore_directives(
        &Url::from_str(TEST_FILE_NAME).unwrap(),
        &comments.comments,
        &text_lines,
      );
      assert!(range_directives.is_empty());
    }

    #[test]
    fn test_parse_range_ignore_comments_nested() {
      let source_code = r#"
        // deno-coverage-ignore-start
        function foo(): any {}

        function bar(): any {
          // deno-coverage-ignore-start
          foo();
          // deno-coverage-ignore-stop
        }
        // deno-coverage-ignore-stop
      "#;
      let (comments, text_lines) = parse_with_text_lines(source_code);
      let range_directives = parse_range_ignore_directives(
        &Url::from_str(TEST_FILE_NAME).unwrap(),
        &comments.comments,
        &text_lines,
      );
      assert_eq!(range_directives.len(), 1);
      assert_eq!(range_directives[0].start_line_index, 1);
      assert_eq!(range_directives[0].stop_line_index, 9);
    }
  }

  mod coverage_ignore_next {
    use super::*;

    #[test]
    fn test_parse_next_ignore_comments() {
      let source_code = r#"
        // deno-coverage-ignore
        function foo(): any {}

        function bar(): any {
          // deno-coverage-ignore
          foo();
        }
      "#;
      let (comments, text_lines) = parse_with_text_lines(source_code);
      let line_directives =
        parse_next_ignore_directives(&comments.comments, &text_lines);
      assert_eq!(line_directives.len(), 2);
      assert!(line_directives.contains(&1));
      assert!(line_directives.contains(&5));
    }
  }

  mod coverage_ignore_file {
    use super::*;

    #[test]
    fn test_parse_global_ignore_directives() {
      let comments = parse("// deno-coverage-ignore-file");
      assert!(has_file_ignore_directive(&comments));
    }

    #[test]
    fn test_parse_global_ignore_directives_with_explanation() {
      let comments =
        parse("// deno-coverage-ignore-file -- reason for ignoring");
      assert!(has_file_ignore_directive(&comments));
    }

    #[test]
    fn test_parse_global_ignore_directives_argument_and_explanation() {
      let comments =
        parse("// deno-coverage-ignore-file foo -- reason for ignoring");
      assert!(has_file_ignore_directive(&comments));
    }

    #[test]
    fn test_parse_global_ignore_directives_not_first_comment() {
      let comments = parse(
        r#"
        // The coverage ignore file comment must be first
        // deno-coverage-ignore-file
        const x = 42;
      "#,
      );
      assert!(!has_file_ignore_directive(&comments));
    }

    #[test]
    fn test_parse_global_ignore_directives_not_before_code() {
      let comments = parse(
        r#"
        const x = 42;
        // deno-coverage-ignore-file
      "#,
      );
      assert!(!has_file_ignore_directive(&comments));
    }

    #[test]
    fn test_parse_global_ignore_directives_shebang() {
      let comments = parse(
        r#"
          #!/usr/bin/env -S deno run
          // deno-coverage-ignore-file
          const x = 42;
        "#
        .trim_start(),
      );
      assert!(has_file_ignore_directive(&comments));
    }

    #[test]
    fn test_parse_global_ignore_directives_shebang_no_code() {
      let comments = parse(
        r#"
        #!/usr/bin/env -S deno run
        // deno-coverage-ignore-file
        "#
        .trim_start(),
      );
      assert!(has_file_ignore_directive(&comments));
    }
  }
}
