// Compile the laufey C++/ObjC++ backend + UIKit shell and link iOS frameworks.
//
// The native laufey backend sources live in a checkout of the `wef` repo. Its
// location is resolved from `LAUFEY_DEV_DIR` (the same env var the desktop
// backend resolver uses); CI sets this to the checked-out backend.
fn main() {
  if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
    return;
  }
  println!("cargo:rerun-if-env-changed=LAUFEY_DEV_DIR");

  let wef = match std::env::var_os("LAUFEY_DEV_DIR") {
    Some(dir) => std::path::PathBuf::from(dir),
    None => panic!(
      "LAUFEY_DEV_DIR must point at a laufey (wef) checkout to build the iOS \
       app; set it to the repo root that contains capi/, webview/, and \
       backend-common/."
    ),
  };
  let webview = wef.join("webview/src");
  let common = wef.join("backend-common");
  let incs = [
    wef.join("capi/include"),
    webview.clone(),
    common.join("include"),
  ];

  // Rebuild when any of the native sources change.
  for src in [
    webview.join("runtime_loader.cc"),
    common.join("src/laufey_value.cc"),
    webview.join("webview_ios.mm"),
    webview.join("main_ios.mm"),
  ] {
    println!("cargo:rerun-if-changed={}", src.display());
  }

  let mut cpp = cc::Build::new();
  cpp.cpp(true).std("c++17");
  for i in &incs {
    cpp.include(i);
  }
  // runtime_loader.cc references the portable backend-common helpers that
  // CMake's LAUFEY_COMMON_SRCS always compiles (platform-agnostic; the *_mac /
  // *_linux / *_win impls are desktop-only and excluded on iOS). Dead-strip
  // drops whatever the iOS path doesn't reference.
  cpp
    .file(webview.join("runtime_loader.cc"))
    .file(common.join("src/laufey_value.cc"))
    .file(common.join("src/test_hooks.cc"))
    .file(common.join("src/parse_options.cc"))
    .file(common.join("src/keymap_vk.cc"))
    .file(common.join("src/title_badge.cc"));
  cpp.compile("laufey_cpp");

  let mut objc = cc::Build::new();
  objc.cpp(true).std("c++17").flag("-fobjc-arc");
  for i in &incs {
    objc.include(i);
  }
  objc
    .file(webview.join("webview_ios.mm"))
    .file(webview.join("main_ios.mm"));
  objc.compile("laufey_objc");

  let out = std::env::var("OUT_DIR").unwrap();
  println!("cargo:rustc-link-arg=-Wl,-force_load,{out}/liblaufey_objc.a");
  println!("cargo:rustc-link-arg=-Wl,-force_load,{out}/liblaufey_cpp.a");
  for s in [
    "_laufey_runtime_init",
    "_laufey_runtime_start",
    "_laufey_runtime_shutdown",
  ] {
    println!("cargo:rustc-link-arg=-Wl,-u,{s}");
  }
  for fw in [
    "UIKit",
    "WebKit",
    "Foundation",
    "CoreGraphics",
    "QuartzCore",
  ] {
    println!("cargo:rustc-link-arg=-framework");
    println!("cargo:rustc-link-arg={fw}");
  }
  println!("cargo:rustc-link-arg=-lobjc");
  println!("cargo:rustc-link-arg=-lc++");
}
