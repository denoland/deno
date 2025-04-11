// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use lsp_types::Uri;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::assertions::assert_wildcard_match;
use crate::testdata_path;

/// Characters that are left unencoded in a `Url` path but will be encoded in a
/// VSCode URI.
const URL_TO_URI_PATH: &percent_encoding::AsciiSet =
  &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'$')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b']')
    .add(b'^')
    .add(b'|');

/// Characters that may be left unencoded in a `Url` query but not valid in a
/// `Uri` query.
const URL_TO_URI_QUERY: &percent_encoding::AsciiSet =
  &URL_TO_URI_PATH.add(b'\\').add(b'`').add(b'{').add(b'}');

/// Characters that may be left unencoded in a `Url` fragment but not valid in
/// a `Uri` fragment.
const URL_TO_URI_FRAGMENT: &percent_encoding::AsciiSet =
  &URL_TO_URI_PATH.add(b'#').add(b'\\').add(b'{').add(b'}');

pub fn url_to_uri(url: &Url) -> Result<Uri, anyhow::Error> {
  let components = url::quirks::internal_components(url);
  let mut input = String::with_capacity(url.as_str().len());
  input.push_str(&url.as_str()[..components.path_start as usize]);
  let path = url.path();
  let mut chars = path.chars();
  let has_drive_letter = chars.next().is_some_and(|c| c == '/')
    && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
    && chars.next().is_some_and(|c| c == ':')
    && chars.next().is_none_or(|c| c == '/');
  if has_drive_letter {
    let (dl_part, rest) = path.split_at(2);
    input.push_str(&dl_part.to_ascii_lowercase());
    input.push_str(
      &percent_encoding::utf8_percent_encode(rest, URL_TO_URI_PATH).to_string(),
    );
  } else {
    input.push_str(
      &percent_encoding::utf8_percent_encode(path, URL_TO_URI_PATH).to_string(),
    );
  }
  if let Some(query) = url.query() {
    input.push('?');
    input.push_str(
      &percent_encoding::utf8_percent_encode(query, URL_TO_URI_QUERY)
        .to_string(),
    );
  }
  if let Some(fragment) = url.fragment() {
    input.push('#');
    input.push_str(
      &percent_encoding::utf8_percent_encode(fragment, URL_TO_URI_FRAGMENT)
        .to_string(),
    );
  }
  Uri::from_str(&input).map_err(|err| {
    anyhow::anyhow!("Could not convert URL \"{url}\" to URI: {err}")
  })
}

pub fn url_to_notebook_cell_uri(url: &Url) -> Uri {
  let uri = url_to_uri(url).unwrap();
  Uri::from_str(&format!(
    "vscode-notebook-cell:{}",
    uri.as_str().strip_prefix("file:").unwrap()
  ))
  .unwrap()
}

/// Represents a path on the file system, which can be used
/// to perform specific actions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct PathRef(PathBuf);

impl AsRef<Path> for PathRef {
  fn as_ref(&self) -> &Path {
    self.as_path()
  }
}

impl AsRef<OsStr> for PathRef {
  fn as_ref(&self) -> &OsStr {
    self.as_path().as_ref()
  }
}

impl std::fmt::Display for PathRef {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.as_path().display())
  }
}

impl PathRef {
  pub fn new(path: impl AsRef<Path>) -> Self {
    Self(path.as_ref().to_path_buf())
  }

  pub fn parent(&self) -> PathRef {
    PathRef(self.as_path().parent().unwrap().to_path_buf())
  }

  pub fn url_dir(&self) -> Url {
    Url::from_directory_path(self.as_path()).unwrap()
  }

  pub fn url_file(&self) -> Url {
    Url::from_file_path(self.as_path()).unwrap()
  }

  pub fn uri_dir(&self) -> Uri {
    url_to_uri(&self.url_dir()).unwrap()
  }

  pub fn uri_file(&self) -> Uri {
    url_to_uri(&self.url_file()).unwrap()
  }

  pub fn as_path(&self) -> &Path {
    self.0.as_path()
  }

  pub fn to_path_buf(&self) -> PathBuf {
    self.0.to_path_buf()
  }

  pub fn to_string_lossy(&self) -> Cow<str> {
    self.0.to_string_lossy()
  }

  pub fn exists(&self) -> bool {
    self.0.exists()
  }

