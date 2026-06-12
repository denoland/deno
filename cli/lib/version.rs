// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;

use deno_runtime::deno_telemetry::OtelRuntimeConfig;

use crate::shared::ReleaseChannel;

pub fn otel_runtime_config() -> OtelRuntimeConfig {
  OtelRuntimeConfig {
    runtime_name: Cow::Borrowed("deno"),
    runtime_version: Cow::Borrowed(crate::version::DENO_VERSION_INFO.deno),
  }
}

const GIT_COMMIT_HASH: &str = env!("GIT_COMMIT_HASH");
const TYPESCRIPT: &str = "6.0.3";
pub const DENO_VERSION: &str = env!("DENO_VERSION");
// TODO(bartlomieju): ideally we could remove this const.
const IS_CANARY: bool = option_env!("DENO_CANARY").is_some();
// TODO(bartlomieju): this is temporary, to allow Homebrew to cut RC releases as well
const IS_RC: bool = option_env!("DENO_RC").is_some();

pub static DENO_VERSION_INFO: std::sync::LazyLock<DenoVersionInfo> =
  std::sync::LazyLock::new(|| {
    let release_channel = {
      // On Linux, bypass libsui::find_section which uses dl_iterate_phdr and
      // hangs in release builds. Read the ELF PT_NOTE segment directly.
      #[cfg(all(unix, not(target_vendor = "apple")))]
      {
        read_denover_from_elf()
          .as_deref()
          .and_then(|buf| std::str::from_utf8(buf).ok())
          .and_then(|str_| ReleaseChannel::deserialize(str_).ok())
          .unwrap_or_else(|| {
            if IS_CANARY {
              ReleaseChannel::Canary
            } else if IS_RC {
              ReleaseChannel::Rc
            } else {
              release_channel_from_version_string(DENO_VERSION)
            }
          })
      }

      // On macOS x86_64 debug, libsui::find_section also hangs; use fallback.
      #[cfg(all(
        debug_assertions,
        target_os = "macos",
        target_arch = "x86_64"
      ))]
      {
        if IS_CANARY {
          ReleaseChannel::Canary
        } else if IS_RC {
          ReleaseChannel::Rc
        } else {
          release_channel_from_version_string(DENO_VERSION)
        }
      }

      // All other targets (macOS arm64/x86_64 release, Windows): libsui works.
      #[cfg(not(any(
        all(unix, not(target_vendor = "apple")),
        all(debug_assertions, target_os = "macos", target_arch = "x86_64")
      )))]
      {
        libsui::find_section("denover")
          .ok()
          .flatten()
          .and_then(|buf| std::str::from_utf8(buf).ok())
          .and_then(|str_| ReleaseChannel::deserialize(str_).ok())
          .unwrap_or({
            if IS_CANARY {
              ReleaseChannel::Canary
            } else if IS_RC {
              ReleaseChannel::Rc
            } else {
              release_channel_from_version_string(DENO_VERSION)
            }
          })
      }
    };

    DenoVersionInfo {
      deno: if release_channel == ReleaseChannel::Canary {
        concat!(env!("DENO_VERSION"), "+", env!("GIT_COMMIT_HASH_SHORT"))
      } else {
        env!("DENO_VERSION")
      },

      release_channel,

      git_hash: GIT_COMMIT_HASH,

      // Keep in sync with `deno` field.
      user_agent: if release_channel == ReleaseChannel::Canary {
        concat!(
          "Deno/",
          env!("DENO_VERSION"),
          "+",
          env!("GIT_COMMIT_HASH_SHORT")
        )
      } else {
        concat!("Deno/", env!("DENO_VERSION"))
      },

      typescript: TYPESCRIPT,
    }
  });

pub struct DenoVersionInfo {
  /// Human-readable version of the current Deno binary.
  ///
  /// For stable release, a semver, eg. `v1.46.2`.
  /// For canary release, a semver + 7-char git hash, eg. `v1.46.3+asdfqwq`.
  pub deno: &'static str,

  pub release_channel: ReleaseChannel,

  /// A full git hash.
  pub git_hash: &'static str,

  /// A user-agent header that will be used in HTTP client.
  pub user_agent: &'static str,

  pub typescript: &'static str,
}

impl DenoVersionInfo {
  /// For stable release, a semver like, eg. `v1.46.2`.
  /// For canary release a full git hash, eg. `9bdab6fb6b93eb43b1930f40987fa4997287f9c8`.
  pub fn version_or_git_hash(&self) -> &'static str {
    if self.release_channel == ReleaseChannel::Canary {
      self.git_hash
    } else {
      DENO_VERSION
    }
  }
}

