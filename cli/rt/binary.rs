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
use deno_error::JsError;
use deno_error::JsErrorBox;
use deno_lib::standalone::binary::DenoRtDeserializable;
use deno_lib::standalone::binary::Metadata;
use deno_lib::standalone::binary::RemoteModuleEntry;
use deno_lib::standalone::binary::SpecifierDataStore;
use deno_lib::standalone::binary::SpecifierId;
use deno_lib::standalone::binary::MAGIC_BYTES;
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
use indexmap::IndexMap;
use thiserror::Error;

use crate::file_system::FileBackedVfs;
use crate::file_system::VfsRoot;

pub struct StandaloneData {
  pub metadata: Metadata,
  pub modules: Arc<StandaloneModules>,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub root_path: PathBuf,
  pub vfs: Arc<FileBackedVfs>,
}

/// This function will try to run this binary as a standalone binary
/// produced by `deno compile`. It determines if this is a standalone
/// binary by skipping over the trailer width at the end of the file,
/// then checking for the magic trailer string `d3n0l4nd`. If found,
/// the bundle is executed. If not, this function exits with `Ok(None)`.
pub fn extract_standalone(
  cli_args: Cow<Vec<OsString>>,
) -> Result<StandaloneData, AnyError> {
  let Some(data) = libsui::find_section("d3n0l4nd") else {
    bail!("Could not find standalone binary section.")
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
  let root_url = deno_path_util::url_from_directory_path(&root_path)?;

  let DeserializedDataSection {
    mut metadata,
    npm_snapshot,
    modules_store: remote_modules,
    vfs_root_entries,
    vfs_files_data,
  } = deserialize_binary_data_section(&root_url, data)?;

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
  Ok(StandaloneData {
    metadata,
    modules: Arc::new(StandaloneModules {
      modules: remote_modules,
      vfs: vfs.clone(),
    }),
    npm_snapshot,
    root_path,
    vfs,
  })
}

pub struct DeserializedDataSection {
  pub metadata: Metadata,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub modules_store: RemoteModulesStore,
  pub vfs_root_entries: VirtualDirectoryEntries,
  pub vfs_files_data: &'static [u8],
}

pub fn deserialize_binary_data_section(
  root_dir_url: &Url,
  data: &'static [u8],
) -> Result<DeserializedDataSection, AnyError> {
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

  let (input, found) = read_magic_bytes(data)?;
  if !found {
    bail!("Did not find magic bytes.");
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
  // 3. Specifiers
  let (input, specifiers_store) =
    SpecifierStore::deserialize(root_dir_url, input)
      .context("deserializing specifiers")?;
  // 4. Redirects
  let (input, redirects_store) =
    SpecifierDataStore::<SpecifierId>::deserialize(input)
      .context("deserializing redirects")?;
  // 5. Remote modules
  let (input, remote_modules_store) =
    SpecifierDataStore::<RemoteModuleEntry<'static>>::deserialize(input)
      .context("deserializing remote modules")?;
  // 6. VFS
  let (input, data) = read_bytes_with_u64_len(input).context("vfs")?;
  let vfs_root_entries: VirtualDirectoryEntries =
    serde_json::from_slice(data).context("deserializing vfs data")?;
  let (input, vfs_files_data) =
    read_bytes_with_u64_len(input).context("reading vfs files data")?;

  // finally ensure we read the magic bytes at the end
  let (_input, found) = read_magic_bytes(input)?;
  if !found {
    bail!("Could not find magic bytes at the end of the data.");
  }

  let modules_store = RemoteModulesStore::new(
    specifiers_store,
    redirects_store,
    remote_modules_store,
  );

  Ok(DeserializedDataSection {
    metadata,
    npm_snapshot,
    modules_store,
    vfs_root_entries,
    vfs_files_data,
  })
}

struct SpecifierStore {
  data: IndexMap<Arc<Url>, SpecifierId>,
  reverse: IndexMap<SpecifierId, Arc<Url>>,
}

impl SpecifierStore {
  pub fn deserialize<'a>(
    root_dir_url: &Url,
    input: &'a [u8],
  ) -> std::io::Result<(&'a [u8], Self)> {
    let (input, len) = read_u32_as_usize(input)?;
    let mut data = IndexMap::with_capacity(len);
    let mut reverse = IndexMap::with_capacity(len);
    let mut input = input;
    for _ in 0..len {
      let (new_input, specifier_str) = read_string_lossy(input)?;
      let specifier = match Url::parse(&specifier_str) {
        Ok(url) => url,
        Err(err) => match root_dir_url.join(&specifier_str) {
          Ok(url) => url,
          Err(_) => {
            return Err(std::io::Error::new(
              std::io::ErrorKind::InvalidData,
              err,
            ));
          }
        },
      };
      let (new_input, id) = SpecifierId::deserialize(new_input)?;
      let specifier = Arc::new(specifier);
      data.insert(specifier.clone(), id);
      reverse.insert(id, specifier);
      input = new_input;
    }
    Ok((input, Self { data, reverse }))
  }

  pub fn get_id(&self, specifier: &Url) -> Option<SpecifierId> {
    self.data.get(specifier).cloned()
  }

  pub fn get_specifier(&self, specifier_id: SpecifierId) -> Option<&Url> {
    self.reverse.get(&specifier_id).map(|url| url.as_ref())
  }
}

pub struct StandaloneModules {
  modules: RemoteModulesStore,
  vfs: Arc<FileBackedVfs>,
}

impl StandaloneModules {
  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a Url,
  ) -> Result<Option<&'a Url>, TooManyRedirectsError> {
    if specifier.scheme() == "file" {
      Ok(Some(specifier))
    } else {
      self.modules.resolve_specifier(specifier)
    }
  }

  pub fn has_file(&self, path: &Path) -> bool {
    self.vfs.file_entry(path).is_ok()
  }

  pub fn read<'a>(
    &'a self,
    specifier: &'a Url,
  ) -> Result<Option<DenoCompileModuleData<'a>>, JsErrorBox> {
    if specifier.scheme() == "file" {
      let path = deno_path_util::url_to_file_path(specifier)
        .map_err(JsErrorBox::from_err)?;
      let mut transpiled = None;
      let mut source_map = None;
      let mut cjs_export_analysis = None;
      let bytes = match self.vfs.file_entry(&path) {
        Ok(entry) => {
          let bytes = self
            .vfs
            .read_file_all(entry)
            .map_err(JsErrorBox::from_err)?;
          transpiled = entry
            .transpiled_offset
            .and_then(|t| self.vfs.read_file_offset_with_len(t).ok());
          source_map = entry
            .source_map_offset
            .and_then(|t| self.vfs.read_file_offset_with_len(t).ok());
          cjs_export_analysis = entry
            .cjs_export_analysis_offset
            .and_then(|t| self.vfs.read_file_offset_with_len(t).ok());
          bytes
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {
          match RealFs.read_file_sync(&path, None) {
            Ok(bytes) => bytes,
            Err(FsError::Io(err)) if err.kind() == ErrorKind::NotFound => {
              return Ok(None)
            }
            Err(err) => return Err(JsErrorBox::from_err(err)),
          }
        }
        Err(err) => return Err(JsErrorBox::from_err(err)),
      };
      Ok(Some(DenoCompileModuleData {
        media_type: MediaType::from_specifier(specifier),
        specifier,
        data: bytes,
        transpiled,
        source_map,
        cjs_export_analysis,
      }))
    } else {
      self.modules.read(specifier).map_err(JsErrorBox::from_err)
    }
  }
}

