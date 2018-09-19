// See https://doc.rust-lang.org/reference/attributes.html
pub struct Platform {
  pub os: String,
  pub family: String,
}

// OS

#[cfg(target_os = "windows")]
fn get_os() -> String {
  "windows".to_string()
}

#[cfg(target_os = "macos")]
fn get_os() -> String {
  "macos".to_string()
}

#[cfg(target_os = "ios")]
fn get_os() -> String {
  "ios".to_string()
}

#[cfg(target_os = "linux")]
fn get_os() -> String {
  "linux".to_string()
}

#[cfg(target_os = "android")]
fn get_os() -> String {
  "android".to_string()
}

#[cfg(target_os = "freebsd")]
fn get_os() -> String {
  "freebsd".to_string()
}

#[cfg(target_os = "dragonfly")]
fn get_os() -> String {
  "dragonfly".to_string()
}

#[cfg(target_os = "bitrig")]
fn get_os() -> String {
  "bitrig".to_string()
}

#[cfg(target_os = "openbsd")]
fn get_os() -> String {
  "openbsd".to_string()
}

#[cfg(target_os = "netbsd")]
fn get_os() -> String {
  "netbsd".to_string()
}

#[cfg(
  not(
    any(
      target_os = "windows",
      target_os = "macos",
      target_os = "ios",
      target_os = "linux",
      target_os = "android",
      target_os = "freebsd",
      target_os = "dragonfly",
      target_os = "bitrig",
      target_os = "openbsd",
      target_os = "netbsd"
    )
  )
)]
fn get_os() -> String {
  // In case of new OS, e.g. fuschia
  "other".to_string()
}

// FAMILY

#[cfg(target_family = "windows")]
fn get_family() -> String {
  "windows".to_string()
}

#[cfg(target_family = "unix")]
fn get_family() -> String {
  "unix".to_string()
}

#[cfg(not(any(target_family = "windows", target_family = "unix")))]
fn get_family() -> String {
  // In case
  "other".to_string()
}

pub fn get_platform() -> Platform {
  Platform {
    os: get_os(),
    family: get_family(),
  }
}
