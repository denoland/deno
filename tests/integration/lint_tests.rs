// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use test_util::assert_contains;
use test_util::assert_not_contains;
use util::TestContext;
use util::TestContextBuilder;

#[test]
fn all_lint_rules_have_docs() {
  let context = TestContextBuilder::new().build();

  let output = context
    .new_command()
    .args("lint --internal-print-all-rules")
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  let lines = output_text.lines();

  let mut missing_docs = vec![];

  for line in lines {
    let snake_case_name = line.replace("-", "_");
    let doc_path = format!("cli/tools/lint/docs/{}.md", snake_case_name);
    if !util::root_path().join(&doc_path).exists() {
      missing_docs.push(doc_path)
    }
  }

  missing_docs.sort();

  assert!(
    missing_docs.is_empty(),
    "Missing lint rule docs:\n{}",
    missing_docs.join("\n")
  );
}

#[test]
fn all_lint_rules_are_listed_in_schema_file() {
  let context = TestContextBuilder::new().build();

  let output = context
    .new_command()
    .args("lint --internal-print-all-rules")
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  let mut rules = output_text.lines().collect::<Vec<_>>();
  rules.sort();

  let mut missing_docs = vec![];

  for line in lines {
    let snake_case_name = line.replace("-", "_");
    let doc_path = format!("cli/tools/lint/docs/{}.md", snake_case_name);
    if !util::root_path().join(&doc_path).exists() {
      missing_docs.push(doc_path)
    }
  }

  assert!(
    missing_docs.is_empty(),
    "Missing lint rule docs:\n{}",
    missing_docs.join("\n")
  );

  // TODO(bartlomieju): assert that the schema file is up to date
}
