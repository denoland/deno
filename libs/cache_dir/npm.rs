// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;

use deno_path_util::normalize_path;
use deno_path_util::url_from_directory_path;
use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use url::Url;

pub struct NpmCacheFolderId {
  /// Package name.
  pub name: String,
  /// Package version.
  pub version: String,
  /// Package copy index.
  pub copy_index: u8,
}

/// The global cache directory of npm packages.
#[derive(Clone, Debug)]
pub struct NpmCacheDir {
  root_dir: PathBuf,
  // cached url representation of the root directory
  root_dir_url: Url,
  // A list of all registry that were discovered via `.npmrc` files
  // turned into a safe directory names.
  known_registries_dirnames: Vec<String>,
}

impl NpmCacheDir {
  pub fn new<Sys: FsCanonicalize + FsCreateDirAll>(
    sys: &Sys,
    root_dir: PathBuf,
    known_registries_urls: Vec<Url>,
  ) -> Self {
    fn try_get_canonicalized_root_dir<Sys: FsCanonicalize + FsCreateDirAll>(
      sys: &Sys,
      root_dir: &Path,
    ) -> Result<PathBuf, std::io::Error> {
      match sys.fs_canonicalize(root_dir) {
        Ok(path) => Ok(path),
        Err(err) if err.kind() == ErrorKind::NotFound => {
          sys.fs_create_dir_all(root_dir)?;
          sys.fs_canonicalize(root_dir)
        }
        Err(err) => Err(err),
      }
    }

    // this may fail on readonly file systems, so just ignore if so
    let root_dir = normalize_path(Cow::Owned(root_dir));
    let root_dir = try_get_canonicalized_root_dir(sys, &root_dir)
      .map(Cow::Owned)
      .unwrap_or(root_dir);
    let root_dir_url = url_from_directory_path(&root_dir).unwrap();

    let known_registries_dirnames: Vec<_> = known_registries_urls
      .into_iter()
      .map(|url| {
        root_url_to_safe_local_dirname(&url)
          .to_string_lossy()
          .replace('\\', "/")
      })
      .collect();

    Self {
      root_dir: root_dir.into_owned(),
      root_dir_url,
      known_registries_dirnames,
    }
  }

  pub fn root_dir(&self) -> &Path {
    &self.root_dir
  }

  pub fn root_dir_url(&self) -> &Url {
    &self.root_dir_url
  }

  /// The safe local directory names (relative to `root_dir`) for every
  /// registry url known via `.npmrc` discovery. May contain multiple path
  /// segments when a registry url has a sub-path (e.g.
  /// `http://mirrors.example.com/npm/` -> `mirrors.example.com/npm`).
  pub fn known_registries_dirnames(&self) -> &[String] {
    &self.known_registries_dirnames
  }

  pub fn package_folder_for_id(
    &self,
    package_name: &str,
    package_version: &str,
    package_copy_index: u8,
    registry_url: &Url,
  ) -> PathBuf {
    if package_copy_index == 0 {
      self
        .package_name_folder(package_name, registry_url)
        .join(package_version)
    } else {
      self
        .package_name_folder(package_name, registry_url)
        .join(format!("{}_{}", package_version, package_copy_index))
    }
  }

  /// The global-cache folder holding a *built* variant of a package: the
  /// snapshot of a package directory taken after its lifecycle (build) scripts
  /// ran, keyed by an input hash so the same build can be reused across
  /// projects (pnpm's "side effects cache").
  ///
  /// This lives beside `package_folder_for_id`'s `{version}` /
  /// `{version}_{copy_index}` folders, using a `.build_{input_hash}` suffix.
  /// The `.` ensures it can never collide with the `_{copy_index}` variant
  /// naming: `resolve_package_folder_id_from_specifier` parses the trailing
  /// `_{n}` as a `u8` copy index, and `{version}.build_{hash}` fails that
  /// parse, so a built variant is never mistaken for a package folder.
  pub fn package_folder_for_id_built(
    &self,
    package_name: &str,
    package_version: &str,
    package_copy_index: u8,
    input_hash: &str,
    registry_url: &Url,
  ) -> PathBuf {
    let base = if package_copy_index == 0 {
      package_version.to_string()
    } else {
      format!("{}_{}", package_version, package_copy_index)
    };
    self
      .package_name_folder(package_name, registry_url)
      .join(format!("{base}.build_{input_hash}"))
  }

  pub fn package_name_folder(&self, name: &str, registry_url: &Url) -> PathBuf {
    let mut dir = self.registry_folder(registry_url);
    if name.to_lowercase() != name {
      let encoded_name = mixed_case_package_name_encode(name);
      // Using the encoded directory may have a collision with an actual package name
      // so prefix it with an underscore since npm packages can't start with that
      dir.join(format!("_{encoded_name}"))
    } else {
      // ensure backslashes are used on windows
      for part in name.split('/') {
        dir = dir.join(part);
      }
      dir
    }
  }

  fn registry_folder(&self, registry_url: &Url) -> PathBuf {
    self
      .root_dir
      .join(root_url_to_safe_local_dirname(registry_url))
  }

