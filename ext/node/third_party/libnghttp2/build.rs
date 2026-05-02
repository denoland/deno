use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const SOURCES: &[&str] = &[
  "nghttp2/lib/sfparse.c",
  "nghttp2/lib/nghttp2_alpn.c",
  "nghttp2/lib/nghttp2_buf.c",
  "nghttp2/lib/nghttp2_callbacks.c",
  "nghttp2/lib/nghttp2_debug.c",
  "nghttp2/lib/nghttp2_extpri.c",
  "nghttp2/lib/nghttp2_frame.c",
  "nghttp2/lib/nghttp2_hd.c",
  "nghttp2/lib/nghttp2_hd_huffman.c",
  "nghttp2/lib/nghttp2_hd_huffman_data.c",
  "nghttp2/lib/nghttp2_helper.c",
  "nghttp2/lib/nghttp2_http.c",
  "nghttp2/lib/nghttp2_map.c",
  "nghttp2/lib/nghttp2_mem.c",
  "nghttp2/lib/nghttp2_option.c",
  "nghttp2/lib/nghttp2_outbound_item.c",
  "nghttp2/lib/nghttp2_pq.c",
  "nghttp2/lib/nghttp2_priority_spec.c",
  "nghttp2/lib/nghttp2_queue.c",
  "nghttp2/lib/nghttp2_rcbuf.c",
  "nghttp2/lib/nghttp2_session.c",
  "nghttp2/lib/nghttp2_stream.c",
  "nghttp2/lib/nghttp2_submit.c",
  "nghttp2/lib/nghttp2_version.c",
  "nghttp2/lib/nghttp2_ratelim.c",
  "nghttp2/lib/nghttp2_time.c",
];

const HEADERS: &[&str] = &[
  "nghttp2/lib/sfparse.h",
  "nghttp2/lib/nghttp2_alpn.h",
  "nghttp2/lib/nghttp2_buf.h",
  "nghttp2/lib/nghttp2_callbacks.h",
  "nghttp2/lib/nghttp2_debug.h",
  "nghttp2/lib/nghttp2_extpri.h",
  "nghttp2/lib/nghttp2_frame.h",
  "nghttp2/lib/nghttp2_hd.h",
  "nghttp2/lib/nghttp2_hd_huffman.h",
  "nghttp2/lib/nghttp2_helper.h",
  "nghttp2/lib/nghttp2_http.h",
  "nghttp2/lib/nghttp2_int.h",
  "nghttp2/lib/nghttp2_map.h",
  "nghttp2/lib/nghttp2_mem.h",
  "nghttp2/lib/nghttp2_net.h",
  "nghttp2/lib/nghttp2_option.h",
  "nghttp2/lib/nghttp2_outbound_item.h",
  "nghttp2/lib/nghttp2_pq.h",
  "nghttp2/lib/nghttp2_priority_spec.h",
  "nghttp2/lib/nghttp2_queue.h",
  "nghttp2/lib/nghttp2_ratelim.h",
  "nghttp2/lib/nghttp2_rcbuf.h",
  "nghttp2/lib/nghttp2_session.h",
  "nghttp2/lib/nghttp2_stream.h",
  "nghttp2/lib/nghttp2_submit.h",
  "nghttp2/lib/nghttp2_time.h",
];

fn main() {
  let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));
  let target = env::var("TARGET").expect("TARGET not set");

  let nghttp2_version = parse_nghttp2_version();

  let install_dir = out_dir.join("i");
  let include_dir = install_dir.join("include");
  let lib_dir = install_dir.join("lib");

  setup_directories(&include_dir, &lib_dir);
  generate_version_header(&include_dir, &nghttp2_version);
  copy_main_header(&include_dir);

  build_nghttp2(&target, &include_dir, &lib_dir);

  generate_pkgconfig(&install_dir, &include_dir, &lib_dir, &nghttp2_version);
  generate_bindings(&out_dir, &include_dir);

  println!("cargo:root={}", install_dir.display());

  // Emit rerun-if-changed directives to avoid unnecessary rebuilds
  emit_rerun_if_changed();
}

fn emit_rerun_if_changed() {
  // Build script itself
  println!("cargo:rerun-if-changed=build.rs");

  // Template files
  println!(
    "cargo:rerun-if-changed=nghttp2/lib/includes/nghttp2/nghttp2ver.h.in"
  );
  println!("cargo:rerun-if-changed=nghttp2/lib/libnghttp2.pc.in");

  // Header files (public)
  println!("cargo:rerun-if-changed=nghttp2/lib/includes/nghttp2/nghttp2.h");

  // Source files
  for source in SOURCES {
    println!("cargo:rerun-if-changed={}", source);
  }

  // Header dependencies (internal)
  for header in HEADERS {
    println!("cargo:rerun-if-changed={}", header);
  }
}

struct NgHttp2Version {
  string: String,
  major: u32,
  minor: u32,
  patch: u32,
}

fn parse_nghttp2_version() -> NgHttp2Version {
  let version_str =
    env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION not set");

  let parts: Vec<u32> = version_str
    .split('.')
    .take(3)
    .map(|s| {
      // Strip any pre-release suffixes like "-rc.0"
      let numeric_part = s.split('-').next().unwrap_or(s);
      numeric_part.parse().expect("Invalid version number")
    })
    .collect();

  assert_eq!(parts.len(), 3, "Version must have 3 components");

  NgHttp2Version {
    string: version_str.to_string(),
    major: parts[0],
    minor: parts[1],
    patch: parts[2],
  }
}

