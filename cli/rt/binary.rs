// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_core::FastString;
use deno_core::ModuleSourceCode;
use deno_core::ModuleType;
use deno_error::JsErrorBox;
use deno_lib::standalone::binary::Metadata;
use deno_lib::standalone::binary::SourceMapStore;
use deno_lib::standalone::binary::MAGIC_BYTES;
use deno_lib::standalone::virtual_fs::VfsFileSubDataKind;
use deno_lib::standalone::virtual_fs::VirtualDirectory;
use deno_lib::standalone::virtual_fs::VirtualDirectoryEntries;
use deno_media_type::MediaType;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::SerializedNpmResolutionSnapshotPackage;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io::fs::FsError;
use deno_semver::package::PackageReq;
use deno_semver::StackString;

use crate::file_system::FileBackedVfs;
use crate::file_system::VfsRoot;

pub struct StandaloneData {
  pub metadata: Metadata,
  pub modules: StandaloneModules,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub root_path: PathBuf,
  pub source_maps: SourceMapStore,
  pub vfs: Arc<FileBackedVfs>,
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by skipping over the trailer width at the end of the file,
/// then checking for the magic trailer string `d3n0l4nd`. If found,
/// the bundle is executed. If not, this function exits with `Ok(None)`.
pub fn extract_standalone(
  cli_args: Cow<Vec<OsString>>,
) -> Result<Option<StandaloneData>, AnyError> {
  let Some(data) = libsui::find_section("d3n0l4nd") else {
    return Ok(None);
  };

  let DeserializedDataSection {
    mut metadata,
    npm_snapshot,
    remote_modules,
    source_maps,
    vfs_root_entries,
    vfs_files_data,
  } = match deserialize_binary_data_section(data)? {
    Some(data_section) => data_section,
    None => return Ok(None),
  };

  let root_path = {
    let maybe_current_exe = std::env::current_exe().ok();
    let current_exe_name = maybe_current_exe
      .as_ref()
      .and_then(|p| p.file_name())
      .map(|p| p.to_string_lossy())
      // should never happen
      .unwrap_or_else(|| Cow::Borrowed("binary"));
    std::env::temp_dir().join(format!("deno-compile-{}", current_exe_name))
  };
  let cli_args = cli_args.into_owned();
  metadata.argv.reserve(cli_args.len() - 1);
  for arg in cli_args.into_iter().skip(1) {
    metadata.argv.push(arg.into_string().unwrap());
  }
  let vfs = {
    let fs_root = VfsRoot {
      dir: VirtualDirectory {
        // align the name of the directory with the root dir
        name: root_path.file_name().unwrap().to_string_lossy().to_string(),
        entries: vfs_root_entries,
      },
      root_path: root_path.clone(),
      start_file_offset: 0,
    };
    Arc::new(FileBackedVfs::new(
      Cow::Borrowed(vfs_files_data),
      fs_root,
      metadata.vfs_case_sensitivity,
    ))
  };
  Ok(Some(StandaloneData {
    metadata,
    modules: StandaloneModules {
      remote_modules,
      vfs: vfs.clone(),
    },
    npm_snapshot,
    root_path,
    source_maps,
    vfs,
  }))
}

pub struct DeserializedDataSection {
  pub metadata: Metadata,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub remote_modules: RemoteModulesStore,
  pub source_maps: SourceMapStore,
  pub vfs_root_entries: VirtualDirectoryEntries,
  pub vfs_files_data: &'static [u8],
}

pub fn deserialize_binary_data_section(
  data: &'static [u8],
) -> Result<Option<DeserializedDataSection>, AnyError> {
  fn read_magic_bytes(input: &[u8]) -> Result<(&[u8], bool), AnyError> {
    if input.len() < MAGIC_BYTES.len() {
      bail!("Unexpected end of data. Could not find magic bytes.");
    }
    let (magic_bytes, input) = input.split_at(MAGIC_BYTES.len());
    if magic_bytes != MAGIC_BYTES {
      return Ok((input, false));
    }
    Ok((input, true))
  }

  #[allow(clippy::type_complexity)]
  fn read_source_map_entry(
    input: &[u8],
  ) -> Result<(&[u8], (Cow<str>, &[u8])), AnyError> {
    let (input, specifier) = read_string_lossy(input)?;
    let (input, source_map) = read_bytes_with_u32_len(input)?;
    Ok((input, (specifier, source_map)))
  }

  let (input, found) = read_magic_bytes(data)?;
  if !found {
    return Ok(None);
  }

  // 1. Metadata
  let (input, data) =
    read_bytes_with_u64_len(input).context("reading metadata")?;
  let metadata: Metadata =
    serde_json::from_slice(data).context("deserializing metadata")?;
  // 2. Npm snapshot
  let (input, data) =
    read_bytes_with_u64_len(input).context("reading npm snapshot")?;
  let npm_snapshot = if data.is_empty() {
    None
  } else {
    Some(deserialize_npm_snapshot(data).context("deserializing npm snapshot")?)
  };
  // 3. Remote modules
  let (input, remote_modules) =
    RemoteModulesStore::build(input).context("deserializing remote modules")?;
  // 4. VFS
  let (input, data) = read_bytes_with_u64_len(input).context("vfs")?;
  let vfs_root_entries: VirtualDirectoryEntries =
    serde_json::from_slice(data).context("deserializing vfs data")?;
  let (input, vfs_files_data) =
    read_bytes_with_u64_len(input).context("reading vfs files data")?;
  // 5. Source maps
  let (mut input, source_map_data_len) = read_u32_as_usize(input)?;
  let mut source_maps = SourceMapStore::with_capacity(source_map_data_len);
  for _ in 0..source_map_data_len {
    let (current_input, (specifier, source_map)) =
      read_source_map_entry(input)?;
    input = current_input;
    source_maps.add(specifier, Cow::Borrowed(source_map));
  }

  // finally ensure we read the magic bytes at the end
  let (_input, found) = read_magic_bytes(input)?;
  if !found {
    bail!("Could not find magic bytes at the end of the data.");
  }

  Ok(Some(DeserializedDataSection {
    metadata,
    npm_snapshot,
    remote_modules,
    source_maps,
    vfs_root_entries,
    vfs_files_data,
  }))
}

pub struct StandaloneModules {
  remote_modules: RemoteModulesStore,
  vfs: Arc<FileBackedVfs>,
}

impl StandaloneModules {
  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a Url,
  ) -> Result<Option<&'a Url>, JsErrorBox> {
    if specifier.scheme() == "file" {
      Ok(Some(specifier))
    } else {
      self.remote_modules.resolve_specifier(specifier)
    }
  }

  pub fn has_file(&self, path: &Path) -> bool {
    self.vfs.file_entry(path).is_ok()
  }

  pub fn read<'a>(
    &'a self,
    specifier: &'a Url,
    kind: VfsFileSubDataKind,
  ) -> Result<Option<DenoCompileModuleData<'a>>, AnyError> {
    if specifier.scheme() == "file" {
      let path = deno_path_util::url_to_file_path(specifier)?;
      let bytes = match self.vfs.file_entry(&path) {
        Ok(entry) => self.vfs.read_file_all(entry, kind)?,
        Err(err) if err.kind() == ErrorKind::NotFound => {
          match RealFs.read_file_sync(&path, None) {
            Ok(bytes) => bytes,
            Err(FsError::Io(err)) if err.kind() == ErrorKind::NotFound => {
              return Ok(None)
            }
            Err(err) => return Err(err.into()),
          }
        }
        Err(err) => return Err(err.into()),
      };
      Ok(Some(DenoCompileModuleData {
        media_type: MediaType::from_specifier(specifier),
        specifier,
        data: bytes,
      }))
    } else {
      self.remote_modules.read(specifier).map(|maybe_entry| {
        maybe_entry.map(|entry| DenoCompileModuleData {
          media_type: entry.media_type,
          specifier: entry.specifier,
          data: match kind {
            VfsFileSubDataKind::Raw => entry.data,
            VfsFileSubDataKind::ModuleGraph => {
              entry.transpiled_data.unwrap_or(entry.data)
            }
          },
        })
      })
    }
  }
}

