// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use test_util as util;
use test_util::assert_contains;
use test_util::assert_ends_with;
use test_util::assert_not_contains;
use util::TempDir;
use util::TestContext;
use util::TestContextBuilder;

#[test]
fn pty_multiline() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("(\n1 + 2\n)");
    console.expect("3");
    console.write_line("{\nfoo: \"foo\"\n}");
    console.expect("{ foo: \"foo\" }");
    console.write_line("`\nfoo\n`");
    console.expect("\"\\nfoo\\n\"");
    console.write_line("`\n\\`\n`");
    console.expect(r#""\n`\n""#);
    console.write_line("'{'");
    console.expect(r#""{""#);
    console.write_line("'('");
    console.expect(r#""(""#);
    console.write_line("'['");
    console.expect(r#""[""#);
    console.write_line("/{/");
    console.expect("/{/");
    console.write_line("/\\(/");
    console.expect("/\\(/");
    console.write_line("/\\[/");
    console.expect("/\\[/");
    console.write_line("console.log(\"{test1} abc {test2} def {{test3}}\".match(/{([^{].+?)}/));");
    console.expect("[");
    console.expect("  \"{test1}\",");
    console.expect("  \"test1\",");
    console.expect("  index: 0,");
    console.expect("  input: \"{test1} abc {test2} def {{test3}}\",");
    console.expect("  groups: undefined");
    console.expect("]");
  });
}

#[test]
fn pty_null() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("null");
    console.expect("null");
  });
}

#[test]
fn pty_unpaired_braces() {
  for right_brace in &[")", "]", "}"] {
    util::with_pty(&["repl"], |mut console| {
      console.write_line(right_brace);
      console.expect("parse error: Expression expected");
    });
  }
}

#[test]
fn pty_bad_input() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("'\\u{1f3b5}'[0]");
    console.expect("Unterminated string literal");
  });
}

#[test]
fn pty_syntax_error_input() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("('\\u')");
    console.expect("Bad character escape sequence, expected 4 hex characters");

    console.write_line("'");
    console.expect("Unterminated string constant");

    console.write_line("[{'a'}];");
    console.expect("Expected a semicolon");
  });
}

#[test]
fn pty_complete_symbol() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line_raw("Symbol.it\t");
    console.expect("Symbol(Symbol.iterator)");
  });
}

#[test]
fn pty_complete_declarations() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("class MyClass {}");
    console.expect("undefined");
    console.write_line_raw("My\t");
    console.expect("[class MyClass]");
    console.write_line("let myVar = 2 + 3;");
    console.expect("undefined");
    console.write_line_raw("myV\t");
    console.expect("5");
  });
}

#[test]
fn pty_complete_primitives() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("let func = function test(){}");
    console.expect("undefined");
    console.write_line_raw("func.appl\t");
    console.expect("func.apply");
    console.write_line("let str = ''");
    console.expect("undefined");
    console.write_line_raw("str.leng\t");
    console.expect("str.length");
    console.write_line_raw("false.valueO\t");
    console.expect("false.valueOf");
    console.write_line_raw("5n.valueO\t");
    console.expect("5n.valueOf");
    console.write_line("let num = 5");
    console.expect("undefined");
    console.write_line_raw("num.toStrin\t");
    console.expect("num.toString");
  });
}

#[test]
fn pty_complete_expression() {
  util::with_pty(&["repl"], |mut console| {
    console.write_raw("Deno.\t\t");
    console.expect("Display all");
    console.write_raw("y");
    console.expect_all(&["symlink", "args", "permissions", "exit"]);
  });
}

#[test]
fn pty_complete_imports() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.create_dir_all("subdir");
  temp_dir.write("./subdir/my_file.ts", "");
  temp_dir.create_dir_all("run");
  temp_dir.write("./run/hello.ts", "console.log('Hello World');");
  temp_dir.write(
    "./run/output.ts",
    r#"export function output(text: string) {
  console.log(text);
}
"#,
  );
  context
    .new_command()
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      // single quotes
      console.write_line_raw("import './run/hel\t'");
      console.expect("Hello World");
      // double quotes
      console.write_line_raw("import { output } from \"./run/out\t\"");
      console.expect("\"./run/output.ts\"");
      console.write_line_raw("output('testing output');");
      console.expect("testing output");
    });

  // ensure when the directory changes that the suggestions come from the cwd
  context
    .new_command()
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      console.write_line("Deno.chdir('./subdir');");
      console.expect("undefined");
      console.write_line_raw("import '../run/he\t'");
      console.expect("Hello World");
    });
}

