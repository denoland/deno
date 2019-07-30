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
  let is_debug = cargo_gn::is_debug();
  let mut gn_args: cargo_gn::GnArgs = Vec::new();
  if is_debug {
    gn_args.push(("is_debug".to_string(), "true".to_string()));
  } else {
    gn_args.push(("is_official_build".to_string(), "true".to_string()));
    gn_args.push(("symbol_level".to_string(), "0".to_string()));
  }

  if env::var_os("DENO_NO_BINARY_DOWNLOAD").is_none() {
    binary_downloads();
  }

  // TODO(ry) Support prebuilt/mac/sccache
  match which("sccache") {
    Ok(sccache_path) => {
      gn_args.push(("cc_wapper".to_string(), format!("{:?}", sccache_path)));
      gn_args
        .push(("rustc_wrapper".to_string(), format!("{:?}", sccache_path)));
    }
    Err(_) => {}
  }

  match env::var("DENO_BUILD_ARGS") {
    Ok(val) => {
      for arg in val.split_whitespace() {
        let split_pos = arg.find("=").unwrap();
        let (arg_key, arg_value) = arg.split_at(split_pos);
        gn_args.push((arg_key.to_string(), arg_value.to_string()));
      }
    }
    Err(_) => {}
  };

  cargo_gn::maybe_gen("..", gn_args)
}