pub struct DenoCompileModuleData<'a> {
  pub specifier: &'a Url,
  pub media_type: MediaType,
  pub data: Cow<'static, [u8]>,
}

impl<'a> DenoCompileModuleData<'a> {
  pub fn into_parts(self) -> (&'a Url, ModuleType, DenoCompileModuleSource) {
    fn into_string_unsafe(data: Cow<'static, [u8]>) -> DenoCompileModuleSource {
      match data {
        Cow::Borrowed(d) => DenoCompileModuleSource::String(
          // SAFETY: we know this is a valid utf8 string
          unsafe { std::str::from_utf8_unchecked(d) },
        ),
        Cow::Owned(d) => DenoCompileModuleSource::Bytes(Cow::Owned(d)),
      }
    }

    let (media_type, source) = match self.media_type {
      MediaType::JavaScript
      | MediaType::Jsx
      | MediaType::Mjs
      | MediaType::Cjs
      | MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => {
        (ModuleType::JavaScript, into_string_unsafe(self.data))
      }
      MediaType::Json => (ModuleType::Json, into_string_unsafe(self.data)),
      MediaType::Wasm => {
        (ModuleType::Wasm, DenoCompileModuleSource::Bytes(self.data))
      }
      // just assume javascript if we made it here
      MediaType::Css | MediaType::SourceMap | MediaType::Unknown => (
        ModuleType::JavaScript,
        DenoCompileModuleSource::Bytes(self.data),
      ),
    };
    (self.specifier, media_type, source)
  }
}

