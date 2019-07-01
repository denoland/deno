use cargo_gn;
use std::path::PathBuf;

// This is essentially a re-write of the original tools/setup.py
// but in rust.
fn setup() -> PathBuf {
  let mut debug_args = "is_debug=true\n".to_string();
  let mut release_args = "is_official_build=true\nsymbol_level=0\n".to_string();

  match cargo_gn::which("sccache") {
    Ok(sccache_path) => {
      debug_args += &format!("cc_wrapper={:?}\n", sccache_path);
      debug_args += &format!("rustc_wrapper={:?}\n", sccache_path);

      release_args += &format!("cc_wrapper={:?}\n", sccache_path);
      release_args += &format!("rustc_wrapper={:?}\n", sccache_path);
    }
    Err(_) => {
      unimplemented!();
    }
  }

  cargo_gn::maybe_gen("..", &debug_args, &release_args)
}