#[test]
fn pty_complete_imports_no_panic_empty_specifier() {
  // does not panic when tabbing when empty
  util::with_pty(&["repl", "-A"], |mut console| {
    if cfg!(windows) {
      console.write_line_raw("import '\t'");
      console.expect_any(&["not prefixed with", "https://deno.land"]);
    } else {
      console.write_raw("import '\t");
      console.expect("import 'https://deno.land");
    }
  });
}

#[test]
fn pty_ignore_symbols() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line_raw("Array.Symbol\t");
    console.expect("undefined");
  });
}

#[test]
fn pty_assign_global_this() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("globalThis = 40 + 2;");
    console.expect("42");
  });
}

#[test]
fn pty_assign_deno_keys_and_deno() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line(
      "Object.keys(Deno).forEach((key)=>{try{Deno[key] = undefined} catch {}})",
    );
    console.expect("undefined");
    console.write_line("delete globalThis.Deno");
    console.expect("true");
    console.write_line("console.log('testing ' + 'this out');");
    console.expect("testing this out");
    console.expect("undefined");
  });
}

#[test]
fn pty_internal_repl() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("'Length: ' + Object.keys(globalThis).filter(k => k.startsWith('__DENO_')).length;");
    console.expect("Length: 0");

    console.write_line_raw("__\t\t");
    console.expect("> __");
    let output = console.read_until("> __");
    assert_contains!(output, "__defineGetter__");
    // should not contain the internal repl variable
    // in the `globalThis` or completions output
    assert_not_contains!(output, "__DENO_");
  });
}

#[test]
fn pty_emoji() {
  // windows was having issues displaying this
  util::with_pty(&["repl"], |mut console| {
    console.write_line(r"console.log('\u{1F995}');");
    console.expect("🦕");
  });
}

#[test]
fn console_log() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console.log('hello');");
    console.expect("hello");
    console.write_line("'world'");
    console.expect("\"world\"");
  });
}

#[test]
fn object_literal() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("{}");
    console.expect("{}");
    console.write_line("{   foo: 'bar'   }");
    console.expect("{ foo: \"bar\" }");
  });
}

#[test]
fn block_expression() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("{};");
    console.expect("undefined");
    console.write_line("{\"\"}");
    console.expect("\"\"");
  });
}

#[test]
fn await_resolve() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("await Promise.resolve('done')");
    console.expect("\"done\"");
  });
}

#[test]
fn await_timeout() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("await new Promise((r) => setTimeout(r, 0, 'done'))");
    console.expect("\"done\"");
  });
}

#[test]
fn let_redeclaration() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("let foo = 0;");
    console.expect("undefined");
    console.write_line("foo");
    console.expect("0");
    console.write_line("let foo = 1;");
    console.expect("undefined");
    console.write_line("foo");
    console.expect("1");
  });
}

#[test]
fn repl_cwd() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  context
    .new_command()
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      console.write_line("Deno.cwd()");
      console.expect(
        temp_dir
          .path()
          .as_path()
          .file_name()
          .unwrap()
          .to_str()
          .unwrap(),
      );
    });
}

#[test]
fn typescript() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("function add(a: number, b: number) { return a + b }");
    console.expect("undefined");
    console.write_line("const result: number = add(1, 2) as number;");
    console.expect("undefined");
    console.write_line("result");
    console.expect("3");
  });
}

#[test]
fn typescript_declarations() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("namespace Test { export enum Values { A, B, C } }");
    console.expect("undefined");
    console.write_line("Test.Values.A");
    console.expect("0");
    console.write_line("Test.Values.C");
    console.expect("2");
    console.write_line("interface MyInterface { prop: string; }");
    console.expect("undefined");
    console.write_line("type MyTypeAlias = string;");
    console.expect("undefined");
  });
}

#[test]
fn typescript_decorators() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write(
    "./deno.json",
    r#"{ "compilerOptions": { "experimentalDecorators": true } }"#,
  );
  let config_path = temp_dir.target_path().join("./deno.json");
  util::with_pty(
    &["repl", "--config", config_path.to_string_lossy().as_ref()],
    |mut console| {
      console.write_line(
        "function dec(target) { target.prototype.test = () => 2; }",
      );
      console.expect("undefined");
      console.write_line("@dec class Test {}");
      console.expect("[class Test]");
      console.write_line("new Test().test()");
      console.expect("2");
    },
  );
}

#[test]
fn eof() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("1 + 2");
    console.expect("3");
  });
}

