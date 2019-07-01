use cargo_gn;
use std::env;
use std::path::PathBuf;
use std::process::Command;
use which::which;

fn binary_downloads() {
  let cwd = env::current_dir().unwrap();
  let root = cwd.join("..");
  let status = Command::new("python")
    .arg("tools/binary_downloads.py")
    .current_dir(root)
    .status()
    .expect("tools/binary_downloads.py failed");
  assert!(status.success());
}

// This is essentially a re-write of the original tools/setup.py
// but in rust.
fn setup() -> PathBuf {
  let mut debug_args = "is_debug=true\n".to_string();
  let mut release_args = "is_official_build=true\nsymbol_level=0\n".to_string();

  if env::var_os("DENO_NO_BINARY_DOWNLOAD").is_none() {
    binary_downloads();
  }

  // TODO(ry) Support prebuilt/mac/sccache
  match which("sccache") {
    Ok(sccache_path) => {
      debug_args += &format!("cc_wrapper={:?}\n", sccache_path);
      debug_args += &format!("rustc_wrapper={:?}\n", sccache_path);

      release_args += &format!("cc_wrapper={:?}\n", sccache_path);
      release_args += &format!("rustc_wrapper={:?}\n", sccache_path);
    }
    Err(_) => {}
  }

  match env::var("DENO_BUILD_ARGS") {
    Ok(val) => {
      for arg in val.split_whitespace() {
        debug_args += arg;
        debug_args += "\n";
        release_args += arg;
        release_args += "\n";
      }
    }
    Err(_) => {}
  };

  cargo_gn::maybe_gen("..", &debug_args, &release_args)
}