pub struct DenoCompileModuleData<'a> {
  pub specifier: &'a Url,
  pub media_type: MediaType,
  pub data: Cow<'static, [u8]>,
  pub transpiled: Option<Cow<'static, [u8]>>,
  pub source_map: Option<Cow<'static, [u8]>>,
  pub cjs_export_analysis: Option<Cow<'static, [u8]>>,
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

    let data = self.transpiled.unwrap_or(self.data);
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
      | MediaType::Tsx => (ModuleType::JavaScript, into_string_unsafe(data)),
      MediaType::Json => (ModuleType::Json, into_string_unsafe(data)),
      MediaType::Wasm => {
        (ModuleType::Wasm, DenoCompileModuleSource::Bytes(data))
      }
      // just assume javascript if we made it here
      MediaType::Css
      | MediaType::Html
      | MediaType::SourceMap
      | MediaType::Sql
      | MediaType::Unknown => {
        (ModuleType::JavaScript, DenoCompileModuleSource::Bytes(data))
      }
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

#[derive(Debug, Error, JsError)]
#[class(generic)]
#[error("Too many redirects resolving: {0}")]
pub struct TooManyRedirectsError(Url);

pub struct RemoteModulesStore {
  specifiers: SpecifierStore,
  redirects: SpecifierDataStore<SpecifierId>,
  remote_modules: SpecifierDataStore<RemoteModuleEntry<'static>>,
}

