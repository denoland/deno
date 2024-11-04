// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::io::Write;

use deno_ast::MediaType;
use deno_core::anyhow::bail;
use deno_core::anyhow::Context;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_core::FastString;
use deno_core::ModuleSourceCode;
use deno_core::ModuleType;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;
use deno_npm::resolution::SerializedNpmResolutionSnapshotPackage;
use deno_npm::resolution::ValidSerializedNpmResolutionSnapshot;
use deno_npm::NpmPackageId;
use deno_semver::package::PackageReq;

use crate::standalone::virtual_fs::VirtualDirectory;

use super::binary::Metadata;
use super::virtual_fs::VfsBuilder;

const MAGIC_BYTES: &[u8; 8] = b"d3n0l4nd";

/// Binary format:
/// * d3n0l4nd
/// * <metadata_len><metadata>
/// * <npm_snapshot_len><npm_snapshot>
/// * <remote_modules_len><remote_modules>
/// * <vfs_headers_len><vfs_headers>
/// * <vfs_file_data_len><vfs_file_data>
/// * d3n0l4nd
pub fn serialize_binary_data_section(
  metadata: &Metadata,
  npm_snapshot: Option<SerializedNpmResolutionSnapshot>,
  remote_modules: &RemoteModulesStoreBuilder,
  vfs: VfsBuilder,
) -> Result<Vec<u8>, AnyError> {
  fn write_bytes_with_len(bytes: &mut Vec<u8>, data: &[u8]) {
    bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
    bytes.extend_from_slice(data);
  }

  let mut bytes = Vec::new();
  bytes.extend_from_slice(MAGIC_BYTES);

  // 1. Metadata
  {
    let metadata = serde_json::to_string(metadata)?;
    write_bytes_with_len(&mut bytes, metadata.as_bytes());
  }
  // 2. Npm snapshot
  {
    let npm_snapshot =
      npm_snapshot.map(serialize_npm_snapshot).unwrap_or_default();
    write_bytes_with_len(&mut bytes, &npm_snapshot);
  }
  // 3. Remote modules
  {
    let update_index = bytes.len();
    bytes.extend_from_slice(&(0_u64).to_le_bytes());
    let start_index = bytes.len();
    remote_modules.write(&mut bytes)?;
    let length = bytes.len() - start_index;
    let length_bytes = (length as u64).to_le_bytes();
    bytes[update_index..update_index + length_bytes.len()]
      .copy_from_slice(&length_bytes);
  }
  // 4. VFS
  {
    let (vfs, vfs_files) = vfs.into_dir_and_files();
    let vfs = serde_json::to_string(&vfs)?;
    write_bytes_with_len(&mut bytes, vfs.as_bytes());
    let vfs_bytes_len = vfs_files.iter().map(|f| f.len() as u64).sum::<u64>();
    bytes.extend_from_slice(&vfs_bytes_len.to_le_bytes());
    for file in &vfs_files {
      bytes.extend_from_slice(file);
    }
  }

  // write the magic bytes at the end so we can use it
  // to make sure we've deserialized correctly
  bytes.extend_from_slice(MAGIC_BYTES);

  Ok(bytes)
}

pub struct DeserializedDataSection {
  pub metadata: Metadata,
  pub npm_snapshot: Option<ValidSerializedNpmResolutionSnapshot>,
  pub remote_modules: RemoteModulesStore,
  pub vfs_dir: VirtualDirectory,
  pub vfs_files_data: &'static [u8],
}

