// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

///!
///! Integration test for the Deno Language Server (`deno lsp`)
///!
use std::fs;
use std::io::Read;
use std::io::Write;
use std::process::Stdio;
use test_util;

struct LspIntegrationTest {
  pub fixtures: Vec<&'static str>,
}

impl LspIntegrationTest {
  pub fn run(&self) -> (String, String) {
    let root_path = test_util::root_path();
    let deno_exe = test_util::deno_exe_path();
    let tests_dir = root_path.join("cli/tests/lsp");
    println!("tests_dir: {:?} deno_exe: {:?}", tests_dir, deno_exe);
    let mut command = test_util::deno_cmd();
    command
      .arg("lsp")
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped());

    let mut process = command.spawn().expect("failed to execute deno");

    let mut stdin = process.stdin.take().unwrap();
    for fixture in &self.fixtures {
      let fixture_path = tests_dir.join(fixture);
      let content =
        fs::read_to_string(&fixture_path).expect("could not read fixture");
      let content_length = content.chars().count();
      write!(
        stdin,
        "Content-Length: {}\r\n\r\n{}",
        content_length, content
      )
      .unwrap();
    }
    drop(stdin);

    let mut so = String::new();
    process.stdout.unwrap().read_to_string(&mut so).unwrap();

    let mut se = String::new();
    process.stderr.unwrap().read_to_string(&mut se).unwrap();

    (so, se)
  }
}

#[test]
fn test_lsp_startup_shutdown() {
  let test = LspIntegrationTest {
    fixtures: vec![
      "initialize_request.json",
      "initialized_notification.json",
      "shutdown_request.json",
      "exit_notification.json",
    ],
  };
  let (response, out) = test.run();
  assert!(response.contains("deno-language-server"));
  assert!(out.contains("Connected to \"test-harness\" 1.0.0"));
}