pub enum DenoCompileModuleSource {
  String(&'static str),
  Bytes(Cow<'static, [u8]>),
}

impl DenoCompileModuleSource {
  pub fn into_for_v8(self) -> ModuleSourceCode {
    fn into_bytes(data: Cow<'static, [u8]>) -> ModuleSourceCode {
      ModuleSourceCode::Bytes(match data {
        Cow::Borrowed(d) => d.into(),
        Cow::Owned(d) => d.into_boxed_slice().into(),
      })
    }

    match self {
      // todo(https://github.com/denoland/deno_core/pull/943): store whether
      // the string is ascii or not ahead of time so we can avoid the is_ascii()
      // check in FastString::from_static
      Self::String(s) => ModuleSourceCode::String(FastString::from_static(s)),
      Self::Bytes(b) => into_bytes(b),
    }
  }
}

pub struct RemoteModuleEntry<'a> {
  pub specifier: &'a Url,
  pub media_type: MediaType,
  pub data: Cow<'static, [u8]>,
  pub transpiled_data: Option<Cow<'static, [u8]>>,
}

enum RemoteModulesStoreSpecifierValue {
  Data(usize),
  Redirect(Url),
}

pub struct RemoteModulesStore {
  specifiers: HashMap<Url, RemoteModulesStoreSpecifierValue>,
  files_data: &'static [u8],
}

impl RemoteModulesStore {
  fn build(input: &'static [u8]) -> Result<(&'static [u8], Self), AnyError> {
    fn read_specifier(input: &[u8]) -> Result<(&[u8], (Url, u64)), AnyError> {
      let (input, specifier) = read_string_lossy(input)?;
      let specifier = Url::parse(&specifier)?;
      let (input, offset) = read_u64(input)?;
      Ok((input, (specifier, offset)))
    }

    fn read_redirect(input: &[u8]) -> Result<(&[u8], (Url, Url)), AnyError> {
      let (input, from) = read_string_lossy(input)?;
      let from = Url::parse(&from)?;
      let (input, to) = read_string_lossy(input)?;
      let to = Url::parse(&to)?;
      Ok((input, (from, to)))
    }

    fn read_headers(
      input: &[u8],
    ) -> Result<(&[u8], HashMap<Url, RemoteModulesStoreSpecifierValue>), AnyError>
    {
      let (input, specifiers_len) = read_u32_as_usize(input)?;
      let (mut input, redirects_len) = read_u32_as_usize(input)?;
      let mut specifiers =
        HashMap::with_capacity(specifiers_len + redirects_len);
      for _ in 0..specifiers_len {
        let (current_input, (specifier, offset)) =
          read_specifier(input).context("reading specifier")?;
        input = current_input;
        specifiers.insert(
          specifier,
          RemoteModulesStoreSpecifierValue::Data(offset as usize),
        );
      }

      for _ in 0..redirects_len {
        let (current_input, (from, to)) = read_redirect(input)?;
        input = current_input;
        specifiers.insert(from, RemoteModulesStoreSpecifierValue::Redirect(to));
      }

      Ok((input, specifiers))
    }

    let (input, specifiers) = read_headers(input)?;
    let (input, files_data) = read_bytes_with_u64_len(input)?;

    Ok((
      input,
      Self {
        specifiers,
        files_data,
      },
    ))
  }

  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a Url,
  ) -> Result<Option<&'a Url>, JsErrorBox> {
    let mut count = 0;
    let mut current = specifier;
    loop {
      if count > 10 {
        return Err(JsErrorBox::generic(format!(
          "Too many redirects resolving '{}'",
          specifier
        )));
      }
      match self.specifiers.get(current) {
        Some(RemoteModulesStoreSpecifierValue::Redirect(to)) => {
          current = to;
          count += 1;
        }
        Some(RemoteModulesStoreSpecifierValue::Data(_)) => {
          return Ok(Some(current));
        }
        None => {
          return Ok(None);
        }
      }
    }
  }

  pub fn read<'a>(
    &'a self,
    original_specifier: &'a Url,
  ) -> Result<Option<RemoteModuleEntry<'a>>, AnyError> {
    let mut count = 0;
    let mut specifier = original_specifier;
    loop {
      if count > 10 {
        bail!("Too many redirects resolving '{}'", original_specifier);
      }
      match self.specifiers.get(specifier) {
        Some(RemoteModulesStoreSpecifierValue::Redirect(to)) => {
          specifier = to;
          count += 1;
        }
        Some(RemoteModulesStoreSpecifierValue::Data(offset)) => {
          let input = &self.files_data[*offset..];
          let (input, media_type_byte) = read_bytes(input, 1)?;
          let media_type = deserialize_media_type(media_type_byte[0])?;
          let (input, data) = read_bytes_with_u32_len(input)?;
          check_has_len(input, 1)?;
          let (input, has_transpiled) = (&input[1..], input[0]);
          let (_, transpiled_data) = match has_transpiled {
            0 => (input, None),
            1 => {
              let (input, data) = read_bytes_with_u32_len(input)?;
              (input, Some(data))
            }
            value => bail!(
              "Invalid transpiled data flag: {}. Compiled data is corrupt.",
              value
            ),
          };
          return Ok(Some(RemoteModuleEntry {
            specifier,
            media_type,
            data: Cow::Borrowed(data),
            transpiled_data: transpiled_data.map(Cow::Borrowed),
          }));
        }
        None => {
          return Ok(None);
        }
      }
    }
  }
}

