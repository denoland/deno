// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::swc::common::comments::Comment;
use deno_ast::swc::common::comments::CommentKind;
use deno_ast::swc::common::Spanned;
use deno_ast::view as ast_view;
use deno_ast::ParsedSource;
use deno_ast::RootNode as _;
use deno_ast::SourceRange;
use deno_ast::SourceRanged;
use deno_ast::SourceRangedForSpanned as _;
use deno_ast::SourceTextInfoProvider as _;
use deno_core::url::Url;
use std::collections::HashMap;

static COVERAGE_IGNORE_START_DIRECTIVE: &str = "deno-coverage-ignore-start";
static COVERAGE_IGNORE_STOP_DIRECTIVE: &str = "deno-coverage-ignore-stop";
static COVERAGE_IGNORE_NEXT_DIRECTIVE: &str = "deno-coverage-ignore-next";
static COVERAGE_IGNORE_FILE_DIRECTIVE: &str = "deno-coverage-ignore-file";

pub type RangeIgnoreDirective = IgnoreDirective<Range>;
pub type NextIgnoreDirective = IgnoreDirective<Next>;
pub type FileIgnoreDirective = IgnoreDirective<File>;

pub enum Range {}
pub enum Next {}
pub enum File {}
pub trait DirectiveKind {}
impl DirectiveKind for Range {}
impl DirectiveKind for Next {}
impl DirectiveKind for File {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IgnoreDirective<T: DirectiveKind> {
  range: SourceRange,
  _marker: std::marker::PhantomData<T>,
}

impl<T: DirectiveKind> IgnoreDirective<T> {
  pub fn range(&self) -> SourceRange {
    self.range
  }
}

pub fn parse_range_ignore_directives(
  is_quiet: bool,
  script_module_specifier: &Url,
  program: &ast_view::Program,
) -> Vec<RangeIgnoreDirective> {
  let mut depth: usize = 0;
  let mut directives = Vec::<RangeIgnoreDirective>::new();
  let mut current_range: Option<SourceRange> = None;

  let mut comments_sorted = program
    .comment_container()
    .all_comments()
    .collect::<Vec<_>>();
  comments_sorted.sort_by(|a, b| a.range().start.cmp(&b.range().start));

  for comment in comments_sorted.iter() {
    if comment.kind != CommentKind::Line {
      continue;
    }

    let comment_text = comment.text.trim();

    if let Some(prefix) = comment_text.split_whitespace().next() {
      if prefix == COVERAGE_IGNORE_START_DIRECTIVE {
        depth += 1;
        if current_range.is_none() {
          current_range = Some(comment.range());
        }
      } else if depth > 0 && prefix == COVERAGE_IGNORE_STOP_DIRECTIVE {
        depth -= 1;
        if depth == 0 {
          let mut range = current_range.take().unwrap();
          range.end = comment.range().end;
          directives.push(IgnoreDirective {
            range,
            _marker: std::marker::PhantomData,
          });
          current_range = None;
        }
      }
    }
  }

  // If the coverage ignore start directive has no corresponding close directive
  // then close it at the end of the program.
  if let Some(mut range) = current_range.take() {
    if !is_quiet {
      let text_info = program.text_info();
      let loc = text_info.line_and_column_display(range.start);
      log::warn!(
        "WARNING: Unterminated {} comment at {}:{}:{}",
        COVERAGE_IGNORE_START_DIRECTIVE,
        script_module_specifier,
        loc.line_number,
        loc.column_number,
      );
    }
    range.end = program.range().end;
    directives.push(IgnoreDirective {
      range,
      _marker: std::marker::PhantomData,
    });
  }

  directives
}

pub fn parse_next_ignore_directives(
  program: &ast_view::Program,
) -> HashMap<usize, NextIgnoreDirective> {
  program
    .comment_container()
    .all_comments()
    .filter_map(|comment| {
      parse_ignore_comment(COVERAGE_IGNORE_NEXT_DIRECTIVE, comment).map(
        |directive| {
          (
            program.text_info().line_index(directive.range().start),
            directive,
          )
        },
      )
    })
    .collect()
}

pub fn parse_file_ignore_directives(
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

  let sorted_comments = parsed_source.comments().get_vec();
  let first_comment = sorted_comments.first();
  let first_module_item = parsed_source.program_ref().body().next();

  match (first_comment, first_module_item) {
    (None, _) => None,
    (Some(first_comment), None) => {
      parse_ignore_comment(COVERAGE_IGNORE_FILE_DIRECTIVE, first_comment)
    }
    (Some(first_comment), Some(first_module_item)) => {
      if first_comment.span_hi() <= first_module_item.span_lo() {
        parse_ignore_comment(COVERAGE_IGNORE_FILE_DIRECTIVE, first_comment)
      } else {
        None
      }
    }
  }
}

fn parse_ignore_comment<T: DirectiveKind>(
  ignore_diagnostic_directive: &str,
  comment: &Comment,
) -> Option<IgnoreDirective<T>> {
  if comment.kind != CommentKind::Line {
    return None;
  }

  let comment_text = comment.text.trim();

  if let Some(prefix) = comment_text.split_whitespace().next() {
    if prefix == ignore_diagnostic_directive {
      return Some(IgnoreDirective::<T> {
        range: comment.range(),
        _marker: std::marker::PhantomData,
      });
    }
  }

  None
}

#[cfg(test)]
mod tests {
  use std::str::FromStr;