#[test]
fn strict() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("let a = {};");
    console.expect("undefined");
    console.write_line("Object.preventExtensions(a)");
    console.expect("{}");
    console.write_line("a.c = 1;");
    console.expect(
      "Uncaught TypeError: Cannot add property c, object is not extensible",
    );
  });
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

  assert_not_contains!(out, "ignored");
  assert!(err.is_empty());
}

#[test]
fn function() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("Deno.writeFileSync");
    console.expect("[Function: writeFileSync]");
  });
}

#[test]
fn multiline() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("(\n1 + 2\n)");
    console.expect("3");
  });
}

#[test]
fn import() {
  let context = TestContextBuilder::default()
    .use_copy_temp_dir("./subdir")
    .build();
  context
    .new_command()
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      console.write_line("import('./subdir/auto_print_hello.ts')");
      console.expect("hello!");
    });
}

#[test]
fn import_declarations() {
  let context = TestContextBuilder::default()
    .use_copy_temp_dir("./subdir")
    .build();
  context
    .new_command()
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      console.write_line("import './subdir/auto_print_hello.ts'");
      console.expect("hello!");
    });
}

#[test]
fn exports_stripped() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("const test = 5 + 1; export default test;");
    console.expect("6");
    console.write_line("export class Test {}");
    console.expect("undefined");
  });
}

#[test]
fn call_eval_unterminated() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("eval('{')");
    console.expect("Unexpected end of input");
  });
}

#[test]
fn unpaired_braces() {
  util::with_pty(&["repl"], |mut console| {
    for right_brace in &[")", "]", "}"] {
      console.write_line(right_brace);
      console.expect("Expression expected");
    }
  });
}

#[test]
fn reference_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("not_a_variable");
    console.expect("not_a_variable is not defined");
  });
}

#[test]
fn syntax_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("syntax error");
    console.expect("parse error: Expected ';', '}' or <eof>");
    // ensure it keeps accepting input after
    console.write_line("7 * 6");
    console.expect("42");
  });
}

#[test]
fn jsx_errors_without_pragma() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("const element = <div />;");
    console.expect("React is not defined");
  });
}

#[test]
fn jsx_import_source() {
  let context = TestContextBuilder::default()
    .use_temp_cwd()
    .use_http_server()
    .build();
  context
    .new_command()
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      console.write_line("/** @jsxImportSource http://localhost:4545/jsx */");
      console.expect("undefined");
      console.write_line("const element = <div />;");
      console.expect("undefined");
    });
}

#[test]
fn type_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console()");
    console.expect("console is not a function");
  });
}

#[test]
fn variable() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("var a = 123 + 456;");
    console.expect("undefined");
    console.write_line("a");
    console.expect("579");
  });
}

#[test]
fn lexical_scoped_variable() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("let a = 123 + 456;");
    console.expect("undefined");
    console.write_line("a");
    console.expect("579");
  });
}

#[test]
fn missing_deno_dir() {
  use std::fs::read_dir;
  let temp_dir = TempDir::new();
  let deno_dir_path = temp_dir.path().join("deno");
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1"]),
    Some(vec![
      ("DENO_DIR".to_owned(), deno_dir_path.to_string()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert!(read_dir(deno_dir_path).is_ok());
  assert_ends_with!(out, "1\n");
  assert!(err.is_empty());
}

#[test]
fn custom_history_path() {
  use std::fs::read;
  let temp_dir = TempDir::new();
  let history_path = temp_dir.path().join("history.txt");
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1"]),
    Some(vec![
      ("DENO_REPL_HISTORY".to_owned(), history_path.to_string()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert!(read(&history_path).is_ok());
  assert_ends_with!(out, "1\n");
  assert!(err.is_empty());
}

#[test]
fn disable_history_file() {
  let deno_dir = util::new_deno_dir();
  let default_history_path = deno_dir.path().join("deno_history.txt");
  let (out, err) = util::run_and_collect_output(
    true,
    "repl",
    Some(vec!["1"]),
    Some(vec![
      ("DENO_DIR".to_owned(), deno_dir.path().to_string()),
      ("DENO_REPL_HISTORY".to_owned(), "".to_owned()),
      ("NO_COLOR".to_owned(), "1".to_owned()),
    ]),
    false,
  );
  assert!(!default_history_path.try_exists().unwrap());
  assert_ends_with!(out, "1\n");
  assert!(err.is_empty());
}

#[test]
fn save_last_eval() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("1 + 2");
    console.expect("3");
    console.write_line("_ + 3");
    console.expect("6");
  });
}

#[test]
fn save_last_thrown() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("throw 1 + 2");
    console.expect("Uncaught 3");
    console.write_line("_error + 3");
    console.expect("6");
  });
}