impl RemoteModulesStore {
  fn new(
    specifiers: SpecifierStore,
    redirects: SpecifierDataStore<SpecifierId>,
    remote_modules: SpecifierDataStore<RemoteModuleEntry<'static>>,
  ) -> Self {
    Self {
      specifiers,
      redirects,
      remote_modules,
    }
  }

  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a Url,
  ) -> Result<Option<&'a Url>, TooManyRedirectsError> {
    let Some(mut current) = self.specifiers.get_id(specifier) else {
      return Ok(None);
    };
    let mut count = 0;
    loop {
      if count > 10 {
        return Err(TooManyRedirectsError(specifier.clone()));
      }
      match self.redirects.get(current) {
        Some(to) => {
          current = *to;
          count += 1;
        }
        None => {
          if count == 0 {
            return Ok(Some(specifier));
          } else {
            return Ok(self.specifiers.get_specifier(current));
          }
        }
      }
    }
  }

  pub fn read<'a>(
    &'a self,
    original_specifier: &'a Url,
  ) -> Result<Option<DenoCompileModuleData<'a>>, TooManyRedirectsError> {
    #[allow(clippy::ptr_arg)]
    fn handle_cow_ref(data: &Cow<'static, [u8]>) -> Cow<'static, [u8]> {
      match data {
        Cow::Borrowed(data) => Cow::Borrowed(data),
        Cow::Owned(data) => {
          // this variant should never happen because the data
          // should always be borrowed static in denort
          debug_assert!(false);
          Cow::Owned(data.clone())
        }
      }
    }

    let mut count = 0;
    let Some(mut specifier) = self.specifiers.get_id(original_specifier) else {
      return Ok(None);
    };
    loop {
      if count > 10 {
        return Err(TooManyRedirectsError(original_specifier.clone()));
      }
      match self.redirects.get(specifier) {
        Some(to) => {
          specifier = *to;
          count += 1;
        }
        None => {
          let Some(entry) = self.remote_modules.get(specifier) else {
            return Ok(None);
          };
          return Ok(Some(DenoCompileModuleData {
            specifier: if count == 0 {
              original_specifier
            } else {
              self.specifiers.get_specifier(specifier).unwrap()
            },
            media_type: entry.media_type,
            data: handle_cow_ref(&entry.data),
            transpiled: entry.maybe_transpiled.as_ref().map(handle_cow_ref),
            source_map: entry.maybe_source_map.as_ref().map(handle_cow_ref),
            cjs_export_analysis: entry
              .maybe_cjs_export_analysis
              .as_ref()
              .map(handle_cow_ref),
          }));
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
  #[allow(clippy::type_complexity)]
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
  #[allow(clippy::type_complexity)]
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
        optional_peer_dependencies: Default::default(),
        has_bin: false,
        has_scripts: false,
        is_deprecated: false,
        extra: Default::default(),
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

fn read_bytes_with_u64_len(input: &[u8]) -> std::io::Result<(&[u8], &[u8])> {
  let (input, len) = read_u64(input)?;
  let (input, data) = read_bytes(input, len as usize)?;
  Ok((input, data))
}

fn read_bytes_with_u32_len(input: &[u8]) -> std::io::Result<(&[u8], &[u8])> {
  let (input, len) = read_u32_as_usize(input)?;
  let (input, data) = read_bytes(input, len)?;
  Ok((input, data))
}

fn read_bytes(input: &[u8], len: usize) -> std::io::Result<(&[u8], &[u8])> {
  check_has_len(input, len)?;
  let (len_bytes, input) = input.split_at(len);
  Ok((input, len_bytes))
}

#[inline(always)]
fn check_has_len(input: &[u8], len: usize) -> std::io::Result<()> {
  if input.len() < len {
    Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      "Unexpected end of data",
    ))
  } else {
    Ok(())
  }
}

fn read_string_lossy(input: &[u8]) -> std::io::Result<(&[u8], Cow<str>)> {
  let (input, data_bytes) = read_bytes_with_u32_len(input)?;
  Ok((input, String::from_utf8_lossy(data_bytes)))
}

fn read_u32_as_usize(input: &[u8]) -> std::io::Result<(&[u8], usize)> {
  let (input, len_bytes) = read_bytes(input, 4)?;
  let len = u32::from_le_bytes(len_bytes.try_into().unwrap());
  Ok((input, len as usize))
}

fn read_u64(input: &[u8]) -> std::io::Result<(&[u8], u64)> {
  let (input, len_bytes) = read_bytes(input, 8)?;
  let len = u64::from_le_bytes(len_bytes.try_into().unwrap());
  Ok((input, len))
}