  use deno_ast::MediaType;
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParsedSource;

  use crate::tools::coverage::ast_parser;

  use super::*;

  const TEST_FILE_NAME: &str = "file:///coverage_test.ts";

  pub fn parse(source_code: &str) -> ParsedSource {
    ast_parser::parse_program(
      ModuleSpecifier::parse(TEST_FILE_NAME).unwrap(),
      MediaType::TypeScript,
      source_code,
    )
    .unwrap()
  }

  pub fn parse_and_then(source_code: &str, test: impl Fn(ast_view::Program)) {
    let parsed_source = parse(source_code);
    parsed_source.with_view(|pg| {
      test(pg);
    });
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

      parse_and_then(source_code, |program| {
        let line_directives = parse_range_ignore_directives(
          true,
          &Url::from_str(TEST_FILE_NAME).unwrap(),
          &program,
        );

        assert_eq!(line_directives.len(), 2);
      });
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

      parse_and_then(source_code, |program| {
        let line_directives = parse_range_ignore_directives(
          true,
          &Url::from_str(TEST_FILE_NAME).unwrap(),
          &program,
        );

        assert_eq!(line_directives.len(), 1);
      });
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

      parse_and_then(source_code, |program| {
        let line_directives = parse_range_ignore_directives(
          true,
          &Url::from_str(TEST_FILE_NAME).unwrap(),
          &program,
        );

        assert_eq!(line_directives.len(), 1);
      });
    }
  }

  mod coverage_ignore_next {
    use super::*;

    #[test]
    fn test_parse_next_ignore_comments() {
      let source_code = r#"
          // deno-coverage-ignore-next
          function foo(): any {}

          function bar(): any {
            // deno-coverage-ignore-next
            foo();
          }
      "#;

      parse_and_then(source_code, |program| {
        let line_directives = parse_next_ignore_directives(&program);

        assert_eq!(line_directives.len(), 2);
      });
    }
  }

  mod coverage_ignore_file {
    use super::*;

    #[test]
    fn test_parse_global_ignore_directives() {
      let parsed_source = parse("// deno-coverage-ignore-file");
      let file_directive = parse_file_ignore_directives(&parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_with_explanation() {
      let parsed_source =
        parse("// deno-coverage-ignore-file -- reason for ignoring");
      let file_directive = parse_file_ignore_directives(&parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_argument_and_explanation() {
      let parsed_source =
        parse("// deno-coverage-ignore-file foo -- reason for ignoring");
      let file_directive = parse_file_ignore_directives(&parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_not_first_comment() {
      let parsed_source = parse(
        r#"
        // The coverage ignore file comment must be first
        // deno-coverage-ignore-file
        const x = 42;
      "#,
      );
      let file_directive = parse_file_ignore_directives(&parsed_source);
      assert!(file_directive.is_none());
    }

    #[test]
    fn test_parse_global_ignore_directives_not_before_code() {
      let parsed_source = parse(
        r#"
        const x = 42;
        // deno-coverage-ignore-file
      "#,
      );
      let file_directive = parse_file_ignore_directives(&parsed_source);
      assert!(file_directive.is_none());
    }

    #[test]
    fn test_parse_global_ignore_directives_shebang() {
      let parsed_source = parse(
        r#"
        #!/usr/bin/env -S deno run
        // deno-coverage-ignore-file
        const x = 42;
      "#
        .trim_start(),
      );
      let file_directive = parse_file_ignore_directives(&parsed_source);
      assert!(file_directive.is_some());
    }

    #[test]
    fn test_parse_global_ignore_directives_shebang_no_code() {
      let parsed_source = parse(
        r#"
       #!/usr/bin/env -S deno run
       // deno-coverage-ignore-file
      "#
        .trim_start(),
      );
      let file_directive = parse_file_ignore_directives(&parsed_source);

      assert!(file_directive.is_some());
    }
  }
}