  pub fn try_exists(&self) -> std::io::Result<bool> {
    self.0.try_exists()
  }

  pub fn is_dir(&self) -> bool {
    self.0.is_dir()
  }

  pub fn is_file(&self) -> bool {
    self.0.is_file()
  }

  pub fn join(&self, path: impl AsRef<Path>) -> PathRef {
    PathRef(self.as_path().join(path))
  }

  pub fn with_extension(&self, ext: impl AsRef<OsStr>) -> PathRef {
    PathRef(self.as_path().with_extension(ext))
  }

  pub fn canonicalize(&self) -> PathRef {
    PathRef(strip_unc_prefix(self.as_path().canonicalize().unwrap()))
  }

  pub fn create_dir_all(&self) {
    fs::create_dir_all(self).unwrap();
  }

  pub fn remove_file(&self) {
    fs::remove_file(self).unwrap();
  }

  pub fn remove_dir_all(&self) {
    fs::remove_dir_all(self).unwrap();
  }

  pub fn read_to_string(&self) -> String {
    self.read_to_string_if_exists().unwrap()
  }

  pub fn read_to_string_if_exists(&self) -> Result<String, anyhow::Error> {
    fs::read_to_string(self)
      .with_context(|| format!("Could not read file: {}", self))
  }

  pub fn read_to_bytes_if_exists(&self) -> Result<Vec<u8>, anyhow::Error> {
    fs::read(self).with_context(|| format!("Could not read file: {}", self))
  }

  #[track_caller]
  pub fn read_json<TValue: DeserializeOwned>(&self) -> TValue {
    serde_json::from_str(&self.read_to_string())
      .with_context(|| format!("Failed deserializing: {}", self))
      .unwrap()
  }

  #[track_caller]
  pub fn read_json_value(&self) -> serde_json::Value {
    serde_json::from_str(&self.read_to_string())
      .with_context(|| format!("Failed deserializing: {}", self))
      .unwrap()
  }

  #[track_caller]
  pub fn read_jsonc_value(&self) -> serde_json::Value {
    jsonc_parser::parse_to_serde_value(
      &self.read_to_string(),
      &Default::default(),
    )
    .with_context(|| format!("Failed to parse {}", self))
    .unwrap()
    .unwrap_or_else(|| panic!("JSON file was empty for {}", self))
  }

  #[track_caller]
  pub fn rename(&self, to: impl AsRef<Path>) {
    let to = self.join(to);
    if let Some(parent_path) = to.as_path().parent() {
      fs::create_dir_all(parent_path).unwrap()
    }
    fs::rename(self, to).unwrap();
  }

  #[track_caller]
  pub fn append(&self, text: impl AsRef<str>) {
    let mut file = OpenOptions::new().append(true).open(self).unwrap();
    file.write_all(text.as_ref().as_bytes()).unwrap();
  }

  #[track_caller]
  pub fn write(&self, text: impl AsRef<[u8]>) {
    if let Some(parent_path) = self.as_path().parent() {
      fs::create_dir_all(parent_path).unwrap()
    }
    fs::write(self, text).unwrap();
  }

  #[track_caller]
  pub fn write_json<TValue: Serialize>(&self, value: &TValue) {
    let text = serde_json::to_string_pretty(value).unwrap();
    self.write(text);
  }

  #[track_caller]
  pub fn symlink_dir(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) {
    let oldpath = self.as_path().join(oldpath);
    let newpath = self.as_path().join(newpath);
    if let Some(parent_path) = newpath.parent() {
      fs::create_dir_all(parent_path).unwrap()
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(oldpath, newpath).unwrap();
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::symlink_dir;
      symlink_dir(oldpath, newpath).unwrap();
    }
  }

  #[track_caller]
  pub fn symlink_file(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) {
    let oldpath = self.as_path().join(oldpath);
    let newpath = self.as_path().join(newpath);
    if let Some(parent_path) = newpath.as_path().parent() {
      fs::create_dir_all(parent_path).unwrap()
    }
    #[cfg(unix)]
    {
      use std::os::unix::fs::symlink;
      symlink(oldpath, newpath).unwrap();
    }
    #[cfg(not(unix))]
    {
      use std::os::windows::fs::symlink_file;
      symlink_file(oldpath, newpath).unwrap();
    }
  }

