// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

use test_util as util;

#[test]
fn pty_multiline() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("(\n1 + 2\n)");
    console.write_line("{\nfoo: \"foo\"\n}");
    console.write_line("`\nfoo\n`");
    console.write_line("`\n\\`\n`");
    console.write_line("'{'");
    console.write_line("'('");
    console.write_line("'['");
    console.write_line("/{/");
    console.write_line("/\\(/");
    console.write_line("/\\[/");
    console.write_line("console.log(\"{test1} abc {test2} def {{test3}}\".match(/{([^{].+?)}/));");
    console.write_line("close();");

    let output = console.read_all_output();
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

#[test]
fn pty_null() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("null");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("null"));
  });
}

#[test]
fn pty_unpaired_braces() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line(")");
    console.write_line("]");
    console.write_line("}");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("Unexpected token `)`"));
    assert!(output.contains("Unexpected token `]`"));
    assert!(output.contains("Unexpected token `}`"));
  });
}

#[test]
fn pty_bad_input() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("'\\u{1f3b5}'[0]");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("Unterminated string literal"));
  });
}

#[test]
fn pty_syntax_error_input() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("('\\u')");
    console.write_line("'");
    console.write_line("[{'a'}];");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output
      .contains("Bad character escape sequence, expected 4 hex characters"));
    assert!(output.contains("Unterminated string constant"));
    assert!(output.contains("Expected a semicolon"));
  });
}

#[test]
fn pty_complete_symbol() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("Symbol.it\t");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("Symbol(Symbol.iterator)"));
  });
}

#[test]
fn pty_complete_declarations() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("class MyClass {}");
    console.write_line("My\t");
    console.write_line("let myVar;");
    console.write_line("myV\t");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("> MyClass"));
    assert!(output.contains("> myVar"));
  });
}

#[test]
fn pty_complete_primitives() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("let func = function test(){}");
    console.write_line("func.appl\t");
    console.write_line("let str = ''");
    console.write_line("str.leng\t");
    console.write_line("false.valueO\t");
    console.write_line("5n.valueO\t");
    console.write_line("let num = 5");
    console.write_line("num.toStrin\t");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("> func.apply"));
    assert!(output.contains("> str.length"));
    assert!(output.contains("> 5n.valueOf"));
    assert!(output.contains("> false.valueOf"));
    assert!(output.contains("> num.toString"));
  });
}

#[test]
fn pty_complete_expression() {
  util::with_pty(&["repl"], |mut console| {
    console.write_text("Deno.\t\t");
    console.write_text("y");
    console.write_line("");
    console.write_line("close();");
    let output = console.read_all_output();
    assert!(output.contains("Display all"));
    assert!(output.contains("core"));
    assert!(output.contains("args"));
    assert!(output.contains("exit"));
    assert!(output.contains("symlink"));
    assert!(output.contains("permissions"));
  });
}

#[test]
fn pty_complete_imports() {
  util::with_pty(&["repl"], |mut console| {
    // single quotes
    console.write_line("import './001_hel\t'");
    // double quotes
    console.write_line("import { output } from \"./045_out\t\"");
    console.write_line("output('testing output');");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("Hello World"));
    assert!(output.contains("testing output\u{1b}"));
  });

  // ensure when the directory changes that the suggestions come from the cwd
  util::with_pty(&["repl"], |mut console| {
    console.write_line("Deno.chdir('./subdir');");
    console.write_line("import '../001_hel\t'");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("Hello World"));
  });
}

#[test]
fn pty_ignore_symbols() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("Array.Symbol\t");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(output.contains("undefined"));
    assert!(
      !output.contains("Uncaught TypeError: Array.Symbol is not a function")
    );
  });
}

#[test]
fn pty_assign_global_this() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("globalThis = 42;");
    console.write_line("close();");

    let output = console.read_all_output();
    assert!(!output.contains("panicked"));
  });
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
  let expected_end_text = "undefined\n0\n2\nundefined\nundefined\n";
  assert_eq!(
    &out[out.len() - expected_end_text.len()..],
    expected_end_text
  );
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
  assert!(out.ends_with("parse error: Expected ';', '}' or <eof> at 1:8\n2\n"));
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

#[test]
fn eval_file_flag_valid_input() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval-file=./001_hello.js"],
    None,
    None,
    false,
  );
  assert!(out.contains("Hello World"));
  assert!(err.is_empty());
}

#[test]
fn eval_file_flag_call_defined_function() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval-file=./tsc/d.ts"],
    Some(vec!["v4()"]),
    None,
    false,
  );
  assert!(out.contains("hello"));
  assert!(err.is_empty());
}

#[test]
fn eval_file_flag_http_input() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval-file=http://127.0.0.1:4545/tsc/d.ts"],
    Some(vec!["v4()"]),
    None,
    true,
  );
  assert!(out.contains("hello"));
  assert!(err.contains("Download"));
}

#[test]
fn eval_file_flag_multiple_files() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval-file=http://127.0.0.1:4545/import_type.ts,./tsc/d.ts,http://127.0.0.1:4545/type_definitions/foo.js"],
    Some(vec!["b.method1=v4", "b.method1()+foo.toUpperCase()"]),
    None,
    true,
  );
  assert!(out.contains("helloFOO"));
  assert!(err.contains("Download"));
}

#[test]
fn pty_clear_function() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console.log('hello');");
    console.write_line("clear();");
    console.write_line("const clear = 1234 + 2000;");
    console.write_line("clear;");
    console.write_line("close();");

    let output = console.read_all_output();
    if cfg!(windows) {
      // Windows will overwrite what's in the console buffer before
      // we read from it. It contains this string repeated many times
      // to clear the screen.
      assert!(output.contains("\r\n\u{1b}[K\r\n\u{1b}[K\r\n\u{1b}[K"));
    } else {
      assert!(output.contains("hello"));
      assert!(output.contains("[1;1H"));
    }
    assert!(output.contains("undefined"));
    assert!(output.contains("const clear = 1234 + 2000;"));
    assert!(output.contains("3234"));
  });
}