fn deserialize_npm_snapshot(
  input: &[u8],
) -> Result<ValidSerializedNpmResolutionSnapshot, AnyError> {
  fn parse_id(input: &[u8]) -> Result<(&[u8], NpmPackageId), AnyError> {
    let (input, id) = read_string_lossy(input)?;
    let id = NpmPackageId::from_serialized(&id)?;
    Ok((input, id))
  }

  #[allow(clippy::needless_lifetimes)] // clippy bug
  fn parse_root_package<'a>(
    id_to_npm_id: &'a impl Fn(usize) -> Result<NpmPackageId, AnyError>,
  ) -> impl Fn(&[u8]) -> Result<(&[u8], (PackageReq, NpmPackageId)), AnyError> + 'a
  {
    |input| {
      let (input, req) = read_string_lossy(input)?;
      let req = PackageReq::from_str(&req)?;
      let (input, id) = read_u32_as_usize(input)?;
      Ok((input, (req, id_to_npm_id(id)?)))
    }
  }

  #[allow(clippy::needless_lifetimes)] // clippy bug
  fn parse_package_dep<'a>(
    id_to_npm_id: &'a impl Fn(usize) -> Result<NpmPackageId, AnyError>,
  ) -> impl Fn(&[u8]) -> Result<(&[u8], (StackString, NpmPackageId)), AnyError> + 'a
  {
    |input| {
      let (input, req) = read_string_lossy(input)?;
      let (input, id) = read_u32_as_usize(input)?;
      let req = StackString::from_cow(req);
      Ok((input, (req, id_to_npm_id(id)?)))
    }
  }

  fn parse_package<'a>(
    input: &'a [u8],
    id: NpmPackageId,
    id_to_npm_id: &impl Fn(usize) -> Result<NpmPackageId, AnyError>,
  ) -> Result<(&'a [u8], SerializedNpmResolutionSnapshotPackage), AnyError> {
    let (input, deps_len) = read_u32_as_usize(input)?;
    let (input, dependencies) =
      parse_hashmap_n_times(input, deps_len, parse_package_dep(id_to_npm_id))?;
    Ok((
      input,
      SerializedNpmResolutionSnapshotPackage {
        id,
        system: Default::default(),
        dist: Default::default(),
        dependencies,
        optional_dependencies: Default::default(),
        bin: None,
        scripts: Default::default(),
        deprecated: Default::default(),
      },
    ))
  }

  let (input, packages_len) = read_u32_as_usize(input)?;

  // get a hashmap of all the npm package ids to their serialized ids
  let (input, data_ids_to_npm_ids) =
    parse_vec_n_times(input, packages_len, parse_id)
      .context("deserializing id")?;
  let data_id_to_npm_id = |id: usize| {
    data_ids_to_npm_ids
      .get(id)
      .cloned()
      .ok_or_else(|| deno_core::anyhow::anyhow!("Invalid npm package id"))
  };

  let (input, root_packages_len) = read_u32_as_usize(input)?;
  let (input, root_packages) = parse_hashmap_n_times(
    input,
    root_packages_len,
    parse_root_package(&data_id_to_npm_id),
  )
  .context("deserializing root package")?;
  let (input, packages) =
    parse_vec_n_times_with_index(input, packages_len, |input, index| {
      parse_package(input, data_id_to_npm_id(index)?, &data_id_to_npm_id)
    })
    .context("deserializing package")?;

  if !input.is_empty() {
    bail!("Unexpected data left over");
  }

  Ok(
    SerializedNpmResolutionSnapshot {
      packages,
      root_packages,
    }
    // this is ok because we have already verified that all the
    // identifiers found in the snapshot are valid via the
    // npm package id -> npm package id mapping
    .into_valid_unsafe(),
  )
}

