// Copyright 2018-2025 the Deno authors. MIT license.

use std::collections::BTreeMap;
use std::collections::HashMap;

use deno_ast::MediaType;
use deno_core::error::AnyError;
use deno_core::serde_json;
use deno_core::url::Url;
use deno_lib::standalone::binary::Metadata;
use deno_lib::standalone::binary::SourceMapStore;
use deno_lib::standalone::binary::MAGIC_BYTES;
use deno_lib::standalone::virtual_fs::BuiltVfs;
use deno_npm::resolution::SerializedNpmResolutionSnapshot;

/// Binary format:
/// * d3n0l4nd
/// * <metadata_len><metadata>
/// * <npm_snapshot_len><npm_snapshot>
/// * <remote_modules>
/// * <vfs_headers_len><vfs_headers>
/// * <vfs_file_data_len><vfs_file_data>
/// * <source_map_data>
/// * d3n0l4nd
pub fn serialize_binary_data_section(
  metadata: &Metadata,
  npm_snapshot: Option<SerializedNpmResolutionSnapshot>,
  remote_modules: &RemoteModulesStoreBuilder,
  source_map_store: &SourceMapStore,
  vfs: &BuiltVfs,
) -> Result<Vec<u8>, AnyError> {
  let metadata = serde_json::to_string(metadata)?;
  let npm_snapshot =
    npm_snapshot.map(serialize_npm_snapshot).unwrap_or_default();
  let serialized_vfs = serde_json::to_string(&vfs.entries)?;

  let bytes = capacity_builder::BytesBuilder::build(|builder| {
    builder.append(MAGIC_BYTES);
    // 1. Metadata
    {
      builder.append_le(metadata.len() as u64);
      builder.append(&metadata);
    }
    // 2. Npm snapshot
    {
      builder.append_le(npm_snapshot.len() as u64);
      builder.append(&npm_snapshot);
    }
    // 3. Remote modules
    {
      remote_modules.write(builder);
    }
    // 4. VFS
    {
      builder.append_le(serialized_vfs.len() as u64);
      builder.append(&serialized_vfs);
      let vfs_bytes_len = vfs.files.iter().map(|f| f.len() as u64).sum::<u64>();
      builder.append_le(vfs_bytes_len);
      for file in &vfs.files {
        builder.append(file);
      }
    }
    // 5. Source maps
    {
      builder.append_le(source_map_store.len() as u32);
      for (specifier, source_map) in source_map_store.iter() {
        builder.append_le(specifier.len() as u32);
        builder.append(specifier);
        builder.append_le(source_map.len() as u32);
        builder.append(source_map);
      }
    }

    // write the magic bytes at the end so we can use it
    // to make sure we've deserialized correctly
    builder.append(MAGIC_BYTES);
  })?;

  Ok(bytes)
}

#[derive(Default)]
pub struct RemoteModulesStoreBuilder {
  specifiers: Vec<(String, u64)>,
  data: Vec<(MediaType, Vec<u8>, Option<Vec<u8>>)>,
  data_byte_len: u64,
  redirects: Vec<(String, String)>,
  redirects_len: u64,
}

impl RemoteModulesStoreBuilder {
  pub fn add(
    &mut self,
    specifier: &Url,
    media_type: MediaType,
    data: Vec<u8>,
    maybe_transpiled: Option<Vec<u8>>,
  ) {
    log::debug!("Adding '{}' ({})", specifier, media_type);
    let specifier = specifier.to_string();
    self.specifiers.push((specifier, self.data_byte_len));
    let maybe_transpiled_len = match &maybe_transpiled {
      // data length (4 bytes), data
      Some(data) => 4 + data.len() as u64,
      None => 0,
    };
    // media type (1 byte), data length (4 bytes), data, has transpiled (1 byte), transpiled length
    self.data_byte_len += 1 + 4 + data.len() as u64 + 1 + maybe_transpiled_len;
    self.data.push((media_type, data, maybe_transpiled));
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

  fn write<'a, TBytes: capacity_builder::BytesType>(
    &'a self,
    builder: &mut capacity_builder::BytesBuilder<'a, TBytes>,
  ) {
    builder.append_le(self.specifiers.len() as u32);
    builder.append_le(self.redirects.len() as u32);
    for (specifier, offset) in &self.specifiers {
      builder.append_le(specifier.len() as u32);
      builder.append(specifier);
      builder.append_le(*offset);
    }
    for (from, to) in &self.redirects {
      builder.append_le(from.len() as u32);
      builder.append(from);
      builder.append_le(to.len() as u32);
      builder.append(to);
    }
    builder.append_le(
      self
        .data
        .iter()
        .map(|(_, data, maybe_transpiled)| {
          1 + 4
            + (data.len() as u64)
            + 1
            + match maybe_transpiled {
              Some(transpiled) => 4 + (transpiled.len() as u64),
              None => 0,
            }
        })
        .sum::<u64>(),
    );
    for (media_type, data, maybe_transpiled) in &self.data {
      builder.append(serialize_media_type(*media_type));
      builder.append_le(data.len() as u32);
      builder.append(data);
      if let Some(transpiled) = maybe_transpiled {
        builder.append(1);
        builder.append_le(transpiled.len() as u32);
        builder.append(transpiled);
      } else {
        builder.append(0);
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
