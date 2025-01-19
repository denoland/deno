// Copyright 2018-2025 the Deno authors. MIT license.

fn main() {
  // todo(dsherret): remove this after Deno 2.2.0 is published and then
  // align the version of this crate with Deno then. We need to wait because
  // there was previously a deno_lib 2.2.0 published (https://crates.io/crates/deno_lib/versions)
  let version_path = std::path::Path::new(".").join("version.txt");
  println!("cargo:rerun-if-changed={}", version_path.display());
  #[allow(clippy::disallowed_methods)]
  let text = std::fs::read_to_string(version_path).unwrap();
  println!("cargo:rustc-env=DENO_VERSION={}", text);

  let commit_hash = git_commit_hash();
  println!("cargo:rustc-env=GIT_COMMIT_HASH={}", commit_hash);
  println!("cargo:rerun-if-env-changed=GIT_COMMIT_HASH");
  println!(
    "cargo:rustc-env=GIT_COMMIT_HASH_SHORT={}",
    &commit_hash[..7]
  );
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
