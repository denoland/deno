use deno_runtime::deno_console;
use deno_runtime::deno_crypto;
use deno_runtime::deno_fetch;
use deno_runtime::deno_file;
use deno_runtime::deno_url;
use deno_runtime::deno_web;
use deno_runtime::deno_webgpu;
use deno_runtime::deno_websocket;
use std::env;
use std::path::Path;

fn main() {
  // Skip building from docs.rs.
  if env::var_os("DOCS_RS").is_some() {
    return;
  }

  println!("cargo:rustc-env=TS_VERSION={}", ts_version());
  println!("cargo:rustc-env=GIT_COMMIT_HASH={}", git_commit_hash());

  println!("cargo:rustc-env=TARGET={}", env::var("TARGET").unwrap());
  println!("cargo:rustc-env=PROFILE={}", env::var("PROFILE").unwrap());
  if let Ok(c) = env::var("DENO_CANARY") {
    println!("cargo:rustc-env=DENO_CANARY={}", c);
  }
  println!(
    "cargo:rustc-env=DENO_CONSOLE_LIB_PATH={}",
    deno_console::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_URL_LIB_PATH={}",
    deno_url::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEB_LIB_PATH={}",
    deno_web::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_FILE_LIB_PATH={}",
    deno_file::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_FETCH_LIB_PATH={}",
    deno_fetch::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBGPU_LIB_PATH={}",
    deno_webgpu::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_WEBSOCKET_LIB_PATH={}",
    deno_websocket::get_declaration().display()
  );
  println!(
    "cargo:rustc-env=DENO_CRYPTO_LIB_PATH={}",
    deno_crypto::get_declaration().display()
  );

  #[cfg(target_os = "windows")]
  {
    let mut res = winres::WindowsResource::new();
    res.set_icon("deno.ico");
    res.set_language(winapi::um::winnt::MAKELANGID(
      winapi::um::winnt::LANG_ENGLISH,
      winapi::um::winnt::SUBLANG_ENGLISH_US,
    ));
    res.compile().unwrap();
  }
}

fn ts_version() -> String {
  let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
  let tsc_js = manifest_dir.join("tsc").join("00_typescript.js");
  std::fs::read_to_string(tsc_js)
    .unwrap()
    .lines()
    .find(|l| l.contains("ts.version = "))
    .expect(
      "Failed to find the pattern `ts.version = ` in typescript source code",
    )
    .chars()
    .skip_while(|c| !char::is_numeric(*c))
    .take_while(|c| *c != '"')
    .collect::<String>()
}

fn git_commit_hash() -> String {
  if let Ok(output) = std::process::Command::new("git")
    .arg("rev-list")
    .arg("-1")
    .arg("HEAD")
    .output()
  {
    if output.status.success() {
      std::str::from_utf8(&output.stdout[..40])
        .unwrap()
        .to_string()
    } else {
      // When not in git repository
      // (e.g. when the user install by `cargo install deno`)
      "UNKNOWN".to_string()
    }
  } else {
    // When there is no git command for some reason
    "UNKNOWN".to_string()
  }
}
