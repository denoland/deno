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
fn setup() -> (PathBuf, Option<cargo_gn::NinjaEnv>) {
  let is_win = cfg!(target_os = "windows");
  let is_debug = cargo_gn::is_debug();

  let mut gn_args: cargo_gn::GnArgs = Vec::new();
  if is_debug && !is_win {
    gn_args.push("is_debug=true".to_string());
  } else if is_debug && is_win {
    // Rust always links with the release flavor of the CRT. Chromium //build
    // uses the debug version of the CRT when 'is_debug` is set, and there's no
    // override. Therefore, we cannot build a debug V8 and use Rust to link it.
    gn_args.push("is_debug=false".to_string());
  } else {
    gn_args.push("is_official_build=true".to_string());
    gn_args.push("symbol_level=0".to_string());
  }

  if env::var_os("DENO_NO_BINARY_DOWNLOAD").is_none() {
    binary_downloads();
  }

  // TODO(ry) Support prebuilt/mac/sccache
  match which("sccache") {
    Ok(sccache_path) => {
      gn_args.push(format!("cc_wrapper={:?}", sccache_path));
      gn_args.push(format!("rustc_wrapper={:?}", sccache_path));
    }
    Err(_) => {}
  }

  match env::var("DENO_BUILD_ARGS") {
    Ok(val) => {
      for arg in val.split_whitespace() {
        gn_args.push(arg.to_string());
      }
    }
    Err(_) => {}
  };

  let cwd = env::current_dir().unwrap();
  let workspace_dir = cwd.parent().unwrap();

  let ninja_env: Option<cargo_gn::NinjaEnv> = if !is_win {
    None
  } else {
    // Windows needs special configuration. This is similar to the function of
    // python_env() in //tools/util.py.
    let mut env = Vec::new();
    let python_path: Vec<String> = vec![
      "third_party/python_packages",
      "third_party/python_packages/win32",
      "third_party/python_packages/win32/lib",
      "third_party/python_packages/Pythonwin",
    ]
    .into_iter()
    .map(|p| {
      workspace_dir
        .join(p)
        .into_os_string()
        .into_string()
        .unwrap()
    })
    .collect();
    let orig_path =
      String::from(";") + &env::var_os("PATH").unwrap().into_string().unwrap();
    let path = workspace_dir
      .join("third_party/python_packages/pywin32_system32")
      .into_os_string()
      .into_string()
      .unwrap();
    env.push(("PYTHONPATH".to_string(), python_path.join(";")));
    env.push(("PATH".to_string(), path + &orig_path));
    env.push(("DEPOT_TOOLS_WIN_TOOLCHAIN".to_string(), "0".to_string()));
    Some(env)
  };

  (cargo_gn::maybe_gen("..", gn_args), ninja_env)
}

/// Detect if we're being invoked by the rust language server (RLS).
/// When RLS is running "cargo check" to analyze the source code, we're not
/// trying to build a working executable, rather we're just compiling all
/// rust code.
/// Unfortunately we can't detect whether we're being run by `cargo check`.
fn is_rls_build() -> bool {
  env::var_os("CARGO")
    .map(PathBuf::from)
    .as_ref()
    .and_then(|p| p.file_stem())
    .and_then(|f| f.to_str())
    .map(|s| s.starts_with("rls"))
    .unwrap_or(false)
}