  pub fn resolve_package_folder_id_from_specifier(
    &self,
    specifier: &Url,
  ) -> Option<NpmCacheFolderId> {
    let mut maybe_relative_url = None;

    // Iterate through known registries and try to get a match.
    for registry_dirname in &self.known_registries_dirnames {
      let registry_root_dir = self
        .root_dir_url
        .join(&format!("{}/", registry_dirname))
        // this not succeeding indicates a fatal issue, so unwrap
        .unwrap();

      let Some(relative_url) = registry_root_dir.make_relative(specifier)
      else {
        continue;
      };

      if relative_url.starts_with("../") {
        continue;
      }

      maybe_relative_url = Some(relative_url);
      break;
    }

    let mut relative_url = maybe_relative_url?;

    // base32 decode the url if it starts with an underscore
    // * Ex. _{base32(package_name)}/
    if let Some(end_url) = relative_url.strip_prefix('_') {
      let mut parts = end_url
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
      match mixed_case_package_name_decode(&parts[0]) {
        Some(part) => {
          parts[0] = part;
        }
        None => return None,
      }
      relative_url = parts.join("/");
    }

    // examples:
    // * chalk/5.0.1/
    // * @types/chalk/5.0.1/
    // * some-package/5.0.1_1/ -- where the `_1` (/_\d+/) is a copy of the folder for peer deps
    let is_scoped_package = relative_url.starts_with('@');
    let mut parts = relative_url
      .split('/')
      .enumerate()
      .take(if is_scoped_package { 3 } else { 2 })
      .map(|(_, part)| part)
      .collect::<Vec<_>>();
    if parts.len() < 2 {
      return None;
    }
    let version_part = parts.pop().unwrap();
    let name = parts.join("/");
    let (version, copy_index) =
      if let Some((version, copy_count)) = version_part.split_once('_') {
        (version, copy_count.parse::<u8>().ok()?)
      } else {
        (version_part, 0)
      };
    Some(NpmCacheFolderId {
      name,
      version: version.to_string(),
      copy_index,
    })
  }
}

pub fn mixed_case_package_name_encode(name: &str) -> String {
  // use base32 encoding because it's reversible and the character set
  // only includes the characters within 0-9 and A-Z so it can be lower cased
  const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";

  let data = name.as_bytes();
  let mut ret = Vec::with_capacity((data.len() * 8).div_ceil(5));

  for chunk in data.chunks(5) {
    let mut buf = [0u8; 5];
    buf[..chunk.len()].copy_from_slice(chunk);

    ret.push(ALPHABET[((buf[0] & 0xF8) >> 3) as usize]);
    ret.push(
      ALPHABET[(((buf[0] & 0x07) << 2) | ((buf[1] & 0xC0) >> 6)) as usize],
    );
    ret.push(ALPHABET[((buf[1] & 0x3E) >> 1) as usize]);
    ret.push(
      ALPHABET[(((buf[1] & 0x01) << 4) | ((buf[2] & 0xF0) >> 4)) as usize],
    );
    ret.push(ALPHABET[(((buf[2] & 0x0F) << 1) | (buf[3] >> 7)) as usize]);
    ret.push(ALPHABET[((buf[3] & 0x7C) >> 2) as usize]);
    ret.push(
      ALPHABET[(((buf[3] & 0x03) << 3) | ((buf[4] & 0xE0) >> 5)) as usize],
    );
    ret.push(ALPHABET[(buf[4] & 0x1F) as usize]);
  }

  if !data.len().is_multiple_of(5) {
    let num_extra = 8 - (data.len() % 5 * 8).div_ceil(5);
    ret.truncate(ret.len() - num_extra);
  }

  String::from_utf8(ret).unwrap()
}

pub fn mixed_case_package_name_decode(name: &str) -> Option<String> {
  fn decode_value(c: u8) -> Option<u8> {
    match c {
      b'a'..=b'z' => Some(c - b'a'),
      b'2'..=b'7' => Some(c - b'2' + 26),
      _ => None,
    }
  }

  let data = name.as_bytes();
  let output_length = data.len() * 5 / 8;
  let mut ret = Vec::with_capacity(output_length.div_ceil(5) * 5);

  for chunk in data.chunks(8) {
    let mut buf = [0u8; 8];
    for (i, &c) in chunk.iter().enumerate() {
      buf[i] = decode_value(c)?;
    }

    ret.push((buf[0] << 3) | (buf[1] >> 2));
    ret.push((buf[1] << 6) | (buf[2] << 1) | (buf[3] >> 4));
    ret.push((buf[3] << 4) | (buf[4] >> 1));
    ret.push((buf[4] << 7) | (buf[5] << 2) | (buf[6] >> 3));
    ret.push((buf[6] << 5) | buf[7]);
  }

  ret.truncate(output_length);
  String::from_utf8(ret).ok()
}

