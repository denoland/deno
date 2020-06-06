use regex::Regex;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::PathBuf;

use crate::fs as deno_fs;
use crate::installer::is_remote_url;
use crate::test_runner::is_supported;

pub struct DocTest {
  // This removes repetition of imports in a file
  imports: HashSet<String>,
  // This contains codes in an @example section with their imports removed
  bodies: Vec<DocTestBody>,
}

struct DocTestBody {
  caption: String,
  line_number: usize,
  path: String,
  value: String,
  ignore: bool,
  is_async: bool,
}

pub fn prepare_doctest(
  mut include: Vec<String>,
  root_path: &PathBuf,
) -> Vec<DocTest> {
  include.retain(|n| !is_remote_url(n));

  let mut prepared = vec![];

  for path in include {
    let p = deno_fs::normalize_path(&root_path.join(path));
    if p.is_dir() {
      let test_files = deno_fs::files_in_subtree(p, |p| {
        let valid_ext = ["ts", "tsx", "js", "jsx"];
        p.extension()
          .and_then(OsStr::to_str)
          .map(|ext| valid_ext.contains(&ext) && !is_supported(p))
          .unwrap_or(false)
      });
      prepared.extend(test_files);
    } else {
      prepared.push(p);
    }
  }

  prepared
    .iter()
    .filter_map(|dir| {
      // TODO(iykekings) use deno error instead
      let content = std::fs::read_to_string(&dir)
        .unwrap_or_else(|_| panic!("File doesn't exist {}", dir.display()));
      extract_jsdoc_examples(content, dir.to_owned())
    })
    .collect::<Vec<_>>()
}

fn extract_jsdoc_examples(input: String, p: PathBuf) -> Option<DocTest> {
  lazy_static! {
    static ref JS_DOC_PATTERN: Regex =
      Regex::new(r"/\*\*\s*\n([^\*]|\*[^/])*\*/").unwrap();
    // IMPORT_PATTERN doesn't match dynamic imports by design
    static ref IMPORT_PATTERN: Regex =
      Regex::new(r"import[^(].*\n").unwrap();
    static ref EXAMPLE_PATTERN: Regex = Regex::new(r"@example\s*(?:<\w+>.*</\w+>)*\n(?:\s*\*\s*\n*)*```").unwrap();
    static ref TEST_TAG_PATTERN: Regex = Regex::new(r"@example\s*(?:<\w+>.*</\w+>)*\n(?:\s*\*\s*\n*)*```(\w+)").unwrap();
    static ref AWAIT_PATTERN: Regex = Regex::new(r"\Wawait\s").unwrap();
  }

  let mut import_set = HashSet::new();

  let test_bodies = JS_DOC_PATTERN
    .captures_iter(&input)
    .filter_map(|caps| caps.get(0).map(|c| (c.start(), c.as_str())))
    .flat_map(|(offset, section)| {
      EXAMPLE_PATTERN.find_iter(section).filter_map(move |cap| {
        section[cap.end()..].find("```").map(|i| {
          (
            offset + cap.end(),
            section[cap.start()..i + cap.end()].to_string(),
          )
        })
      })
    })
    .filter_map(|(offset, example_section)| {
      let test_tag = TEST_TAG_PATTERN
        .captures(&example_section)
        .and_then(|m| m.get(1).map(|c| c.as_str()));

      if test_tag == Some("text") {
        return None;
      }

      IMPORT_PATTERN
        .captures_iter(&example_section)
        .filter_map(|caps| caps.get(0).map(|m| m.as_str()))
        .for_each(|import| {
          import_set.insert(import.to_string());
        });

      let caption = get_caption_from_example(&example_section);
      let line_number = &input[0..offset].lines().count();
      let code_block = get_code_from_example(&example_section);
      let is_async = AWAIT_PATTERN.find(&example_section).is_some();

      let cwd = std::env::current_dir()
        .expect("expected: process has a current working directory");
      let path = p
        .to_str()
        .map(|x| x.replace(cwd.to_str().unwrap_or(""), ""));
      Some(DocTestBody {
        caption,
        line_number: *line_number,
        path: path.unwrap_or("".to_string()),
        value: code_block,
        ignore: test_tag == Some("ignore"),
        is_async,
      })
    })
    .collect::<Vec<_>>();

  match test_bodies.len() {
    0 => None,
    _ => Some(DocTest {
      imports: import_set,
      bodies: test_bodies,
    }),
  }
}

pub fn render_doctest_file(
  doctests: Vec<DocTest>,
  fail_fast: bool,
  quiet: bool,
  filter: Option<String>,
) -> String {
  let mut test_file = "".to_string();

  // TODO(iykekings) - discuss with team if this is fine
  let default_import = "import { 
    assert,
    assertArrayContains,
    assertEquals,
    assertMatch,
    assertNotEquals,
    assertStrContains,
    assertStrictEq,
    assertThrows,
    assertThrowsAsync,
    equal,
    unimplemented,
    unreachable,
   } from \"https://deno.land/std/testing/asserts.ts\";\n";

  test_file.push_str(default_import);

  let all_imports: String = doctests
    .iter()
    .map(|doctest| doctest.imports.clone())
    .flatten()
    .collect();

  test_file.push_str(&all_imports);
  test_file.push_str("\n");

  let all_test_section = doctests
      .into_iter()
      .map(|doctest| doctest.bodies.into_iter())
      .flatten()
      .map(|test| {
          let async_str = if test.is_async {"async "} else {""};
          format!(
              "Deno.test({{\n\tname: \"{} - {} (line {})\",\n\tignore: {},\n\t{}fn() {{\n{}\n}}\n}});\n",
              &test.path[1..],
              test.caption,
              test.line_number,
              test.ignore,
              async_str,
              test.value
          )
      })
      .collect::<Vec<_>>()
      .join("\n");

  test_file.push_str(&all_test_section);

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

  test_file
}

fn get_caption_from_example(ex: &str) -> String {
  lazy_static! {
    static ref CAPTION_PATTERN: Regex =
      Regex::new(r"<caption>([\s\w\W]+)</caption>").unwrap();
  }
  CAPTION_PATTERN
    .captures(ex)
    .and_then(|cap| cap.get(1).map(|m| m.as_str()))
    .unwrap_or("")
    .to_string()
}

fn get_code_from_example(ex: &str) -> String {
  lazy_static! {
    static ref TICKS_OR_IMPORT_PATTERN: Regex =
      Regex::new(r"(?:import[^(].*)|(?:```\w*)").unwrap();
  }
  TICKS_OR_IMPORT_PATTERN
    .replace_all(ex, "\n")
    .lines()
    .skip(1)
    .filter_map(|line| {
      let res = match line.trim_start().starts_with('*') {
        true => line.replacen("*", "", 1).trim_start().to_string(),
        false => line.trim_start().to_string(),
      };
      match res.len() {
        0 => None,
        _ => Some(format!("  {}", res)),
      }
    })
    .collect::<Vec<_>>()
    .join("\n")
}
