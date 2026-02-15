// Copyright 2018-2026 the Deno authors. MIT license.

// port constants extracted from servers/mod.rs so they are available
// without compiling the heavy server dependencies
pub const PORT: u16 = 4545;
pub const JSR_REGISTRY_SERVER_PORT: u16 = 4250;
pub const PROVENANCE_MOCK_SERVER_PORT: u16 = 4251;
pub const NODEJS_ORG_MIRROR_SERVER_PORT: u16 = 4252;
pub const PUBLIC_NPM_REGISTRY_PORT: u16 = 4260;
pub const PRIVATE_NPM_REGISTRY_1_PORT: u16 = 4261;
pub const PRIVATE_NPM_REGISTRY_2_PORT: u16 = 4262;
pub const PRIVATE_NPM_REGISTRY_3_PORT: u16 = 4263;
pub const SOCKET_DEV_API_PORT: u16 = 4268;
pub const PUBLIC_NPM_JSR_REGISTRY_PORT: u16 = 4269;

#[allow(unused)]
pub mod tsgo {
  include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../cli/tsc/go/tsgo_version.rs"
  ));
}

pub const TSGO_PLATFORM: &str = tsgo_platform();
const fn tsgo_platform() -> &'static str {
  match (
    std::env::consts::OS.as_bytes(),
    std::env::consts::ARCH.as_bytes(),
  ) {
    (b"windows", b"x86_64") => "windows-x64",
    (b"windows", b"aarch64") => "windows-arm64",
    (b"macos", b"x86_64") => "macos-x64",
    (b"macos", b"aarch64") => "macos-arm64",
    (b"linux", b"x86_64") => "linux-x64",
    (b"linux", b"aarch64") => "linux-arm64",
    _ => {
      panic!("unsupported platform");
    }
  }
}

pub fn tsgo_prebuilt_path() -> crate::fs::PathRef {
  if let Ok(path) = std::env::var("DENO_TSGO_PATH") {
    return crate::fs::PathRef::new(path);
  }
  let folder = match std::env::consts::OS {
    "linux" => "linux64",
    "windows" => "win",
    "macos" | "apple" => "mac",
    _ => panic!("unsupported platform"),
  };
  crate::prebuilt_path().join(folder).join(format!(
    "tsgo-{}-{}",
    tsgo::VERSION,
    TSGO_PLATFORM
  ))
}