#[test]
fn assign_underscore() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("_ = 1");
    console.expect("Last evaluation result is no longer saved to _.");
    console.write_line("2 + 3");
    console.expect("5");
    console.write_line("_");
    console.expect("1");
  });
}

#[test]
fn assign_underscore_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("_error = 1");
    console.expect("Last thrown error is no longer saved to _error.");
    console.write_line("throw 2");
    console.expect("Uncaught 2");
    console.write_line("_error");
    console.expect("1");
  });
}

#[test]
fn custom_inspect() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line(
      r#"const o = {
      [Symbol.for("Deno.customInspect")]() {
        throw new Error('Oops custom inspect error');
      },
    };"#,
    );
    console.expect("undefined");
    console.write_line("o");
    console.expect("Oops custom inspect error");
  });
}

#[test]
fn eval_flag_valid_input() {
  util::with_pty(&["repl", "--eval", "const t = 10;"], |mut console| {
    console.write_line("t * 500");
    console.expect("5000");
  });
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
  assert_contains!(
    test_util::strip_ansi_codes(&out),
    "Error in --eval flag: parse error: Unexpected token `%`."
  );
  assert_contains!(out, "2500"); // should not prevent input
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
  assert_contains!(out, "Error in --eval flag: Uncaught Error: Testing");
  assert_contains!(out, "2500"); // should not prevent input
  assert!(err.is_empty());
}

#[test]
fn eval_file_flag_valid_input() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval-file=./run/001_hello.js"],
    None,
    None,
    false,
  );
  assert_contains!(out, "Hello World");
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
  assert_contains!(out, "hello");
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
  assert_contains!(out, "hello");
  assert!(err.contains("Download"));
}

#[test]
fn eval_file_flag_multiple_files() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--allow-read", "--eval-file=http://127.0.0.1:4545/repl/import_type.ts,./tsc/d.ts,http://127.0.0.1:4545/type_definitions/foo.js"],
    Some(vec!["b.method1=v4", "b.method1()+foo.toUpperCase()"]),
    None,
    true,
  );
  assert_contains!(out, "helloFOO");
  assert_contains!(err, "Download");
}

#[flaky_test::flaky_test]
fn pty_clear_function() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console.log('h' + 'ello');");
    console.expect_all(&["hello", "undefined"]);
    console.write_line_raw("clear();");
    if cfg!(windows) {
      // expect a bunch of these in the output
      console.expect_raw_in_current_output(
        "\r\n\u{1b}[K\r\n\u{1b}[K\r\n\u{1b}[K\r\n\u{1b}[K\r\n\u{1b}[K",
      );
    } else {
      console.expect_raw_in_current_output("[1;1H");
    }
    console.expect("undefined"); // advance past the "clear()"'s undefined
    console.expect(">");
    console.write_line("const clear = 1234 + 2000;");
    console.expect("undefined");
    console.write_line("clear;");
    console.expect("3234");
  });
}

#[test]
fn pty_tab_handler() {
  // If the last character is **not** whitespace, we show the completions
  util::with_pty(&["repl"], |mut console| {
    console.write_raw("a\t\t");
    console.expect_all(&["addEventListener", "alert", "atob"]);
  });
  // If the last character is whitespace, we just insert a tab
  util::with_pty(&["repl"], |mut console| {
    console.write_line("const a = 5;");
    console.expect("undefined");
    console.write_raw("a; \t\ta + 2;\n"); // last character is whitespace
    console.expect_any(&[
      // windows
      "a;         a + 2;",
      // unix
      "a; \t\ta + 2;",
    ]);
  });
}

#[test]
fn repl_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console.log(1);");
    console.expect_all(&["1", "undefined"]);
    console.write_line(r#"throw new Error("foo");"#);
    console.expect("Uncaught Error: foo");
    console.expect("    at <anonymous>");
    console.write_line("console.log(2);");
    console.expect("2");
  });
}

