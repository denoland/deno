// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::collections::BTreeMap;

use deno_media_type::MediaType;
use deno_resolver::workspace::PackageJsonDepResolution;
use deno_runtime::deno_permissions::PermissionsOptions;
use deno_runtime::deno_telemetry::OtelConfig;
use deno_semver::Version;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use url::Url;

use super::virtual_fs::FileSystemCaseSensitivity;
use crate::args::UnstableConfig;

pub const MAGIC_BYTES: &[u8; 8] = b"d3n0l4nd";

pub trait DenoRtDeserializable<'a>: Sized {
  fn deserialize(input: &'a [u8]) -> std::io::Result<(&'a [u8], Self)>;
}

impl<'a> DenoRtDeserializable<'a> for Cow<'a, [u8]> {
  fn deserialize(input: &'a [u8]) -> std::io::Result<(&'a [u8], Self)> {
    let (input, data) = read_bytes_with_u32_len(input)?;
    Ok((input, Cow::Borrowed(data)))
  }
}

pub trait DenoRtSerializable<'a> {
  fn serialize(
    &'a self,
    builder: &mut capacity_builder::BytesBuilder<'a, Vec<u8>>,
  );
}

#[derive(Deserialize, Serialize)]
pub enum NodeModules {
  Managed {
    /// Relative path for the node_modules directory in the vfs.
    node_modules_dir: Option<String>,
  },
  Byonm {
    root_node_modules_dir: Option<String>,
  },
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolverImportMap {
  pub specifier: String,
  pub json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializedResolverWorkspaceJsrPackage {
  pub relative_base: String,
  pub name: String,
  pub version: Option<Version>,
  pub exports: IndexMap<String, String>,
}

#[derive(Deserialize, Serialize)]
pub struct SerializedWorkspaceResolver {
  pub import_map: Option<SerializedWorkspaceResolverImportMap>,
  pub jsr_pkgs: Vec<SerializedResolverWorkspaceJsrPackage>,
  pub package_jsons: BTreeMap<String, serde_json::Value>,
  pub pkg_json_resolution: PackageJsonDepResolution,
}

// Note: Don't use hashmaps/hashsets. Ensure the serialization
// is deterministic.
#[derive(Deserialize, Serialize)]
pub struct Metadata {
  pub argv: Vec<String>,
  pub seed: Option<u64>,
  pub code_cache_key: Option<u64>,
  pub permissions: PermissionsOptions,
  pub location: Option<Url>,
  pub v8_flags: Vec<String>,
  pub log_level: Option<log::Level>,
  pub ca_stores: Option<Vec<String>>,
  pub ca_data: Option<Vec<u8>>,
  pub unsafely_ignore_certificate_errors: Option<Vec<String>>,
  pub env_vars_from_env_file: IndexMap<String, String>,
  pub workspace_resolver: SerializedWorkspaceResolver,
  pub entrypoint_key: String,
  pub node_modules: Option<NodeModules>,
  pub unstable_config: UnstableConfig,
  pub otel_config: OtelConfig,
  pub vfs_case_sensitivity: FileSystemCaseSensitivity,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct SpecifierId(u32);

impl SpecifierId {
  pub fn new(id: u32) -> Self {
    Self(id)
  }
}

impl<'a> capacity_builder::BytesAppendable<'a> for SpecifierId {
  fn append_to_builder<TBytes: capacity_builder::BytesType>(
    self,
    builder: &mut capacity_builder::BytesBuilder<'a, TBytes>,
  ) {
    builder.append_le(self.0);
  }
}

impl<'a> DenoRtSerializable<'a> for SpecifierId {
  fn serialize(
    &'a self,
    builder: &mut capacity_builder::BytesBuilder<'a, Vec<u8>>,
  ) {
    builder.append_le(self.0);
  }
}

impl<'a> DenoRtDeserializable<'a> for SpecifierId {
  fn deserialize(input: &'a [u8]) -> std::io::Result<(&'a [u8], Self)> {
    let (input, id) = read_u32(input)?;
    Ok((input, Self(id)))
  }
}

#[derive(Deserialize, Serialize)]
pub enum CjsExportAnalysisEntry {
  Esm,
  Cjs(Vec<String>),
  Error(String),
}

const HAS_TRANSPILED_FLAG: u8 = 1 << 0;
const HAS_SOURCE_MAP_FLAG: u8 = 1 << 1;
const HAS_CJS_EXPORT_ANALYSIS_FLAG: u8 = 1 << 2;

pub struct RemoteModuleEntry<'a> {
  pub media_type: MediaType,
  pub data: Cow<'a, [u8]>,
  pub maybe_transpiled: Option<Cow<'a, [u8]>>,
  pub maybe_source_map: Option<Cow<'a, [u8]>>,
  pub maybe_cjs_export_analysis: Option<Cow<'a, [u8]>>,
}

impl<'a> DenoRtSerializable<'a> for RemoteModuleEntry<'a> {
  fn serialize(
    &'a self,
    builder: &mut capacity_builder::BytesBuilder<'a, Vec<u8>>,
  ) {
    fn append_maybe_data<'a>(
      builder: &mut capacity_builder::BytesBuilder<'a, Vec<u8>>,
      maybe_data: Option<&'a [u8]>,
    ) {
      if let Some(data) = maybe_data {
        builder.append_le(data.len() as u32);
        builder.append(data);
      }
    }

    let mut has_data_flags = 0;
    if self.maybe_transpiled.is_some() {
      has_data_flags |= HAS_TRANSPILED_FLAG;
    }
    if self.maybe_source_map.is_some() {
      has_data_flags |= HAS_SOURCE_MAP_FLAG;
    }
    if self.maybe_cjs_export_analysis.is_some() {
      has_data_flags |= HAS_CJS_EXPORT_ANALYSIS_FLAG;
    }
    builder.append(serialize_media_type(self.media_type));
    builder.append_le(self.data.len() as u32);
    builder.append(self.data.as_ref());
    builder.append(has_data_flags);
    append_maybe_data(builder, self.maybe_transpiled.as_deref());
    append_maybe_data(builder, self.maybe_source_map.as_deref());
    append_maybe_data(builder, self.maybe_cjs_export_analysis.as_deref());
  }
}

