// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

#[cfg(target_os = "linux")]
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
