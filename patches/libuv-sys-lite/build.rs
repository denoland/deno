use std::path::PathBuf;

fn generate_bindings() {
  let mut builder = bindgen::builder().header("libuv-1.48.0-include/uv.h");
  builder = builder
    .allowlist_type("uv_.+")
    .allowlist_type("UV_.+")
    .allowlist_var("uv_.+")
    .allowlist_var("UV_.+")
    .allowlist_type("FILE")
    .default_enum_style(bindgen::EnumVariation::NewType {
      is_bitfield: false,
      is_global: false,
    });
  builder
    .clang_arg("-Ilibuv-1.48.0-include")
    .generate()
    .unwrap()
    .write_to_file(PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bindings.rs"))
    .unwrap();
}

fn env_is_set(var: &str) -> bool {
  std::env::var(var).is_ok()
}

fn cfg_is_set(var: &str) -> bool {
  env_is_set(&format!(
    "CARGO_FEATURE_{}",
    var.to_uppercase().replace('-', "_")
  ))
}

fn main() {
  if env_is_set("LIBUV_SYS_LITE_BINDGEN") || cfg_is_set("bindgen") {
    generate_bindings();
    return;
  }

  let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
  let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
  match (&*target_os, &*target_arch) {
    ("windows" | "linux" | "macos", "x86_64" | "aarch64") => {
      let path = PathBuf::from("src").join(format!("bindings_{}_{}.rs", target_os, target_arch));
      let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("bindings.rs");
      std::fs::copy(&path, &dest).unwrap();
    }
    _ => generate_bindings(),
  }
}