/// On Linux, reads the `denover` section from the ELF binary's PT_NOTE segment
/// directly (via `/proc/self/exe`), avoiding `dl_iterate_phdr` which hangs in
/// release builds when libsui-embedded sections are present.
#[cfg(all(unix, not(target_vendor = "apple")))]
fn read_denover_from_elf() -> Option<Vec<u8>> {
  use std::io::Read;
  use std::io::Seek;
  use std::io::SeekFrom;

  let mut file = std::fs::File::open("/proc/self/exe")
    .or_else(|_| std::env::current_exe().and_then(std::fs::File::open))
    .ok()?;

  let mut ehdr = [0u8; 64];
  file.read_exact(&mut ehdr).ok()?;
  if &ehdr[0..4] != b"\x7fELF" || ehdr[4] != 2 || ehdr[5] != 1 {
    return None;
  }

  let e_phoff = u64::from_le_bytes(ehdr[32..40].try_into().ok()?) as usize;
  let e_phentsize = u16::from_le_bytes(ehdr[54..56].try_into().ok()?) as usize;
  let e_phnum = u16::from_le_bytes(ehdr[56..58].try_into().ok()?) as usize;

  if e_phentsize < 56 || e_phnum == 0 {
    return None;
  }

  let phdrs_len = e_phentsize.checked_mul(e_phnum)?;
  file.seek(SeekFrom::Start(e_phoff as u64)).ok()?;
  let mut phdrs = vec![0u8; phdrs_len];
  file.read_exact(&mut phdrs).ok()?;

  const PT_NOTE: u32 = 4;
  const SUI_NOTE_TYPE: u32 = 0x5355_4901;

  for i in 0..e_phnum {
    let ph = &phdrs[i * e_phentsize..];
    let p_type = u32::from_le_bytes(ph[0..4].try_into().ok()?);
    if p_type != PT_NOTE {
      continue;
    }
    let p_offset = u64::from_le_bytes(ph[8..16].try_into().ok()?);
    let p_filesz = u64::from_le_bytes(ph[32..40].try_into().ok()?) as usize;
    if p_filesz == 0 {
      continue;
    }

    file.seek(SeekFrom::Start(p_offset)).ok()?;
    let mut note_data = vec![0u8; p_filesz];
    file.read_exact(&mut note_data).ok()?;

    let mut pos = 0usize;
    while pos + 12 <= note_data.len() {
      let namesz =
        u32::from_le_bytes(note_data[pos..pos + 4].try_into().ok()?) as usize;
      let descsz =
        u32::from_le_bytes(note_data[pos + 4..pos + 8].try_into().ok()?)
          as usize;
      let note_type =
        u32::from_le_bytes(note_data[pos + 8..pos + 12].try_into().ok()?);
      pos += 12;

      if pos + namesz > note_data.len() {
        break;
      }
      let raw_name = &note_data[pos..pos + namesz];
      let name_end = raw_name
        .iter()
        .rposition(|&b| b != 0)
        .map(|i| i + 1)
        .unwrap_or(0);
      let note_name = &raw_name[..name_end];
      pos = (pos + namesz + 3) & !3;

      if pos + descsz > note_data.len() {
        break;
      }
      let desc = &note_data[pos..pos + descsz];
      pos = (pos + descsz + 3) & !3;

      if note_name != b"SUI" || note_type != SUI_NOTE_TYPE {
        continue;
      }
      if desc.len() < 2 {
        continue;
      }
      let inner_len = u16::from_le_bytes(desc[0..2].try_into().ok()?) as usize;
      if desc.len() < 2 + inner_len {
        continue;
      }
      if &desc[2..2 + inner_len] == b"denover" {
        return Some(desc[2 + inner_len..].to_vec());
      }
    }
  }

  None
}

fn release_channel_from_version_string(version: &str) -> ReleaseChannel {
  let v = deno_semver::Version::parse_standard(version).ok();
  match v.and_then(|v| v.pre.first().map(|s| s.as_str().to_string())) {
    Some(ref s) if s == "alpha" => ReleaseChannel::Alpha,
    Some(ref s) if s == "beta" => ReleaseChannel::Beta,
    Some(ref s) if s == "rc" => ReleaseChannel::Rc,
    _ => ReleaseChannel::Stable,
  }
}
