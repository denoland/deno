// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;

use deno_ast::swc::common::comments::Comment;
use deno_ast::swc::common::comments::CommentKind;
use deno_ast::swc::common::Spanned;
use deno_ast::ParsedSource;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned as _;
use deno_ast::SourceTextInfo;
use deno_core::url::Url;

static COVERAGE_IGNORE_START_DIRECTIVE: &str = "deno-coverage-ignore-start";
static COVERAGE_IGNORE_STOP_DIRECTIVE: &str = "deno-coverage-ignore-stop";
static COVERAGE_IGNORE_NEXT_DIRECTIVE: &str = "deno-coverage-ignore";
static COVERAGE_IGNORE_FILE_DIRECTIVE: &str = "deno-coverage-ignore-file";

pub struct RangeIgnoreDirective {
  pub start_line_index: usize,
  pub stop_line_index: usize,
}
pub struct NextIgnoreDirective;
pub struct FileIgnoreDirective;

pub fn parse_range_ignore_directives(
  is_quiet: bool,
  script_module_specifier: &Url,
  sorted_comments: &[Comment],
  text_info: &SourceTextInfo,
) -> Vec<RangeIgnoreDirective> {
  let mut depth: usize = 0;
  let mut directives = Vec::<RangeIgnoreDirective>::new();
  let mut current_range: Option<SourceRange> = None;

  for comment in sorted_comments.iter() {
    if comment.kind != CommentKind::Line {
      continue;
    }

    let comment_text = comment.text.trim();

    if let Some(prefix) = comment_text.split_whitespace().next() {
      if prefix == COVERAGE_IGNORE_START_DIRECTIVE {
        if !is_quiet && depth > 0 {
          let unterminated_loc =
            text_info.line_and_column_display(current_range.unwrap().start);
          let loc = text_info.line_and_column_display(comment.range().start);
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
          current_range = Some(comment.range());
        }
      } else if depth > 0 && prefix == COVERAGE_IGNORE_STOP_DIRECTIVE {
        depth -= 1;
        if depth == 0 {
          let start_line_index =
            text_info.line_index(current_range.take().unwrap().start);
          let stop_line_index = text_info.line_index(comment.range().end);
          directives.push(RangeIgnoreDirective {
            start_line_index,
            stop_line_index,
          });
          current_range = None;
        }
      } else if !is_quiet
        && depth == 0
        && prefix == COVERAGE_IGNORE_STOP_DIRECTIVE
      {
        let loc = text_info.line_and_column_display(comment.range().start);
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
    if !is_quiet {
      let loc = text_info.line_and_column_display(range.start);
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
  sorted_comments: &[Comment],
  text_info: &SourceTextInfo,
) -> HashMap<usize, NextIgnoreDirective> {
  sorted_comments
    .iter()
    .filter_map(|comment| {
      parse_ignore_comment(COVERAGE_IGNORE_NEXT_DIRECTIVE, comment, |comment| {
        let line_index = text_info.line_index(comment.range().start);
        (line_index, NextIgnoreDirective)
      })
    })
    .collect()
}

pub fn parse_file_ignore_directives(
  sorted_comments: &[Comment],
  parsed_source: &ParsedSource,
) -> Option<FileIgnoreDirective> {
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

  let first_comment = sorted_comments.first();
  let first_module_item = parsed_source.program_ref().body().next();

  match (first_comment, first_module_item) {
    (None, _) => None,
    (Some(first_comment), None) => parse_ignore_comment(
      COVERAGE_IGNORE_FILE_DIRECTIVE,
      first_comment,
      |_| FileIgnoreDirective,
    ),
    (Some(first_comment), Some(first_module_item)) => {
      if first_comment.span_hi() <= first_module_item.span_lo() {
        parse_ignore_comment(
          COVERAGE_IGNORE_FILE_DIRECTIVE,
          first_comment,
          |_| FileIgnoreDirective,
        )
      } else {
        None
      }
    }
  }
}

fn is_ignore_comment(
  ignore_diagnostic_directive: &str,
  comment: &Comment,
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

fn parse_ignore_comment<T, TMapper>(
  ignore_diagnostic_directive: &str,
  comment: &Comment,
  mapper: TMapper,
) -> Option<T>
where
  TMapper: FnOnce(&Comment) -> T,
{
  if is_ignore_comment(ignore_diagnostic_directive, comment) {
    Some(mapper(comment))
  } else {
    None
  }
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParsedSource;

  use super::*;
  use crate::tools::coverage::ast_parser;

  const TEST_FILE_NAME: &str = "file:///coverage_test.ts";

  pub fn parse(source_code: &str) -> ParsedSource {
    ast_parser::parse_program(
      ModuleSpecifier::parse(TEST_FILE_NAME).unwrap(),
      MediaType::TypeScript,
      source_code,
    )
    .unwrap()
  }

  pub fn parse_with_sorted_comments(
    source_code: &str,
  ) -> (ParsedSource, Vec<Comment>) {
    let parsed_source = parse(source_code);
    let sorted_comments = parsed_source.comments().get_vec();
    (parsed_source, sorted_comments)
  }

  pub fn parse_with_sorted_comments_and_text_info(
    source_code: &str,
  ) -> (ParsedSource, Vec<Comment>, SourceTextInfo) {
    let parsed_source = parse(source_code);
    let sorted_comments = parsed_source.comments().get_vec();
    let text_info = SourceTextInfo::new(parsed_source.text().clone());
    (parsed_source, sorted_comments, text_info)
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
      let (_, sorted_comments, text_info) =
        parse_with_sorted_comments_and_text_info(source_code);
      let range_directives = parse_range_ignore_directives(
        true,
        &Url::from_str(TEST_FILE_NAME).unwrap(),
        &sorted_comments,
        &text_info,
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
      let (_, sorted_comments, text_info) =
        parse_with_sorted_comments_and_text_info(source_code);
      let range_directives = parse_range_ignore_directives(
        true,
        &Url::from_str(TEST_FILE_NAME).unwrap(),
        &sorted_comments,
        &text_info,
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
      let (_, sorted_comments, text_info) =
        parse_with_sorted_comments_and_text_info(source_code);
      let range_directives = parse_range_ignore_directives(
        true,
        &Url::from_str(TEST_FILE_NAME).unwrap(),
        &sorted_comments,
        &text_info,
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
      let (_, sorted_comments, text_info) =
        parse_with_sorted_comments_and_text_info(source_code);
      let line_directives =
        parse_next_ignore_directives(&sorted_comments, &text_info);
      assert_eq!(line_directives.len(), 2);
      assert!(line_directives.contains_key(&1));
      assert!(line_directives.contains_key(&5));
    }
  }

  mod coverage_ignore_file {
    use super::*;

    #[test]
    fn test_parse_global_ignore_directives() {
      let (parsed_source, sorted_comments) =
        parse_with_sorted_comments("// deno-coverage-ignore-file");
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_with_explanation() {
      let (parsed_source, sorted_comments) = parse_with_sorted_comments(
        "// deno-coverage-ignore-file -- reason for ignoring",
      );
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_argument_and_explanation() {
      let (parsed_source, sorted_comments) = parse_with_sorted_comments(
        "// deno-coverage-ignore-file foo -- reason for ignoring",
      );
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_not_first_comment() {
      let (parsed_source, sorted_comments) = parse_with_sorted_comments(
        r#"
        // The coverage ignore file comment must be first
        // deno-coverage-ignore-file
        const x = 42;
      "#,
      );
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);
      assert!(file_directive.is_none());
    }

    #[test]
    fn test_parse_global_ignore_directives_not_before_code() {
      let (parsed_source, sorted_comments) = parse_with_sorted_comments(
        r#"
        const x = 42;
        // deno-coverage-ignore-file
      "#,
      );
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);
      assert!(file_directive.is_none());
    }

    #[test]
    fn test_parse_global_ignore_directives_shebang() {
      let (parsed_source, sorted_comments) = parse_with_sorted_comments(
        r#"
          #!/usr/bin/env -S deno run
          // deno-coverage-ignore-file
          const x = 42;
        "#
        .trim_start(),
      );
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_shebang_no_code() {
      let (parsed_source, sorted_comments) = parse_with_sorted_comments(
        r#"
        #!/usr/bin/env -S deno run
        // deno-coverage-ignore-file
        "#
        .trim_start(),
      );
      let file_directive =
        parse_file_ignore_directives(&sorted_comments, &parsed_source);

      assert!(file_directive.is_some());
    }
  }
}
