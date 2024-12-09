// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use deno_core::serde_json;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use serde::Serialize;
use test_util as util;
use util::TestContextBuilder;

fn get_all_rules() -> Vec<String> {
  let context = TestContextBuilder::new().build();

  let output = context
    .new_command()
    .args("lint --internal-print-all-rules")
    .run();

  output.assert_exit_code(0);
  let output_text = output.combined_output();
  let mut rules = output_text
    .lines()
    .into_iter()
    .map(|s| s.to_string())
    .collect::<Vec<String>>();
  rules.sort();
  rules
}

#[test]
fn all_lint_rules_have_docs() {
  let all_rules = get_all_rules();
  let mut missing_docs = vec![];

  for rule in all_rules {
    let snake_case_name = rule.replace("-", "_");
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
  let all_rules = get_all_rules();

  let rules_schema_path =
    util::root_path().join("cli/schemas/lint-rules.v1.json");
  let rules_schema_file = std::fs::read_to_string(&rules_schema_path).unwrap();

  #[derive(Serialize, Deserialize)]
  struct RulesSchema {
    #[serde(rename = "$schema")]
    schema: String,

    #[serde(rename = "enum")]
    rules: Vec<String>,
  }
  let schema: RulesSchema = serde_json::from_str(&rules_schema_file).unwrap();

  const UPDATE_ENV_VAR_NAME: &'static str = "UPDATE_EXPECTED";

  if std::env::var(UPDATE_ENV_VAR_NAME).ok().is_none() {
    assert_eq!(
      schema.rules, all_rules,
      "Lint rules schema file not up to date. Run again with {} to update the expected output",
      UPDATE_ENV_VAR_NAME
    );
  } else {
    std::fs::write(
      &rules_schema_path,
      serde_json::to_string_pretty(&RulesSchema {
        schema: schema.schema,
        rules: all_rules,
      })
      .unwrap(),
    )
    .unwrap();
  }
}
