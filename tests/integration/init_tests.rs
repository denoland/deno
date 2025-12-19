// Copyright 2018-2025 the Deno authors. MIT license.

use test_util as util;
use test_util::test;
use util::TestContextBuilder;
use util::assert_contains;

#[test]
fn init_subcommand_without_dir() {
  let context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  let output = context.new_command().args("init").split_output().run();

  output.assert_exit_code(0);

  let stderr = output.stderr();
  assert_contains!(stderr, "Project initialized");
  assert!(!stderr.contains("cd"));
  assert_contains!(stderr, "deno run main.ts");
  assert_contains!(stderr, "deno task dev");
  assert_contains!(stderr, "deno test");

  assert!(cwd.join("deno.json").exists());

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("run main.ts")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_eq!(output.stdout().as_bytes(), b"Add 2 + 3 = 5\n");

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("test")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1 passed");
  output.skip_output_check();
}

#[test]
fn init_subcommand_with_dir_arg() {
  let context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  let output = context
    .new_command()
    .args("init my_dir")
    .split_output()
    .run();

  output.assert_exit_code(0);

  let stderr = output.stderr();
  assert_contains!(stderr, "Project initialized");
  assert_contains!(stderr, "cd my_dir");
  assert_contains!(stderr, "deno run main.ts");
  assert_contains!(stderr, "deno task dev");
  assert_contains!(stderr, "deno test");

  assert!(cwd.join("my_dir/deno.json").exists());

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("run my_dir/main.ts")
    .split_output()
    .run();

  output.assert_exit_code(0);

  assert_eq!(output.stdout().as_bytes(), b"Add 2 + 3 = 5\n");
  output.skip_output_check();

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .current_dir("my_dir")
    .args("test main_test.ts")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1 passed");
  output.skip_output_check();
}

#[test]
fn init_subcommand_with_quiet_arg() {
  let context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  let output = context
    .new_command()
    .args("init --quiet")
    .split_output()
    .run();

  output.assert_exit_code(0);

  assert_eq!(output.stdout(), "");
  assert!(cwd.join("deno.json").exists());

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("run main.ts")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_eq!(output.stdout().as_bytes(), b"Add 2 + 3 = 5\n");
  output.skip_output_check();

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("test")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "1 passed");
  output.skip_output_check();
}

#[test]
fn init_subcommand_with_existing_file() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  cwd
    .join("main.ts")
    .write("console.log('Log from main.ts that already exists');");

  let output = context.new_command().args("init").split_output().run();

  output.assert_exit_code(0);
  output.assert_stderr_matches_text(
    "ℹ️ Skipped creating main.ts as it already exists
✅ Project initialized

Run these commands to get started

  # Run the program
  deno run main.ts

  # Run the program and watch for file changes
  deno task dev

  # Run the tests
  deno test
",
  );

  assert!(cwd.join("deno.json").exists());

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("run main.ts")
    .run();

  output.assert_exit_code(0);
  output.assert_matches_text("Log from main.ts that already exists\n");
}

#[test]
fn init_subcommand_empty() {
  let context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  let output = context
    .new_command()
    .args("init --empty")
    .split_output()
    .run();

  output.assert_exit_code(0);

  let stderr = output.stderr();
  assert_contains!(stderr, "Project initialized");
  assert!(!stderr.contains("cd"));
  assert_contains!(stderr, "deno run main.ts");
  assert_contains!(stderr, "deno task dev");
  assert!(!stderr.contains("deno test"));

  let deno_json_path = cwd.join("deno.json");
  assert!(deno_json_path.exists());

  let deno_json_content = deno_json_path.read_to_string();
  assert!(!deno_json_content.contains("@std/assert"));

  assert!(cwd.join("main.ts").exists());
  assert!(!cwd.join("main_test.ts").exists());

  let main_content = cwd.join("main.ts").read_to_string();
  assert_eq!(main_content, "console.log('Hello world!');\n");

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("run main.ts")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_eq!(output.stdout().as_bytes(), b"Hello world!\n");
  output.skip_output_check();
}

#[tokio::test]
#[cfg(not(all(target_os = "macos", target_arch = "x86_64")))]
async fn init_subcommand_serve() {
  let context = TestContextBuilder::for_jsr().use_temp_cwd().build();
  let cwd = context.temp_dir().path();

  let output = context
    .new_command()
    .args("init --serve")
    .split_output()
    .run();

  output.assert_exit_code(0);

  let stderr = output.stderr();
  assert_contains!(stderr, "Project initialized");
  assert_contains!(stderr, "deno serve -R main.ts");
  assert_contains!(stderr, "deno task dev");
  assert_contains!(stderr, "deno test -R");

  assert!(cwd.join("deno.json").exists());

  let mut child = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("serve -R --port 9500 main.ts")
    .spawn_with_piped_output();

  tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
  let resp = match reqwest::get("http://127.0.0.1:9500").await {
    Ok(resp) => resp,
    Err(_) => {
      // retry once
      tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
      reqwest::get("http://127.0.0.1:9500").await.unwrap()
    }
  };

  let body = resp.text().await.unwrap();
  assert_eq!(body, "Home page");

  let _ = child.kill();

  let output = context
    .new_command()
    .env("NO_COLOR", "1")
    .args("test -R")
    .split_output()
    .run();

  output.assert_exit_code(0);
  assert_contains!(output.stdout(), "4 passed");
  output.skip_output_check();
}

#[test(flaky)]
fn init_npm() {
  let context = TestContextBuilder::for_npm().use_temp_cwd().build();
  let cwd = context.temp_dir().path();
  context
    .new_command()
    .args("init --npm @denotest")
    .with_pty(|mut pty| {
      pty.expect("Do you want to continue?");
      pty.write_raw("y\n");
      pty.expect("Initialized!");
      assert_eq!(cwd.join("3").read_to_string(), "test");
    });
}
