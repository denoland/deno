// Copyright 2018-2025 the Deno authors. MIT license.

fn main() {
  let symbols_file_name = match std::env::consts::OS {
    "android" | "freebsd" | "openbsd" => {
      "generated_symbol_exports_list_linux.def".to_string()
    }
    os => format!("generated_symbol_exports_list_{}.def", os),
  };
  let symbols_path = std::path::Path::new(".")
    .join(symbols_file_name)
    .canonicalize()
    .expect(
        "Missing symbols list! Generate using tools/napi/generate_symbols_lists.js",
    );

  println!("cargo:rustc-rerun-if-changed={}", symbols_path.display());

  let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
    .join("napi_symbol_path.txt");
  std::fs::write(path, symbols_path.as_os_str().as_encoded_bytes()).unwrap();
}
