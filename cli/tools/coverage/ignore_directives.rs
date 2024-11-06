// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_ast::swc::common::comments::Comment;
use deno_ast::swc::common::comments::CommentKind;
use deno_ast::view as ast_view;
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
  program: &ast_view::Program,
) -> Option<FileIgnoreDirective> {
  // We want to get a file's leading comments, even if they come after a
  // shebang. There are three cases:
  // 1. No shebang. The file's leading comments are the program's leading
  //    comments.
  // 2. Shebang, and the program has statements or declarations. The file's
  //    leading comments are really the first statment/declaration's leading
  //    comments.
  // 3. Shebang, and the program is empty. The file's leading comments are the
  //    program's trailing comments.
  let (has_shebang, first_item_range) = match program {
    ast_view::Program::Module(module) => (
      module.shebang().is_some(),
      module.body.first().map(SourceRanged::range),
    ),
    ast_view::Program::Script(script) => (
      script.shebang().is_some(),
      script.body.first().map(SourceRanged::range),
    ),
  };

  let comments = program.comment_container();
  let mut initial_comments = match (has_shebang, first_item_range) {
    (false, _) => comments.leading_comments(program.start()),
    (true, Some(range)) => comments.leading_comments(range.start),
    (true, None) => comments.trailing_comments(program.end()),
  };
  initial_comments.find_map(|comment| {
    parse_ignore_comment(COVERAGE_IGNORE_FILE_DIRECTIVE, comment)
  })
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

  #[test]
  fn test_parse_global_ignore_directives() {
    parse_and_then("// deno-coverage-ignore-file", |program| {
      let file_directive = parse_file_ignore_directives(&program);

      assert!(file_directive.is_some());
    });

    parse_and_then(
      "// deno-coverage-ignore-file -- reason for ignoring",
      |program| {
        let file_directive = parse_file_ignore_directives(&program);

        assert!(file_directive.is_some());
      },
    );

    parse_and_then(
      "// deno-coverage-ignore-file foo -- reason for ignoring",
      |program| {
        let file_directive = parse_file_ignore_directives(&program);

        assert!(file_directive.is_some());
      },
    );

    parse_and_then(
      r#"
      const x = 42;
      // deno-coverage-ignore-file
      "#,
      |program| {
        let file_directive = parse_file_ignore_directives(&program);

        assert!(file_directive.is_none());
      },
    );

    parse_and_then(
      "#!/usr/bin/env -S deno run\n// deno-coverage-ignore-file",
      |program| {
        let file_directive = parse_file_ignore_directives(&program);

        assert!(file_directive.is_some());
      },
    );

    parse_and_then(
      "#!/usr/bin/env -S deno run\n// deno-coverage-ignore-file -- reason for ignoring",
      |program| {
        let file_directive =
          parse_file_ignore_directives(&program);

        assert!(file_directive.is_some());
      },
    );

    parse_and_then(
      "#!/usr/bin/env -S deno run\n// deno-coverage-ignore-file\nconst a = 42;",
      |program| {
        let file_directive = parse_file_ignore_directives(&program);

        assert!(file_directive.is_some());
      },
    );

    parse_and_then(
      "#!/usr/bin/env -S deno run\n// deno-coverage-ignore-file -- reason for ignoring\nconst a = 42;",
      |program| {
        let file_directive = parse_file_ignore_directives(&program);

        assert!(file_directive.is_some());
      },
    );
  }
}
