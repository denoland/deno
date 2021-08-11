// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

use test_util as util;

#[cfg(unix)]
#[test]
fn pty_multiline() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"(\n1 + 2\n)\n").unwrap();
    master.write_all(b"{\nfoo: \"foo\"\n}\n").unwrap();
    master.write_all(b"`\nfoo\n`\n").unwrap();
    master.write_all(b"`\n\\`\n`\n").unwrap();
    master.write_all(b"'{'\n").unwrap();
    master.write_all(b"'('\n").unwrap();
    master.write_all(b"'['\n").unwrap();
    master.write_all(b"/{/\n").unwrap();
    master.write_all(b"/\\(/\n").unwrap();
    master.write_all(b"/\\[/\n").unwrap();
    master.write_all(b"console.log(\"{test1} abc {test2} def {{test3}}\".match(/{([^{].+?)}/));\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains('3'));
    assert!(output.contains("{ foo: \"foo\" }"));
    assert!(output.contains("\"\\nfoo\\n\""));
    assert!(output.contains("\"\\n`\\n\""));
    assert!(output.contains("\"{\""));
    assert!(output.contains("\"(\""));
    assert!(output.contains("\"[\""));
    assert!(output.contains("/{/"));
    assert!(output.contains("/\\(/"));
    assert!(output.contains("/\\[/"));
    assert!(output.contains("[ \"{test1}\", \"test1\" ]"));
  });
}

#[cfg(unix)]
#[test]
fn pty_unpaired_braces() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b")\n").unwrap();
    master.write_all(b"]\n").unwrap();
    master.write_all(b"}\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("Unexpected token `)`"));
    assert!(output.contains("Unexpected token `]`"));
    assert!(output.contains("Unexpected token `}`"));
  });
}

#[cfg(unix)]
#[test]
fn pty_bad_input() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"'\\u{1f3b5}'[0]\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("Unterminated string literal"));
  });
}

#[cfg(unix)]
#[test]
fn pty_syntax_error_input() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"('\\u')\n").unwrap();
    master.write_all(b"('\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("Unterminated string constant"));
    assert!(output.contains("Unexpected eof"));
  });
}

#[cfg(unix)]
#[test]
fn pty_complete_symbol() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"Symbol.it\t\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("Symbol(Symbol.iterator)"));
  });
}

#[cfg(unix)]
#[test]
fn pty_complete_declarations() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"class MyClass {}\n").unwrap();
    master.write_all(b"My\t\n").unwrap();
    master.write_all(b"let myVar;\n").unwrap();
    master.write_all(b"myV\t\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("> MyClass"));
    assert!(output.contains("> myVar"));
  });
}

#[cfg(unix)]
#[test]
fn pty_complete_primitives() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"let func = function test(){}\n").unwrap();
    master.write_all(b"func.appl\t\n").unwrap();
    master.write_all(b"let str = ''\n").unwrap();
    master.write_all(b"str.leng\t\n").unwrap();
    master.write_all(b"false.valueO\t\n").unwrap();
    master.write_all(b"5n.valueO\t\n").unwrap();
    master.write_all(b"let num = 5\n").unwrap();
    master.write_all(b"num.toStrin\t\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("> func.apply"));
    assert!(output.contains("> str.length"));
    assert!(output.contains("> 5n.valueOf"));
    assert!(output.contains("> false.valueOf"));
    assert!(output.contains("> num.toString"));
  });
}

#[cfg(unix)]
#[test]
fn pty_ignore_symbols() {
  use std::io::{Read, Write};
  run_pty_test(|master| {
    master.write_all(b"Array.Symbol\t\n").unwrap();
    master.write_all(b"close();\n").unwrap();

    let mut output = String::new();
    master.read_to_string(&mut output).unwrap();

    assert!(output.contains("undefined"));
    assert!(
      !output.contains("Uncaught TypeError: Array.Symbol is not a function")
    );
  });
}