#[flaky_test::flaky_test]
fn repl_reject() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console.log(1);");
    console.expect_all(&["1", "undefined"]);
    console.write_line(r#"Promise.reject(new Error("foo"));"#);
    console.expect("Promise {");
    console.expect("  <rejected> Error: foo");
    console.expect("Uncaught Error: foo");
    console.expect("    at <anonymous>");
    console.write_line("console.log(2);");
    console.expect("2");
    console.write_line(r#"throw "hello";"#);
    console.expect(r#"Uncaught "hello""#);
    console.write_line(r#"throw `hello ${"world"}`;"#);
    console.expect(r#"Uncaught "hello world""#);
  });
}

#[flaky_test::flaky_test]
fn repl_report_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("console.log(1);");
    console.expect_all(&["1", "undefined"]);
    console.write_line(r#"reportError(new Error("foo"));"#);
    console.expect("undefined");
    console.expect("Uncaught Error: foo");
    console.expect("    at <anonymous>");
    console.write_line("console.log(2);");
    console.expect("2");
  });
}

#[flaky_test::flaky_test]
fn repl_error_undefined() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line(r#"throw undefined;"#);
    console.expect("Uncaught undefined");
    console.write_line(r#"Promise.reject();"#);
    console.expect("Promise { <rejected> undefined }");
    console.expect("Uncaught undefined");
    console.write_line(r#"reportError(undefined);"#);
    console.expect("undefined");
    console.expect("Uncaught undefined");
  });
}

#[test]
fn pty_aggregate_error() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("await Promise.any([])");
    console.expect("AggregateError");
  });
}

#[test]
fn repl_with_quiet_flag() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--quiet"],
    Some(vec!["await Promise.resolve('done')"]),
    Some(vec![("NO_COLOR".to_owned(), "1".to_owned())]),
    false,
  );
  assert!(!out.contains("Deno"));
  assert!(!out.contains("exit using ctrl+d, ctrl+c, or close()"));
  assert_ends_with!(out, "\"done\"\n");
  assert!(err.is_empty(), "Error: {}", err);
}

#[test]
fn repl_deno_test() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line_raw(
      "\
        console.log('Hello from outside of test!'); \
        Deno.test('test1', async (t) => { \
          console.log('Hello from inside of test!'); \
          await t.step('step1', () => {}); \
        }); \
        Deno.test('test2', () => { \
          throw new Error('some message'); \
        }); \
        console.log('Hello again from outside of test!'); \
      ",
    );

    console.expect("Hello from outside of test!");
    console.expect("Hello again from outside of test!");
    // FIXME(nayeemrmn): REPL unit tests don't support output capturing.
    console.expect("Hello from inside of test!");
    console.expect("  step1 ... ok (");
    console.expect("test1 ... ok (");
    console.expect("test2 ... FAILED (");
    console.expect("ERRORS");
    console.expect("test2 => <anonymous>:6:6");
    console.expect("error: Error: some message");
    console.expect("   at <anonymous>:7:9");
    console.expect("FAILURES");
    console.expect("test2 => <anonymous>:6:6");
    console.expect("FAILED | 1 passed (1 step) | 1 failed (");
    console.expect("undefined");

    console.write_line("Deno.test('test2', () => {});");

    console.expect("test2 ... ok (");
    console.expect("ok | 1 passed | 0 failed (");
    console.expect("undefined");
  });
}