fn setup_directories(include_dir: &Path, lib_dir: &Path) {
  fs::create_dir_all(include_dir.join("nghttp2"))
    .expect("Failed to create include directory");
  fs::create_dir_all(lib_dir.join("pkgconfig"))
    .expect("Failed to create lib/pkgconfig directory");
}

// Generate nghttp2ver.h from template
fn generate_version_header(include_dir: &Path, version: &NgHttp2Version) {
  let template =
    fs::read_to_string("nghttp2/lib/includes/nghttp2/nghttp2ver.h.in")
      .expect("Failed to read nghttp2ver.h.in template");

  let version_num = format!(
    "0x{:02x}{:02x}{:02x}",
    version.major, version.minor, version.patch
  );

  let header = template
    .replace("@PACKAGE_VERSION@", &version.string)
    .replace("@PACKAGE_VERSION_NUM@", &version_num);

  fs::write(include_dir.join("nghttp2/nghttp2ver.h"), header)
    .expect("Failed to write nghttp2ver.h");
}

fn copy_main_header(include_dir: &Path) {
  fs::copy(
    "nghttp2/lib/includes/nghttp2/nghttp2.h",
    include_dir.join("nghttp2/nghttp2.h"),
  )
  .expect("Failed to copy nghttp2.h");
}

fn build_nghttp2(target: &str, include_dir: &Path, lib_dir: &Path) {
  let mut build = cc::Build::new();

  build.include("nghttp2/lib/includes").include(include_dir);
  add_source_files(&mut build);
  build
    .warnings(false)
    .define("NGHTTP2_STATICLIB", None)
    .define("HAVE_NETINET_IN", None)
    .define("HAVE_TIME_H", None)
    .out_dir(lib_dir);
  configure_platform(&mut build, target);

  build.compile("nghttp2");
}

fn add_source_files(build: &mut cc::Build) {
  for source in SOURCES {
    build.file(source);
  }
}

fn configure_platform(build: &mut cc::Build, target: &str) {
  if target.contains("windows") {
    // MSVC doesn't have ssize_t, define it based on pointer width
    if target.contains("msvc") {
      let pointer_width = env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .expect("CARGO_CFG_TARGET_POINTER_WIDTH not set");

      match pointer_width.as_str() {
        "64" => build.define("ssize_t", "int64_t"),
        "32" => build.define("ssize_t", "int32_t"),
        width => panic!("Unsupported pointer width: {}", width),
      };
    }
  } else {
    build.define("HAVE_ARPA_INET_H", None);
  }
}

fn generate_pkgconfig(
  install_dir: &Path,
  include_dir: &Path,
  lib_dir: &Path,
  version: &NgHttp2Version,
) {
  let template = fs::read_to_string("nghttp2/lib/libnghttp2.pc.in")
    .expect("Failed to read libnghttp2.pc.in template");

  let pkgconfig = template
    .replace("@prefix@", install_dir.to_str().unwrap())
    .replace("@exec_prefix@", "")
    .replace("@libdir@", lib_dir.to_str().unwrap())
    .replace("@includedir@", include_dir.to_str().unwrap())
    .replace("@VERSION@", &version.string);

  fs::write(lib_dir.join("pkgconfig/libnghttp2.pc"), pkgconfig)
    .expect("Failed to write libnghttp2.pc");
}

fn generate_bindings(out_dir: &Path, include_dir: &Path) {
  let header_path = include_dir.join("nghttp2/nghttp2.h");
  let target = env::var("TARGET").expect("TARGET not set");

  let mut builder = bindgen::Builder::default()
    .header(header_path.to_str().unwrap())
    .clang_arg(format!("-I{}", include_dir.display()))
    .clang_arg("-Inghttp2/lib/includes");

  // On Windows MSVC, define ssize_t for clang/bindgen
  if target.contains("windows") && target.contains("msvc") {
    let pointer_width = env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
      .expect("CARGO_CFG_TARGET_POINTER_WIDTH not set");

    let ssize_t_def = match pointer_width.as_str() {
      "64" => "ssize_t=long long",
      "32" => "ssize_t=long",
      width => panic!("Unsupported pointer width: {}", width),
    };

    builder = builder.clang_arg(format!("-D{}", ssize_t_def));
  }

  // Note: We don't use CargoCallbacks here because it would emit
  // rerun-if-changed for generated headers in OUT_DIR (whose path changes
  // between builds) and system headers. We manually emit rerun-if-changed
  // for the source files in emit_rerun_if_changed() instead.
  let bindings = builder
    // Only include nghttp2 symbols
    .allowlist_function("nghttp2_.*")
    .allowlist_type("nghttp2_.*")
    .allowlist_var("NGHTTP2_.*")
    // Generate enum constants with consistent types across platforms.
    // Without this, C enums become i32 on MSVC (C standard: enum is int)
    // but u32 on GCC/Clang (where the underlying type matches the values).
    .default_enum_style(bindgen::EnumVariation::Consts)
    .translate_enum_integer_types(true)
    // Opaque types that should not derive Copy
    .opaque_type("nghttp2_session")
    .opaque_type("nghttp2_rcbuf")
    .opaque_type("nghttp2_session_callbacks")
    .opaque_type("nghttp2_option")
    .opaque_type("nghttp2_hd_deflater")
    .opaque_type("nghttp2_hd_inflater")
    .opaque_type("nghttp2_stream")
    .layout_tests(false)
    //.generate_comments(false)
    .prepend_enum_name(false)
    .blocklist_function(".*vprintf.*")
    .blocklist_type(".*va_list.*")
    .blocklist_type("nghttp2_debug_vprintf_callback")
    .generate()
    .expect("Failed to generate bindings");

  bindings
    .write_to_file(out_dir.join("bindings.rs"))
    .expect("Failed to write bindings");
}