#[cfg(unix)]
fn run_pty_test(mut run: impl FnMut(&mut util::pty::fork::Master)) {
  use util::pty::fork::*;
  let deno_exe = util::deno_exe_path();
  let fork = Fork::from_ptmx().unwrap();
  if let Ok(mut master) = fork.is_parent() {
    run(&mut master);
    fork.wait().unwrap();
  } else {
    std::env::set_var("NO_COLOR", "1");
    let err = exec::Command::new(deno_exe).arg("repl").exec();
    println!("err {}", err);
    unreachable!()
  }
}

#[test]
fn console_log() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["console.log('hello')", "'world'"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("hello\nundefined\n\"world\"\n"));
  assert!(err.is_empty());
}

#[test]
fn object_literal() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["{}", "{ foo: 'bar' }"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("{}\n{ foo: \"bar\" }\n"));
  assert!(err.is_empty());
}

#[test]
fn block_expression() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["{};", "{\"\"}"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n\"\"\n"));
  assert!(err.is_empty());
}

#[test]
fn await_resolve() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["await Promise.resolve('done')"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("\"done\"\n"));
  assert!(err.is_empty());
}

#[test]
fn await_timeout() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["await new Promise((r) => setTimeout(r, 0, 'done'))"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("\"done\"\n"));
  assert!(err.is_empty());
}

#[test]
fn let_redeclaration() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["let foo = 0;", "foo", "let foo = 1;", "foo"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n0\nundefined\n1\n"));
  assert!(err.is_empty());
}

#[test]
fn repl_cwd() {
  let (_out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["Deno.cwd()"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(err.is_empty());
}

#[test]
fn typescript() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      "function add(a: number, b: number) { return a + b }",
      "const result: number = add(1, 2) as number;",
      "result",
    ]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\nundefined\n3\n"));
  assert!(err.is_empty());
}

#[test]
fn typescript_declarations() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      "namespace Test { export enum Values { A, B, C } }",
      "Test.Values.A",
      "Test.Values.C",
      "interface MyInterface { prop: string; }",
      "type MyTypeAlias = string;",
    ]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n0\n2\nundefined\nundefined\n"));
  assert!(err.is_empty());
}

#[test]
fn typescript_decorators() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      "function dec(target) { target.prototype.test = () => 2; }",
      "@dec class Test {}",
      "new Test().test()",
    ]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\nundefined\n2\n"));
  assert!(err.is_empty());
}

#[test]
fn eof() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1 + 2"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("3\n"));
  assert!(err.is_empty());
}

#[test]
fn strict() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      "let a = {};",
      "Object.preventExtensions(a);",
      "a.c = 1;",
    ]),
    None,
    false,
  );
  assert!(out.contains(
    "Uncaught TypeError: Cannot add property c, object is not extensible"
  ));
  assert!(err.is_empty());
}

#[test]
fn close_command() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["close()", "'ignored'"]),
    None,
    false,
  );

  assert!(!out.contains("ignored"));
  assert!(err.is_empty());
}

#[test]
fn function() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["Deno.writeFileSync"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("[Function: writeFileSync]\n"));
  assert!(err.is_empty());
}

#[test]
#[ignore]
fn multiline() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["(\n1 + 2\n)"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("3\n"));
  assert!(err.is_empty());
}

#[test]
fn import() {
  let (out, _) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["import('./subdir/auto_print_hello.ts')"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.contains("hello!\n"));
}

#[test]
fn import_declarations() {
  let (out, _) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["import './subdir/auto_print_hello.ts';"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.contains("hello!\n"));
}

#[test]
fn exports_stripped() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["export default 5;", "export class Test {}"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.contains("5\n"));
  assert!(err.is_empty());
}

#[test]
fn call_eval_unterminated() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["eval('{')"]),
    None,
    false,
  );
  assert!(out.contains("Unexpected end of input"));
  assert!(err.is_empty());
}