pub fn deserialize_binary_data_section(
  data: &'static [u8],
) -> Result<Option<DeserializedDataSection>, AnyError> {
  fn read_bytes_with_len(input: &[u8]) -> Result<(&[u8], &[u8]), AnyError> {
    let (input, len) = read_u64(input)?;
    let (input, data) = read_bytes(input, len as usize)?;
    Ok((input, data))
  }

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
    return Ok(None);
  }

  // 1. Metadata
  let (input, data) = read_bytes_with_len(input).context("reading metadata")?;
  let metadata: Metadata =
    serde_json::from_slice(data).context("deserializing metadata")?;
  // 2. Npm snapshot
  let (input, data) =
    read_bytes_with_len(input).context("reading npm snapshot")?;
  let npm_snapshot = if data.is_empty() {
    None
  } else {
    Some(deserialize_npm_snapshot(data).context("deserializing npm snapshot")?)
  };
  // 3. Remote modules
  let (input, data) =
    read_bytes_with_len(input).context("reading remote modules data")?;
  let remote_modules =
    RemoteModulesStore::build(data).context("deserializing remote modules")?;
  // 4. VFS
  let (input, data) = read_bytes_with_len(input).context("vfs")?;
  let vfs_dir: VirtualDirectory =
    serde_json::from_slice(data).context("deserializing vfs data")?;
  let (input, vfs_files_data) =
    read_bytes_with_len(input).context("reading vfs files data")?;

  // finally ensure we read the magic bytes at the end
  let (_input, found) = read_magic_bytes(input)?;
  if !found {
    bail!("Could not find magic bytes at the end of the data.");
  }

  Ok(Some(DeserializedDataSection {
    metadata,
    npm_snapshot,
    remote_modules,
    vfs_dir,
    vfs_files_data,
  }))
}

#[derive(Default)]
pub struct RemoteModulesStoreBuilder {
  specifiers: Vec<(String, u64)>,
  data: Vec<(MediaType, Vec<u8>)>,
  data_byte_len: u64,
  redirects: Vec<(String, String)>,
  redirects_len: u64,
}

impl RemoteModulesStoreBuilder {
  pub fn add(&mut self, specifier: &Url, media_type: MediaType, data: Vec<u8>) {
    log::debug!("Adding '{}' ({})", specifier, media_type);
    let specifier = specifier.to_string();
    self.specifiers.push((specifier, self.data_byte_len));
    self.data_byte_len += 1 + 8 + data.len() as u64; // media type (1 byte), data length (8 bytes), data
    self.data.push((media_type, data));
  }

  pub fn add_redirects(&mut self, redirects: &BTreeMap<Url, Url>) {
    self.redirects.reserve(redirects.len());
    for (from, to) in redirects {
      log::debug!("Adding redirect '{}' -> '{}'", from, to);
      let from = from.to_string();
      let to = to.to_string();
      self.redirects_len += (4 + from.len() + 4 + to.len()) as u64;
      self.redirects.push((from, to));
    }
  }

  fn write(&self, writer: &mut dyn Write) -> Result<(), AnyError> {
    writer.write_all(&(self.specifiers.len() as u32).to_le_bytes())?;
    writer.write_all(&(self.redirects.len() as u32).to_le_bytes())?;
    for (specifier, offset) in &self.specifiers {
      writer.write_all(&(specifier.len() as u32).to_le_bytes())?;
      writer.write_all(specifier.as_bytes())?;
      writer.write_all(&offset.to_le_bytes())?;
    }
    for (from, to) in &self.redirects {
      writer.write_all(&(from.len() as u32).to_le_bytes())?;
      writer.write_all(from.as_bytes())?;
      writer.write_all(&(to.len() as u32).to_le_bytes())?;
      writer.write_all(to.as_bytes())?;
    }
    for (media_type, data) in &self.data {
      writer.write_all(&[serialize_media_type(*media_type)])?;
      writer.write_all(&(data.len() as u64).to_le_bytes())?;
      writer.write_all(data)?;
    }
    Ok(())
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

enum RemoteModulesStoreSpecifierValue {
  Data(usize),
  Redirect(Url),
}

pub struct RemoteModulesStore {
  specifiers: HashMap<Url, RemoteModulesStoreSpecifierValue>,
  files_data: &'static [u8],
}

impl RemoteModulesStore {
  fn build(data: &'static [u8]) -> Result<Self, AnyError> {
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

    let (files_data, specifiers) = read_headers(data)?;

    Ok(Self {
      specifiers,
      files_data,
    })
  }

  pub fn resolve_specifier<'a>(
    &'a self,
    specifier: &'a Url,
  ) -> Result<Option<&'a Url>, AnyError> {
    let mut count = 0;
    let mut current = specifier;
    loop {
      if count > 10 {
        bail!("Too many redirects resolving '{}'", specifier);
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
  ) -> Result<Option<DenoCompileModuleData<'a>>, AnyError> {
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
          let (input, len) = read_u64(input)?;
          let (_input, data) = read_bytes(input, len as usize)?;
          return Ok(Some(DenoCompileModuleData {
            specifier,
            media_type,
            data: Cow::Borrowed(data),
          }));
        }
        None => {
          return Ok(None);
        }
      }
    }
  }
}