  #[track_caller]
  pub fn read_dir(&self) -> fs::ReadDir {
    fs::read_dir(self.as_path())
      .with_context(|| format!("Reading {}", self.as_path().display()))
      .unwrap()
  }

  #[track_caller]
  pub fn copy(&self, to: &impl AsRef<Path>) {
    std::fs::copy(self.as_path(), to)
      .with_context(|| format!("Copying {} to {}", self, to.as_ref().display()))
      .unwrap();
  }

  /// Copies this directory to another directory.
  ///
  /// Note: Does not handle symlinks.
  pub fn copy_to_recursive(&self, to: &PathRef) {
    self.copy_to_recursive_with_exclusions(to, &HashSet::new())
  }

  pub fn copy_to_recursive_with_exclusions(
    &self,
    to: &PathRef,
    file_exclusions: &HashSet<PathRef>,
  ) {
    to.create_dir_all();
    let read_dir = self.read_dir();

    for entry in read_dir {
      let entry = entry.unwrap();
      let file_type = entry.file_type().unwrap();
      let new_from = self.join(entry.file_name());
      let new_to = to.join(entry.file_name());

      if file_type.is_dir() {
        new_from.copy_to_recursive(&new_to);
      } else if file_type.is_file() && !file_exclusions.contains(&new_from) {
        new_from.copy(&new_to);
      }
    }
  }

  #[track_caller]
  pub fn mark_executable(&self) {
    if cfg!(unix) {
      Command::new("chmod").arg("+x").arg(self).output().unwrap();
    }
  }

  #[track_caller]
  pub fn make_dir_readonly(&self) {
    self.create_dir_all();
    if cfg!(windows) {
      Command::new("attrib").arg("+r").arg(self).output().unwrap();
    } else if cfg!(unix) {
      Command::new("chmod").arg("555").arg(self).output().unwrap();
    }
  }

  #[track_caller]
  pub fn assert_matches_file(&self, wildcard_file: impl AsRef<Path>) -> &Self {
    let wildcard_file = testdata_path().join(wildcard_file);
    #[allow(clippy::print_stdout)]
    {
      println!("output path {}", wildcard_file);
    }
    let expected_text = wildcard_file.read_to_string();
    self.assert_matches_text(&expected_text)
  }

  #[track_caller]
  pub fn assert_matches_text(&self, wildcard_text: impl AsRef<str>) -> &Self {
    let actual = self.read_to_string();
    assert_wildcard_match(&actual, wildcard_text.as_ref());
    self
  }

  #[track_caller]
  pub fn assert_matches_json(&self, expected: serde_json::Value) {
    let actual_json = self.read_json_value();
    if actual_json != expected {
      let actual_text = serde_json::to_string_pretty(&actual_json).unwrap();
      let expected_text = serde_json::to_string_pretty(&expected).unwrap();
      assert_eq!(actual_text, expected_text);
    }
  }
}

#[cfg(not(windows))]
#[inline]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
  path
}

