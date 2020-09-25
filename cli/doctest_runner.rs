use crate::ast;
use crate::flags::Flags;
use crate::global_state::GlobalState;
use crate::media_type::MediaType;
use deno_core::serde_json::json;
use deno_core::url::Url;
use deno_core::{error::AnyError, ModuleSpecifier};
use deno_doc::DocParser;
use jsdoc::{self, ast::Tag, Input};
use regex::Regex;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::test_runner::is_supported;

lazy_static! {
  static ref IMPORT_PATTERN: Regex =
    Regex::new(r#"import\s+?(?:(?:(?:[\w*\s{},]*)\s+from\s+?)|)(?:(?:".*?")|(?:'.*?'))[\s]*?(?:;|$|)"#).unwrap();
  static ref EXAMPLE_PATTERN: Regex = Regex::new(r"@example\s*(?:<\w+>.*</\w+>)*\n(?:\s*\*\s*\n*)*```").unwrap();
  static ref TEST_TAG_PATTERN: Regex = Regex::new(r"```(\w+)?").unwrap();
  static ref AWAIT_PATTERN: Regex = Regex::new(r"\Wawait\s").unwrap();
  static ref CAPTION_PATTERN: Regex =
      Regex::new(r"<caption>([\s\w\W]+)</caption>").unwrap();
  static ref TICKS_OR_IMPORT_PATTERN: Regex =
      Regex::new(r"(?:import[^(].*)|(?:```\w*)").unwrap();
}

pub async fn parse_jsdocs(
  source_files: &Vec<Url>,
  flags: Flags,
) -> Result<Vec<(ast::Location, String)>, AnyError> {
  let global_state = GlobalState::new(flags.clone())?;
  let loader = Box::new(global_state.file_fetcher.clone());

  let doc_parser = DocParser::new(loader, false);
  let mut results = vec![];
  for url in source_files {
    let source_code =
      doc_parser.loader.load_source_code(&url.to_string()).await?;
    let path = PathBuf::from(&url.to_string());
    let media_type = MediaType::from(&path);
    let specifier = ModuleSpecifier::resolve_url(&url.to_string())?;
    let module = ast::parse(&specifier, &source_code, &media_type)?;

    let result = module
      .get_leading_comments()
      .into_iter()
      .map(|comment| {
        jsdoc::parse(Input::from(&comment))
          .expect("Error Parsing Jsdoc")
          .1
      })
      .flat_map(|jsdoc| jsdoc.tags)
      .filter_map(|tag_item| match tag_item.tag {
        Tag::Example(ex_tag) => Some((
          module.get_location(&ex_tag.text.span),
          ex_tag.text.value.to_string(),
        )),
        _ => None,
      })
      .collect::<Vec<_>>();
    results.extend(result);
  }
  Ok(results)
}

pub fn prepare_doctests(
  jsdocs: Vec<(ast::Location, String)>,
  fail_fast: bool,
  quiet: bool,
  filter: Option<String>,
) -> Result<String, AnyError> {
  let mut test_file =  "import * as assert from \"https://deno.land/std@0.70.0/testing/asserts.ts\";\n".to_string();
  let mut import_set = HashSet::new();
  let cwd = std::env::current_dir()?;
  let cwd_url_str = Url::from_directory_path(cwd)
    .map(|url| url.to_string())
    .unwrap_or("".to_string());
  let tests: String = jsdocs
    .into_iter()
    .map(|(loc, example)| (loc, clean_string(&example)))
    .filter_map(|(l, x)| {
      let test_tag = extract_test_tag(&x);
      if test_tag == Some("text") {
        return  None;
      }
      let ignore = test_tag == Some("ignore");
      extract_imports(&x).into_iter().for_each(|import| { import_set.insert(import.to_string()); });
      let caption = extract_caption(&x);
      let code_body = extract_code_body(&x);
      let is_async = has_await(&x);
      let res = format!(
        "\nDeno.test({{\n\tname: \"{} - {} (line {})\",\n\tignore: {},\n\t{}fn() {{\n{}\n}}\n}});\n",
        l.filename.replace(&cwd_url_str, ""),
        caption.unwrap_or(""),
        l.line,
        ignore,
        if is_async { "async"} else {""},
        code_body
    );
    Some(res)
    })
    .collect();
  let imports_str = import_set.into_iter().collect::<Vec<_>>().join("\n");
  test_file.push_str(&imports_str);
  test_file.push_str("\n");
  test_file.push_str(&tests);

  let options = if let Some(filter) = filter {
    json!({ "failFast": fail_fast, "reportToConsole": !quiet, "disableLog": quiet, "isDoctest": true, "filter": filter })
  } else {
    json!({ "failFast": fail_fast, "reportToConsole": !quiet, "disableLog": quiet, "isDoctest": true })
  };

  let run_tests_cmd = format!(
    "\n// @ts-ignore\nDeno[Deno.internal].runTests({});\n",
    options
  );

  test_file.push_str(&run_tests_cmd);
  Ok(test_file)
}

fn clean_string(input: &str) -> String {
  input
    .lines()
    .map(|line| {
      if line.trim().starts_with("*") {
        &line.trim()[1..]
      } else {
        line.trim()
      }
    })
    .filter(|line| line.len() > 0)
    .collect::<Vec<_>>()
    .join("\n")
}

pub fn is_supported_doctest(path: &Path) -> bool {
  let valid_ext = ["ts", "tsx", "js", "jsx"];
  path
    .extension()
    .and_then(OsStr::to_str)
    .map(|ext| valid_ext.contains(&ext) && !is_supported(path))
    .unwrap_or(false)
}

fn extract_test_tag(input: &str) -> Option<&str> {
  TEST_TAG_PATTERN
    .captures(input)
    .and_then(|m| m.get(1).map(|c| c.as_str()))
}

fn extract_caption(input: &str) -> Option<&str> {
  CAPTION_PATTERN
    .captures(input)
    .and_then(|m| m.get(1).map(|c| c.as_str()))
}

fn extract_imports(input: &str) -> Vec<&str> {
  IMPORT_PATTERN
    .captures_iter(input)
    .filter_map(|caps| caps.get(0).map(|m| m.as_str()))
    .collect()
}

fn has_await(input: &str) -> bool {
  AWAIT_PATTERN.find(input).is_some()
}

fn extract_code_body(ex: &str) -> String {
  let code_sans_imports = IMPORT_PATTERN
    .replace_all(ex, "\n")
    .lines()
    .filter_map(|line| {
      let res = line.trim();
      if line.len() > 0 {
        Some(res)
      } else {
        None
      }
    })
    .collect::<Vec<_>>()
    .join("\n");
  let code_sans_tag = TEST_TAG_PATTERN.replace_all(&code_sans_imports, "");
  CAPTION_PATTERN
    .replace(&code_sans_tag, "")
    .trim()
    .to_string()
}

#[cfg(test)]
mod test {
  use super::*;
  #[test]
  fn test_extract_jsdoc() {
    let test = r#"<caption>Linkedlists.compareWith</caption>
    * ```ts
    * import { LinkedList } from './js_test/linkedlist.ts'
    * const testArr = [1, 2, 3, 4, 5, 6, 78, 9, 0, 65];
    * const firstList = new LinkedList<number>();
    * const secondList = new LinkedList<number>();
    * for (let data of testArr) {
    *   firstList.insertNode(data);
    *   secondList.insertNode(data);
    * }
    * const result = firstList.compareWith(secondList);
    * assert(result);
    ```"#;
    let res = clean_string(test);
    assert_eq!(extract_caption(&res), Some("Linkedlists.compareWith"));
    assert_eq!(
      extract_imports(&res),
      vec!["import { LinkedList } from './js_test/linkedlist.ts'"]
    );
    assert!(!has_await(&res));
    assert_eq!(extract_test_tag(&res), Some("ts"));
    assert_eq!(
      extract_code_body(&res),
      vec![
        "const testArr = [1, 2, 3, 4, 5, 6, 78, 9, 0, 65];",
        "const firstList = new LinkedList<number>();",
        "const secondList = new LinkedList<number>();",
        "for (let data of testArr) {",
        "firstList.insertNode(data);",
        "secondList.insertNode(data);",
        "}",
        "const result = firstList.compareWith(secondList);",
        "assert.assert(result);"
      ]
      .join("\n")
    )
  }

  #[test]
  fn test_async_detection() {
    let test = r#"
    ```ts
    const response = await fetch("https://deno.land");
    const body = await response.text();
    assert(body.length > 0);
    ```"#;
    assert!(has_await(&test));
  }
}