#[test]
fn unpaired_braces() {
  for right_brace in &[")", "]", "}"] {
    let (out, err) = util::run_and_collect_output(
      true,
      "repl",
      Some(vec![right_brace]),
      None,
      false,
    );
    assert!(out.contains("Unexpected token"));
    assert!(err.is_empty());
  }
}

#[test]
fn reference_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["not_a_variable"]),
    None,
    false,
  );
  assert!(out.contains("not_a_variable is not defined"));
  assert!(err.is_empty());
}

#[test]
fn syntax_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      "syntax error",
      "2", // ensure it keeps accepting input after
    ]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("parse error: Expected ';', '}' or <eof> at 1:7\n2\n"));
  assert!(err.is_empty());
}

#[test]
fn syntax_error_jsx() {
  // JSX is not supported in the REPL
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["const element = <div />;"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.contains("Unexpected token `>`"));
  assert!(err.is_empty());
}

#[test]
fn type_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["console()"]),
    None,
    false,
  );
  assert!(out.contains("console is not a function"));
  assert!(err.is_empty());
}

#[test]
fn variable() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["var a = 123;", "a"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n123\n"));
  assert!(err.is_empty());
}

#[test]
fn lexical_scoped_variable() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["let a = 123;", "a"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("undefined\n123\n"));
  assert!(err.is_empty());
}

#[test]
fn missing_deno_dir() {
  use std::fs::{read_dir, remove_dir_all};
  const DENO_DIR: &str = "nonexistent";
  let test_deno_dir = test_util::testdata_path().join(DENO_DIR);
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1"]),
    Some(vec![
      ("DENO_DIR".to_owned(), DENO_DIR.to_owned()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert!(read_dir(&test_deno_dir).is_ok());
  remove_dir_all(&test_deno_dir).unwrap();
  assert!(out.ends_with("1\n"));
  assert!(err.is_empty());
}

#[test]
fn save_last_eval() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1", "_"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("1\n1\n"));
  assert!(err.is_empty());
}

#[test]
fn save_last_thrown() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["throw 1", "_error"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(out.ends_with("Uncaught 1\n1\n"));
  assert!(err.is_empty());
}

#[test]
fn assign_underscore() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["_ = 1", "2", "_"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(
    out.ends_with("Last evaluation result is no longer saved to _.\n1\n2\n1\n")
  );
  assert!(err.is_empty());
}

#[test]
fn assign_underscore_error() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["_error = 1", "throw 2", "_error"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  println!("{}", out);
  assert!(out.ends_with(
    "Last thrown error is no longer saved to _error.\n1\nUncaught 2\n1\n"
  ));
  assert!(err.is_empty());
}

#[test]
fn custom_inspect() {
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec![
      r#"const o = {
        [Symbol.for("Deno.customInspect")]() {
          throw new Error('Oops custom inspect error');
        },
      };"#,
      "o",
    ]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );

  assert!(out.contains("Oops custom inspect error"));
  assert!(err.is_empty());
}

#[test]
fn eval_flag_valid_input() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval", "const t = 10;"],
    Some(vec!["t * 500;"]),
    None,
    false,
  );
  assert!(out.contains("5000"));
  assert!(err.is_empty());
}

#[test]
fn eval_flag_parse_error() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval", "const %"],
    Some(vec!["250 * 10"]),
    None,
    false,
  );
  assert!(test_util::strip_ansi_codes(&out)
    .contains("error in --eval flag. parse error: Unexpected token `%`."));
  assert!(out.contains("2500")); // should not prevent input
  assert!(err.is_empty());
}

#[test]
fn eval_flag_runtime_error() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval", "throw new Error('Testing')"],
    Some(vec!["250 * 10"]),
    None,
    false,
  );
  assert!(out.contains("error in --eval flag. Uncaught Error: Testing"));
  assert!(out.contains("2500")); // should not prevent input
  assert!(err.is_empty());
}