/// Strips the unc prefix (ex. \\?\) from Windows paths.
///
/// Lifted from deno_core for use in the tests.
#[cfg(windows)]
fn strip_unc_prefix(path: PathBuf) -> PathBuf {
  use std::path::Component;
  use std::path::Prefix;

  let mut components = path.components();
  match components.next() {
    Some(Component::Prefix(prefix)) => {
      match prefix.kind() {
        // \\?\device
        Prefix::Verbatim(device) => {
          let mut path = PathBuf::new();
          path.push(format!(r"\\{}\", device.to_string_lossy()));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        // \\?\c:\path
        Prefix::VerbatimDisk(_) => {
          let mut path = PathBuf::new();
          path.push(prefix.as_os_str().to_string_lossy().replace(r"\\?\", ""));
          path.extend(components);
          path
        }
        // \\?\UNC\hostname\share_name\path
        Prefix::VerbatimUNC(hostname, share_name) => {
          let mut path = PathBuf::new();
          path.push(format!(
            r"\\{}\{}\",
            hostname.to_string_lossy(),
            share_name.to_string_lossy()
          ));
          path.extend(components.filter(|c| !matches!(c, Component::RootDir)));
          path
        }
        _ => path,
      }
    }
    _ => path,
  }
}

enum TempDirInner {
  TempDir {
    path_ref: PathRef,
    // kept alive for the duration of the temp dir
    _dir: tempfile::TempDir,
  },
  Path(PathRef),
  Symlinked {
    symlink: Arc<TempDirInner>,
    target: Arc<TempDirInner>,
  },
}

impl TempDirInner {
  pub fn path(&self) -> &PathRef {
    match self {
      Self::Path(path_ref) => path_ref,
      Self::TempDir { path_ref, .. } => path_ref,
      Self::Symlinked { symlink, .. } => symlink.path(),
    }
  }

  pub fn target_path(&self) -> &PathRef {
    match self {
      TempDirInner::Symlinked { target, .. } => target.target_path(),
      _ => self.path(),
    }
  }
}

impl Drop for TempDirInner {
  fn drop(&mut self) {
    if let Self::Path(path) = self {
      _ = fs::remove_dir_all(path);
    }
  }
}

/// For creating temporary directories in tests.
///
/// This was done because `tempfiles::TempDir` was very slow on Windows.
///
/// Note: Do not use this in actual code as this does not protect against
/// "insecure temporary file" security vulnerabilities.
#[derive(Clone)]
pub struct TempDir(Arc<TempDirInner>);

impl Default for TempDir {
  fn default() -> Self {
    Self::new()
  }
}

impl TempDir {
  pub fn new() -> Self {
    Self::new_inner(&std::env::temp_dir(), None)
  }

  pub fn new_with_prefix(prefix: &str) -> Self {
    Self::new_inner(&std::env::temp_dir(), Some(prefix))
  }

  pub fn new_in(parent_dir: &Path) -> Self {
    Self::new_inner(parent_dir, None)
  }

  pub fn new_with_path(path: &Path) -> Self {
    Self(Arc::new(TempDirInner::Path(PathRef(path.to_path_buf()))))
  }

  pub fn new_symlinked(target: TempDir) -> Self {
    let target_path = target.path();
    let path = target_path.parent().join(format!(
      "{}_symlinked",
      target_path.as_path().file_name().unwrap().to_str().unwrap()
    ));
    target.symlink_dir(target.path(), &path);
    TempDir(Arc::new(TempDirInner::Symlinked {
      target: target.0,
      symlink: Self::new_with_path(path.as_path()).0,
    }))
  }

  /// Create a new temporary directory with the given prefix as part of its name, if specified.
  fn new_inner(parent_dir: &Path, prefix: Option<&str>) -> Self {
    let mut builder = tempfile::Builder::new();
    builder.prefix(prefix.unwrap_or("deno-cli-test"));
    let dir = builder
      .tempdir_in(parent_dir)
      .expect("Failed to create a temporary directory");
    Self(Arc::new(TempDirInner::TempDir {
      path_ref: PathRef(dir.path().to_path_buf()),
      _dir: dir,
    }))
  }

  pub fn url(&self) -> Url {
    Url::from_directory_path(self.path()).unwrap()
  }

  pub fn uri(&self) -> Uri {
    url_to_uri(&self.url()).unwrap()
  }

  pub fn path(&self) -> &PathRef {
    self.0.path()
  }

  /// The resolved final target path if this is a symlink.
  pub fn target_path(&self) -> &PathRef {
    self.0.target_path()
  }

  pub fn create_dir_all(&self, path: impl AsRef<Path>) {
    self.target_path().join(path).create_dir_all()
  }

  pub fn remove_file(&self, path: impl AsRef<Path>) {
    self.target_path().join(path).remove_file()
  }

  pub fn remove_dir_all(&self, path: impl AsRef<Path>) {
    self.target_path().join(path).remove_dir_all()
  }

  pub fn read_to_string(&self, path: impl AsRef<Path>) -> String {
    self.target_path().join(path).read_to_string()
  }

  pub fn rename(&self, from: impl AsRef<Path>, to: impl AsRef<Path>) {
    self.target_path().join(from).rename(to)
  }

  pub fn write(&self, path: impl AsRef<Path>, text: impl AsRef<[u8]>) {
    self.target_path().join(path).write(text)
  }

  pub fn symlink_dir(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) {
    self.target_path().symlink_dir(oldpath, newpath)
  }

  pub fn symlink_file(
    &self,
    oldpath: impl AsRef<Path>,
    newpath: impl AsRef<Path>,
  ) {
    self.target_path().symlink_file(oldpath, newpath)
  }
}