fn serialize_npm_snapshot(
  mut snapshot: SerializedNpmResolutionSnapshot,
) -> Vec<u8> {
  fn append_string(bytes: &mut Vec<u8>, string: &str) {
    let len = string.len() as u32;
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(string.as_bytes());
  }

  snapshot.packages.sort_by(|a, b| a.id.cmp(&b.id)); // determinism
  let ids_to_stored_ids = snapshot
    .packages
    .iter()
    .enumerate()
    .map(|(i, pkg)| (&pkg.id, i as u32))
    .collect::<HashMap<_, _>>();

  let mut root_packages: Vec<_> = snapshot.root_packages.iter().collect();
  root_packages.sort();
  let mut bytes = Vec::new();

  bytes.extend_from_slice(&(snapshot.packages.len() as u32).to_le_bytes());
  for pkg in &snapshot.packages {
    append_string(&mut bytes, &pkg.id.as_serialized());
  }

  bytes.extend_from_slice(&(root_packages.len() as u32).to_le_bytes());
  for (req, id) in root_packages {
    append_string(&mut bytes, &req.to_string());
    let id = ids_to_stored_ids.get(&id).unwrap();
    bytes.extend_from_slice(&id.to_le_bytes());
  }

  for pkg in &snapshot.packages {
    let deps_len = pkg.dependencies.len() as u32;
    bytes.extend_from_slice(&deps_len.to_le_bytes());
    let mut deps: Vec<_> = pkg.dependencies.iter().collect();
    deps.sort();
    for (req, id) in deps {
      append_string(&mut bytes, req);
      let id = ids_to_stored_ids.get(&id).unwrap();
      bytes.extend_from_slice(&id.to_le_bytes());
    }
  }

  bytes
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
  ) -> impl Fn(&[u8]) -> Result<(&[u8], (String, NpmPackageId)), AnyError> + 'a
  {
    |input| {
      let (input, req) = read_string_lossy(input)?;
      let (input, id) = read_u32_as_usize(input)?;
      Ok((input, (req.into_owned(), id_to_npm_id(id)?)))
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

fn serialize_media_type(media_type: MediaType) -> u8 {
  match media_type {
    MediaType::JavaScript => 0,
    MediaType::Jsx => 1,
    MediaType::Mjs => 2,
    MediaType::Cjs => 3,
    MediaType::TypeScript => 4,
    MediaType::Mts => 5,
    MediaType::Cts => 6,
    MediaType::Dts => 7,
    MediaType::Dmts => 8,
    MediaType::Dcts => 9,
    MediaType::Tsx => 10,
    MediaType::Json => 11,
    MediaType::Wasm => 12,
    MediaType::Css => 13,
    MediaType::SourceMap => 14,
    MediaType::Unknown => 15,
  }
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

fn read_bytes(input: &[u8], len: usize) -> Result<(&[u8], &[u8]), AnyError> {
  if input.len() < len {
    bail!("Unexpected end of data.",);
  }
  let (len_bytes, input) = input.split_at(len);
  Ok((input, len_bytes))
}

fn read_string_lossy(input: &[u8]) -> Result<(&[u8], Cow<str>), AnyError> {
  let (input, str_len) = read_u32_as_usize(input)?;
  let (input, data_bytes) = read_bytes(input, str_len)?;
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