impl<'a> DenoRtDeserializable<'a> for RemoteModuleEntry<'a> {
  fn deserialize(input: &'a [u8]) -> std::io::Result<(&'a [u8], Self)> {
    #[allow(clippy::type_complexity)]
    fn deserialize_data_if_has_flag(
      input: &[u8],
      has_data_flags: u8,
      flag: u8,
    ) -> std::io::Result<(&[u8], Option<Cow<[u8]>>)> {
      if has_data_flags & flag != 0 {
        let (input, bytes) = read_bytes_with_u32_len(input)?;
        Ok((input, Some(Cow::Borrowed(bytes))))
      } else {
        Ok((input, None))
      }
    }

    let (input, media_type) = MediaType::deserialize(input)?;
    let (input, data) = read_bytes_with_u32_len(input)?;
    let (input, has_data_flags) = read_u8(input)?;
    let (input, maybe_transpiled) =
      deserialize_data_if_has_flag(input, has_data_flags, HAS_TRANSPILED_FLAG)?;
    let (input, maybe_source_map) =
      deserialize_data_if_has_flag(input, has_data_flags, HAS_SOURCE_MAP_FLAG)?;
    let (input, maybe_cjs_export_analysis) = deserialize_data_if_has_flag(
      input,
      has_data_flags,
      HAS_CJS_EXPORT_ANALYSIS_FLAG,
    )?;
    Ok((
      input,
      Self {
        media_type,
        data: Cow::Borrowed(data),
        maybe_transpiled,
        maybe_source_map,
        maybe_cjs_export_analysis,
      },
    ))
  }
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
    MediaType::Html => 14,
    MediaType::SourceMap => 15,
    MediaType::Sql => 16,
    MediaType::Unknown => 17,
  }
}