fn deserialize_media_type(value: u8) -> Result<MediaType, AnyError> {
  match value {
    0 => Ok(MediaType::JavaScript),
    1 => Ok(MediaType::Jsx),
    2 => Ok(MediaType::Mjs),
    3 => Ok(MediaType::Cjs),
    4 => Ok(MediaType::TypeScript),
    5 => Ok(MediaType::Mts),
    6 => Ok(MediaType::Cts),
    7 => Ok(MediaType::Dts),
    8 => Ok(MediaType::Dmts),
    9 => Ok(MediaType::Dcts),
    10 => Ok(MediaType::Tsx),
    11 => Ok(MediaType::Json),
    12 => Ok(MediaType::Wasm),
    13 => Ok(MediaType::Css),
    14 => Ok(MediaType::SourceMap),
    15 => Ok(MediaType::Unknown),
    _ => bail!("Unknown media type value: {}", value),
  }
}

fn parse_hashmap_n_times<TKey: std::cmp::Eq + std::hash::Hash, TValue>(
  mut input: &[u8],
  times: usize,
  parse: impl Fn(&[u8]) -> Result<(&[u8], (TKey, TValue)), AnyError>,
) -> Result<(&[u8], HashMap<TKey, TValue>), AnyError> {
  let mut results = HashMap::with_capacity(times);
  for _ in 0..times {
    let result = parse(input);
    let (new_input, (key, value)) = result?;
    results.insert(key, value);
    input = new_input;
  }
  Ok((input, results))
}

fn parse_vec_n_times<TResult>(
  input: &[u8],
  times: usize,
  parse: impl Fn(&[u8]) -> Result<(&[u8], TResult), AnyError>,
) -> Result<(&[u8], Vec<TResult>), AnyError> {
  parse_vec_n_times_with_index(input, times, |input, _index| parse(input))
}

fn parse_vec_n_times_with_index<TResult>(
  mut input: &[u8],
  times: usize,
  parse: impl Fn(&[u8], usize) -> Result<(&[u8], TResult), AnyError>,
) -> Result<(&[u8], Vec<TResult>), AnyError> {
  let mut results = Vec::with_capacity(times);
  for i in 0..times {
    let result = parse(input, i);
    let (new_input, result) = result?;
    results.push(result);
    input = new_input;
  }
  Ok((input, results))
}

fn read_bytes_with_u64_len(input: &[u8]) -> Result<(&[u8], &[u8]), AnyError> {
  let (input, len) = read_u64(input)?;
  let (input, data) = read_bytes(input, len as usize)?;
  Ok((input, data))
}

fn read_bytes_with_u32_len(input: &[u8]) -> Result<(&[u8], &[u8]), AnyError> {
  let (input, len) = read_u32_as_usize(input)?;
  let (input, data) = read_bytes(input, len)?;
  Ok((input, data))
}

fn read_bytes(input: &[u8], len: usize) -> Result<(&[u8], &[u8]), AnyError> {
  check_has_len(input, len)?;
  let (len_bytes, input) = input.split_at(len);
  Ok((input, len_bytes))
}

#[inline(always)]
fn check_has_len(input: &[u8], len: usize) -> Result<(), AnyError> {
  if input.len() < len {
    bail!("Unexpected end of data.");
  }
  Ok(())
}

fn read_string_lossy(input: &[u8]) -> Result<(&[u8], Cow<str>), AnyError> {
  let (input, data_bytes) = read_bytes_with_u32_len(input)?;
  Ok((input, String::from_utf8_lossy(data_bytes)))
}

fn read_u32_as_usize(input: &[u8]) -> Result<(&[u8], usize), AnyError> {
  let (input, len_bytes) = read_bytes(input, 4)?;
  let len = u32::from_le_bytes(len_bytes.try_into()?);
  Ok((input, len as usize))
}

fn read_u64(input: &[u8]) -> Result<(&[u8], u64), AnyError> {
  let (input, len_bytes) = read_bytes(input, 8)?;
  let len = u64::from_le_bytes(len_bytes.try_into()?);
  Ok((input, len))
}
