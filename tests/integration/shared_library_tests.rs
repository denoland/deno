// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
// https://github.com/denoland/deno/issues/18266
fn linux_shared_libraries() {
  use test_util as util;

  const EXPECTED: [&str; 7] = [
    "linux-vdso.so.1",
    "libdl.so.2",
    "libgcc_s.so.1",
    "libpthread.so.0",
    "libm.so.6",
    "libc.so.6",
    "/lib64/ld-linux-x86-64.so.2",
  ];

  let ldd = std::process::Command::new("ldd")
    .arg("-L")
    .arg(util::deno_exe_path())
    .output()
    .expect("Failed to execute ldd");

  let output = std::str::from_utf8(&ldd.stdout).unwrap();
  // Ensure that the output contains only the expected shared libraries.
  for line in output.lines().skip(1) {
    let path = line.split_whitespace().next().unwrap();
    assert!(
      EXPECTED.contains(&path),
      "Unexpected shared library: {}",
      path
    );
  }
}

#[cfg(target_os = "macos")]
#[test]
// https://github.com/denoland/deno/issues/18243
// This test is to prevent inadvertently linking to more shared system libraries that usually
// increases dyld startup time.
fn macos_shared_libraries() {
  use test_util as util;

  // target/release/deno:
  // 	/usr/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)
  // 	/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1319.0.0)
  // 	/usr/lib/libobjc.A.dylib (compatibility version 1.0.0, current version 228.0.0)

  // path and whether its weak or not
  #[cfg(target_arch = "x86_64")]
  const EXPECTED: [(&str, bool); 9] = [
    ("/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation", false),
    ("/System/Library/Frameworks/CoreServices.framework/Versions/A/CoreServices", false),
    ("/System/Library/Frameworks/QuartzCore.framework/Versions/A/QuartzCore", true),
    ("/System/Library/Frameworks/Metal.framework/Versions/A/Metal", true),
    ("/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics", true),
    ("/System/Library/Frameworks/MetalPerformanceShaders.framework/Versions/A/MetalPerformanceShaders", true),
    ("/usr/lib/libiconv.2.dylib", false),
    ("/usr/lib/libSystem.B.dylib", false),
    ("/usr/lib/libobjc.A.dylib", false),
  ];
  #[cfg(target_arch = "aarch64")]
  const EXPECTED: [(&str, bool); 3] = [
    ("/usr/lib/libiconv.2.dylib", false),
    ("/usr/lib/libSystem.B.dylib", false),
    ("/usr/lib/libobjc.A.dylib", false),
  ];

  let otool = std::process::Command::new("otool")
    .arg("-L")
    .arg(util::deno_exe_path())
    .output()
    .expect("Failed to execute otool");

  let output = std::str::from_utf8(&otool.stdout).unwrap();
  // Ensure that the output contains only the expected shared libraries.
  for line in output.lines().skip(1) {
    let (path, attributes) = line.trim().split_once(' ').unwrap();
    assert!(
      EXPECTED.contains(&(path, attributes.ends_with("weak)"))),
      "Unexpected shared library: {}",
      path
    );
  }
}