impl<'a> DenoRtDeserializable<'a> for MediaType {
  fn deserialize(input: &'a [u8]) -> std::io::Result<(&'a [u8], Self)> {
    let (input, value) = read_u8(input)?;
    let value = match value {
      0 => MediaType::JavaScript,
      1 => MediaType::Jsx,
      2 => MediaType::Mjs,
      3 => MediaType::Cjs,
      4 => MediaType::TypeScript,
      5 => MediaType::Mts,
      6 => MediaType::Cts,
      7 => MediaType::Dts,
      8 => MediaType::Dmts,
      9 => MediaType::Dcts,
      10 => MediaType::Tsx,
      11 => MediaType::Json,
      12 => MediaType::Wasm,
      13 => MediaType::Css,
      14 => MediaType::Html,
      15 => MediaType::SourceMap,
      16 => MediaType::Sql,
      17 => MediaType::Unknown,
      value => {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          format!("Unknown media type value: {value}"),
        ))
      }
    };
    Ok((input, value))
  }
}

/// Data stored keyed by specifier.
pub struct SpecifierDataStore<TData> {
  data: IndexMap<SpecifierId, TData>,
}

impl<TData> Default for SpecifierDataStore<TData> {
  fn default() -> Self {
    Self {
      data: IndexMap::new(),
    }
  }
}

impl<TData> SpecifierDataStore<TData> {
  pub fn with_capacity(capacity: usize) -> Self {
    Self {
      data: IndexMap::with_capacity(capacity),
    }
  }

  pub fn iter(&self) -> impl Iterator<Item = (SpecifierId, &TData)> {
    self.data.iter().map(|(k, v)| (*k, v))
  }

  #[allow(clippy::len_without_is_empty)]
  pub fn len(&self) -> usize {
    self.data.len()
  }

  pub fn contains(&self, specifier: SpecifierId) -> bool {
    self.data.contains_key(&specifier)
  }

  pub fn add(&mut self, specifier: SpecifierId, value: TData) {
    self.data.insert(specifier, value);
  }

  pub fn get(&self, specifier: SpecifierId) -> Option<&TData> {
    self.data.get(&specifier)
  }
}

impl<'a, TData> SpecifierDataStore<TData>
where
  TData: DenoRtSerializable<'a> + 'a,
{
  pub fn serialize(
    &'a self,
    builder: &mut capacity_builder::BytesBuilder<'a, Vec<u8>>,
  ) {
    builder.append_le(self.len() as u32);
    for (specifier, value) in self.iter() {
      builder.append(specifier);
      value.serialize(builder);
    }
  }
}

impl<'a, TData> DenoRtDeserializable<'a> for SpecifierDataStore<TData>
where
  TData: DenoRtDeserializable<'a>,
{
  fn deserialize(input: &'a [u8]) -> std::io::Result<(&'a [u8], Self)> {
    let (input, len) = read_u32_as_usize(input)?;
    let mut data = IndexMap::with_capacity(len);
    let mut input = input;
    for _ in 0..len {
      let (new_input, specifier) = SpecifierId::deserialize(input)?;
      let (new_input, value) = TData::deserialize(new_input)?;
      data.insert(specifier, value);
      input = new_input;
    }
    Ok((input, Self { data }))
  }
}

fn read_bytes_with_u32_len(input: &[u8]) -> std::io::Result<(&[u8], &[u8])> {
  let (input, len) = read_u32_as_usize(input)?;
  let (input, data) = read_bytes(input, len)?;
  Ok((input, data))
}

fn read_u32_as_usize(input: &[u8]) -> std::io::Result<(&[u8], usize)> {
  read_u32(input).map(|(input, len)| (input, len as usize))
}

fn read_u32(input: &[u8]) -> std::io::Result<(&[u8], u32)> {
  let (input, len_bytes) = read_bytes(input, 4)?;
  let len = u32::from_le_bytes(len_bytes.try_into().unwrap());
  Ok((input, len))
}

fn read_u8(input: &[u8]) -> std::io::Result<(&[u8], u8)> {
  check_has_len(input, 1)?;
  Ok((&input[1..], input[0]))
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