#[test]
fn npm_packages() {
  let mut env_vars = util::env_vars_for_npm_tests();
  env_vars.push(("NO_COLOR".to_owned(), "1".to_owned()));
  let temp_dir = TempDir::new();
  env_vars.push(("DENO_DIR".to_string(), temp_dir.path().to_string()));

  {
    let (out, err) = util::run_and_collect_output_with_args(
      true,
      vec!["repl", "--quiet", "--allow-read", "--allow-env"],
      Some(vec![
        r#"import chalk from "npm:chalk";"#,
        "chalk.red('hel' + 'lo')",
      ]),
      Some(env_vars.clone()),
      true,
    );

    assert_contains!(out, "hello");
    assert!(err.is_empty(), "Error: {}", err);
  }

  {
    let (out, err) = util::run_and_collect_output_with_args(
      true,
      vec!["repl", "--quiet", "--allow-read", "--allow-env"],
      Some(vec![
        r#"const chalk = await import("npm:chalk");"#,
        "chalk.default.red('hel' + 'lo')",
      ]),
      Some(env_vars.clone()),
      true,
    );

    assert_contains!(out, "hello");
    assert!(err.is_empty(), "Error: {}", err);
  }

  {
    let (out, err) = util::run_and_collect_output_with_args(
      true,
      vec!["repl", "--quiet", "--allow-read", "--allow-env"],
      Some(vec![r#"export {} from "npm:chalk";"#]),
      Some(env_vars.clone()),
      true,
    );

    assert_contains!(out, "[Module: null prototype] {");
    assert_contains!(out, "Chalk: [class Chalk],");
    assert!(err.is_empty(), "Error: {}", err);
  }

  {
    let (out, err) = util::run_and_collect_output_with_args(
      true,
      vec!["repl", "--quiet", "--allow-read", "--allow-env"],
      Some(vec![r#"import foo from "npm:asdfawe52345asdf""#]),
      Some(env_vars.clone()),
      true,
    );

    assert_contains!(
      out,
      "error: npm package 'asdfawe52345asdf' does not exist"
    );
    assert!(err.is_empty(), "Error: {}", err);
  }

  {
    let (out, err) = util::run_and_collect_output_with_args(
      true,
      vec!["repl", "--quiet", "--allow-read", "--allow-env"],
      Some(vec![
        "import path from 'node:path';",
        "path.isGlob('asdf') ? 'yes' : 'no'",
      ]),
      Some(env_vars.clone()),
      true,
    );

    assert_contains!(out, "no");
    assert!(err.is_empty(), "Error: {}", err);
  }
}

#[test]
fn pty_tab_indexable_props() {
  util::with_pty(&["repl"], |mut console| {
    console.write_line("const arr = [1, 2, 3]");
    console.expect("undefined");
    console.write_raw("arr.\t\t");
    console.expect("> arr.");
    let output = console.read_until("> arr.");
    assert_contains!(output, "constructor");
    assert_contains!(output, "sort");
    assert_contains!(output, "at");
    assert_not_contains!(output, "0", "1", "2");
  });
}

// TODO(2.0): this should first run `deno install`
#[flaky_test::flaky_test]
#[ignore]
fn package_json_uncached_no_error() {
  let test_context = TestContextBuilder::for_npm()
    .use_temp_cwd()
    .use_http_server()
    .env("RUST_BACKTRACE", "1")
    .build();
  let temp_dir = test_context.temp_dir();
  temp_dir.write(
    "package.json",
    r#"{
  "dependencies": {
    "@denotest/esm-basic": "1.0.0"
  }
}
"#,
  );
  test_context.new_command().with_pty(|mut console| {
    console.write_line("console.log(123 + 456);");
    console.expect_all(&["579", "undefined"]);
    assert_not_contains!(
      console.all_output(),
      "Could not set npm package requirements",
    );

    // should support getting the package now though
    console
      .write_line("import { getValue, setValue } from '@denotest/esm-basic';");
    console.expect_all(&["undefined", "Download"]);
    console.write_line("setValue(12 + 30);");
    console.expect("undefined");
    console.write_line("getValue()");
    console.expect("42");

    assert!(temp_dir.path().join("node_modules").exists());
  });
}

#[test]
fn closed_file_pre_load_does_not_occur() {
  TestContext::default()
    .new_command()
    .args_vec(["repl", "-A", "--log-level=debug"])
    .with_pty(|console| {
      assert_contains!(
        console.all_output(),
        "Skipped workspace walk due to client incapability.",
      );
    });
}

#[test]
fn env_file() {
  TestContext::default()
    .new_command()
    .args_vec([
      "repl",
      "--env=env",
      "--allow-env",
      "--eval",
      "console.log(Deno.env.get('FOO'))",
    ])
    .with_pty(|console| {
      assert_contains!(console.all_output(), "BAR",);
    });
}

// Regression test for https://github.com/denoland/deno/issues/20528
#[test]
fn pty_promise_was_collected_regression_test() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl"],
    Some(vec!["new Uint8Array(64 * 1024 * 1024)"]),
    None,
    false,
  );

  assert_contains!(out, "Uint8Array(67108864)");
  assert!(err.is_empty());
}

#[test]
fn eval_file_promise_error() {
  let (out, err) = util::run_and_collect_output_with_args(
    true,
    vec!["repl", "--eval-file=./repl/promise_rejection.ts"],
    None,
    None,
    false,
  );
  assert_contains!(out, "Uncaught undefined");
  assert!(err.is_empty());
}

#[test]
fn repl_json_imports() {
  let context = TestContextBuilder::default().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  temp_dir.write("./data.json", r#"{"hello": "world"}"#);
  context
    .new_command()
    .env("NO_COLOR", "1")
    .args_vec(["repl", "-A"])
    .with_pty(|mut console| {
      console.write_line_raw(
        "import data from './data.json' with { type: 'json' };",
      );
      console.expect("undefined");
      console.write_line_raw("data");
      console.expect(r#"{ hello: "world" }"#);
    });
}
