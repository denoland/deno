// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#[cfg(target_os = "macos")]
#[test]
// https://github.com/denoland/deno/issues/18243
// This test is to prevent inadvertently linking to more shared system libraries that usually
// increases dyld startup time.
fn macos_shared_libraries() {
  use test_util as util;

  // target/release/deno:
  // 	/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation (compatibility version 150.0.0, current version 1953.255.0)
  // 	/System/Library/Frameworks/CoreServices.framework/Versions/A/CoreServices (compatibility version 1.0.0, current version 1228.0.0)
  // 	/System/Library/Frameworks/Security.framework/Versions/A/Security (compatibility version 1.0.0, current version 60420.60.24)
  // 	/usr/lib/libiconv.2.dylib (compatibility version 7.0.0, current version 7.0.0)
  // 	/usr/lib/libSystem.B.dylib (compatibility version 1.0.0, current version 1319.0.0)
  const EXPECTED: [&str; 5] =
    ["/System/Library/Frameworks/CoreFoundation.framework/Versions/A/CoreFoundation",
     "/System/Library/Frameworks/CoreServices.framework/Versions/A/CoreServices",
     "/System/Library/Frameworks/Security.framework/Versions/A/Security",
     "/usr/lib/libiconv.2.dylib",
     "/usr/lib/libSystem.B.dylib"];

  let otool = std::process::Command::new("otool")
    .arg("-L")
    .arg(util::deno_exe_path())
    .output()
    .expect("Failed to execute otool");

  let output = std::str::from_utf8(&otool.stdout).unwrap();
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