/// Gets a safe local directory name for the provided url.
///
/// For example:
/// https://deno.land:8080/path -> deno.land_8080/path
fn root_url_to_safe_local_dirname(root: &Url) -> PathBuf {
  fn sanitize_segment(text: &str) -> String {
    text
      .chars()
      .map(|c| if is_banned_segment_char(c) { '_' } else { c })
      .collect()
  }

  fn is_banned_segment_char(c: char) -> bool {
    matches!(c, '/' | '\\') || is_banned_path_char(c)
  }

  let mut result = String::new();
  if let Some(domain) = root.domain() {
    result.push_str(&sanitize_segment(domain));
  }
  if let Some(port) = root.port() {
    if !result.is_empty() {
      result.push('_');
    }
    result.push_str(&port.to_string());
  }
  let mut result = PathBuf::from(result);
  if let Some(segments) = root.path_segments() {
    for segment in segments.filter(|s| !s.is_empty()) {
      result = result.join(sanitize_segment(segment));
    }
  }

  result
}

/// Gets if the provided character is not supported on all
/// kinds of file systems.
fn is_banned_path_char(c: char) -> bool {
  matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
}

#[cfg(test)]
mod test {
  use std::path::PathBuf;

  use sys_traits::FsCreateDirAll;
  use url::Url;

  use super::NpmCacheDir;
  use super::mixed_case_package_name_decode;
  use super::mixed_case_package_name_encode;

  #[test]
  fn should_get_package_folder() {
    let sys = sys_traits::impls::InMemorySys::default();
    let root_dir = if cfg!(windows) {
      PathBuf::from("C:\\cache")
    } else {
      PathBuf::from("/cache")
    };
    sys.fs_create_dir_all(&root_dir).unwrap();
    let registry_url = Url::parse("https://registry.npmjs.org/").unwrap();
    let cache =
      NpmCacheDir::new(&sys, root_dir.clone(), vec![registry_url.clone()]);

    assert_eq!(
      cache.package_folder_for_id("json", "1.2.5", 0, &registry_url,),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5"),
    );

    assert_eq!(
      cache.package_folder_for_id("json", "1.2.5", 1, &registry_url,),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5_1"),
    );

    assert_eq!(
      cache.package_folder_for_id("JSON", "2.1.5", 0, &registry_url,),
      root_dir
        .join("registry.npmjs.org")
        .join("_jjju6tq")
        .join("2.1.5"),
    );

    assert_eq!(
      cache.package_folder_for_id("@types/JSON", "2.1.5", 0, &registry_url,),
      root_dir
        .join("registry.npmjs.org")
        .join("_ib2hs4dfomxuuu2pjy")
        .join("2.1.5"),
    );
  }

  #[test]
  fn should_get_built_package_folder() {
    let sys = sys_traits::impls::InMemorySys::default();
    let root_dir = if cfg!(windows) {
      PathBuf::from("C:\\cache")
    } else {
      PathBuf::from("/cache")
    };
    sys.fs_create_dir_all(&root_dir).unwrap();
    let registry_url = Url::parse("https://registry.npmjs.org/").unwrap();
    let cache =
      NpmCacheDir::new(&sys, root_dir.clone(), vec![registry_url.clone()]);

    // The `.build_<hash>` suffix sits beside the pristine `{version}` folder.
    assert_eq!(
      cache.package_folder_for_id_built(
        "json",
        "1.2.5",
        0,
        "abc123",
        &registry_url
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5.build_abc123"),
    );

    // A copy index is preserved before the `.build_` suffix.
    assert_eq!(
      cache.package_folder_for_id_built(
        "json",
        "1.2.5",
        2,
        "abc123",
        &registry_url
      ),
      root_dir
        .join("registry.npmjs.org")
        .join("json")
        .join("1.2.5_2.build_abc123"),
    );

    // A built-variant directory name must not be parsed back as a package
    // folder id (the `.` makes the copy-index parse fail), so it can never be
    // confused with a real package version or `_{copy_index}` variant.
    let specifier = cache
      .root_dir_url()
      .join("registry.npmjs.org/json/1.2.5.build_abc123/")
      .unwrap();
    assert!(
      cache
        .resolve_package_folder_id_from_specifier(&specifier)
        .is_none()
    );
  }

  #[test]
  fn should_encode_and_decode_mixed_case_package_names() {
    let cases = [
      ("JSON", "jjju6tq"),
      ("@types/JSON", "ib2hs4dfomxuuu2pjy"),
      ("Deno_123", "irsw4327gezdg"),
    ];
    for (name, encoded) in cases {
      assert_eq!(mixed_case_package_name_encode(name), encoded);
      assert_eq!(
        mixed_case_package_name_decode(encoded).as_deref(),
        Some(name)
      );
    }
  }

  #[test]
  fn should_preserve_base32_decode_leniency() {
    assert_eq!(mixed_case_package_name_decode("j").as_deref(), Some(""));
    assert_eq!(mixed_case_package_name_decode("jj").as_deref(), Some("J"));
    assert_eq!(mixed_case_package_name_decode("JJ"), None);
    assert_eq!(mixed_case_package_name_decode("jj======"), None);
  }
}
