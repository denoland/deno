// Copyright 2018-2026 the Deno authors. MIT license.

use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::Index;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;

use async_trait::async_trait;
use deno_semver::SmallStackString;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::VersionReq;
use deno_semver::npm::NpmVersionReqParseError;
use deno_semver::package::PackageName;
use deno_semver::package::PackageNv;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeMap;
use serde_json::Value;
use thiserror::Error;

use crate::fast_registry_json;
use crate::resolution::NewestDependencyDate;
use crate::resolution::NpmPackageVersionNotFound;

// npm registry docs: https://github.com/npm/registry/blob/master/docs/REGISTRY-API.md

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub struct NpmPackageInfo {
  pub name: PackageName,
  pub versions: NpmPackageVersionInfos,
  #[serde(default, rename = "dist-tags")]
  pub dist_tags: HashMap<String, Version>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  #[serde(deserialize_with = "deserializers::hashmap")]
  pub time: HashMap<Version, chrono::DateTime<chrono::Utc>>,
}

/// Custom `_deno.*` properties stored alongside a cached packument.
#[derive(Debug, Default, Clone)]
pub struct NpmPackageInfoCacheMetadata {
  /// The `_deno.etag` property.
  pub etag: Option<String>,
  /// Whether the `_deno.packumentFormat` property records that this cache
  /// entry was created from a full packument response. When set, an empty
  /// `time` map means the registry provides no publish dates rather than
  /// that the abbreviated install manifest omitted them.
  pub full_packument: bool,
}

impl NpmPackageInfo {
  /// Fill in each version's `exports` subpath keys from the raw packument JSON
  /// this info was parsed from.
  ///
  /// The keys are `skip_deserializing` in the normal parse because only the LSP
  /// (npm import-specifier completion) reads them and retaining them for every
  /// version of every package regressed `deno run` memory badly (see
  /// denoland/deno#35664). The LSP calls this to populate them for its own use;
  /// no other command pays for it.
  pub fn fill_export_keys(
    &mut self,
    packument_json: &[u8],
  ) -> Result<(), serde_json::Error> {
    #[derive(Deserialize)]
    struct Packument {
      #[serde(default)]
      versions: HashMap<Version, VersionExports>,
    }
    #[derive(Deserialize)]
    struct VersionExports {
      #[serde(default)]
      exports: Option<NpmPackageExportKeys>,
    }

    let packument: Packument = serde_json::from_slice(packument_json)?;
    for (version, version_exports) in packument.versions {
      let Some(keys) = version_exports.exports else {
        continue;
      };
      if let Some(version_info) = self.versions.get_mut(&version) {
        version_info.exports = Some(keys);
      }
    }
    Ok(())
  }

  pub fn from_packument_slice(bytes: &[u8]) -> Result<Self, String> {
    Self::from_packument_slice_with_etag(bytes).map(|(info, _)| info)
  }

  pub fn from_packument_bytes(bytes: Vec<u8>) -> Result<Self, String> {
    Self::from_packument_bytes_with_etag(bytes).map(|(info, _)| info)
  }

  pub fn from_packument_slice_with_etag(
    bytes: &[u8],
  ) -> Result<(Self, Option<String>), String> {
    let text = std::str::from_utf8(bytes).map_err(|err| err.to_string())?;
    Self::from_packument_source_with_cache_info(Arc::new(text.to_owned()))
      .map(|(info, cache_info)| (info, cache_info.etag))
  }

  pub fn from_packument_bytes_with_etag(
    bytes: Vec<u8>,
  ) -> Result<(Self, Option<String>), String> {
    Self::from_packument_bytes_with_cache_info(bytes)
      .map(|(info, cache_info)| (info, cache_info.etag))
  }

  pub fn from_packument_bytes_with_cache_info(
    bytes: Vec<u8>,
  ) -> Result<(Self, NpmPackageInfoCacheMetadata), String> {
    let text = String::from_utf8(bytes).map_err(|err| err.to_string())?;
    Self::from_packument_source_with_cache_info(Arc::new(text))
  }

  fn from_packument_source_with_cache_info(
    source: Arc<String>,
  ) -> Result<(Self, NpmPackageInfoCacheMetadata), String> {
    let text = source.as_str();
    let index = fast_registry_json::pluck_packument_index(text)
      .map_err(|err| format!("error indexing npm packument: {err:?}"))?;
    let deno_etag = index
      .deno_etag
      .map(unescape_json_string_content)
      .transpose()?;
    let name = PackageName::from_str(
      index
        .name
        .ok_or_else(|| "npm packument missing name".to_string())?,
    );

    let mut version_ranges = HashMap::with_capacity(index.versions.len());
    for (version_text, (start, end)) in
      index.versions.iter().zip(index.version_ranges.iter())
    {
      version_ranges.insert(
        Version::parse_from_npm(version_text).map_err(|err| {
          format!("invalid npm version '{version_text}': {err}")
        })?,
        *start as usize..*end as usize,
      );
    }

    let mut dist_tags = HashMap::with_capacity(index.dist_tags.len());
    for (tag, version_text) in index.dist_tags.iter() {
      dist_tags.insert(
        tag.to_string(),
        Version::parse_from_npm(version_text).map_err(|err| {
          format!(
            "invalid npm dist-tag '{tag}' version '{version_text}': {err}"
          )
        })?,
      );
    }

    let mut time = HashMap::with_capacity(index.time.len());
    for (version_text, date_text) in index.time.iter() {
      let Ok(version) = Version::parse_from_npm(version_text) else {
        continue;
      };
      let date = chrono::DateTime::parse_from_rfc3339(date_text)
        .map_err(|err| {
          format!(
            "invalid npm publish date '{date_text}' for '{version_text}': {err}"
          )
        })?
        .to_utc();
      time.insert(version, date);
    }

    let mut trust_evidence = HashMap::with_capacity(index.trust_evidence.len());
    for (version_text, evidence) in index.trust_evidence {
      trust_evidence.insert(
        Version::parse_from_npm(version_text).map_err(|err| {
          format!("invalid npm version '{version_text}': {err}")
        })?,
        match evidence {
          fast_registry_json::TrustEvidence::Provenance => {
            TrustEvidence::Provenance
          }
          fast_registry_json::TrustEvidence::TrustedPublisher => {
            TrustEvidence::TrustedPublisher
          }
          fast_registry_json::TrustEvidence::StagedPublish => {
            TrustEvidence::StagedPublish
          }
        },
      );
    }

    let full_packument = index.deno_packument_format == Some("full");

    Ok((
      Self {
        name,
        versions: NpmPackageVersionInfos::lazy(
          source,
          version_ranges,
          trust_evidence,
        ),
        dist_tags,
        time,
      },
      NpmPackageInfoCacheMetadata {
        etag: deno_etag,
        full_packument,
      },
    ))
  }

  pub fn lazy_packument_source_bytes(&self) -> Option<&[u8]> {
    self.versions.lazy_source_bytes()
  }

  pub fn version_info<'a>(
    &'a self,
    nv: &PackageNv,
    link_packages: &'a HashMap<PackageName, Vec<NpmPackageVersionInfo>>,
  ) -> Result<&'a NpmPackageVersionInfo, NpmPackageVersionNotFound> {
    if let Some(packages) = link_packages.get(&nv.name) {
      for pkg in packages {
        if pkg.version == nv.version {
          return Ok(pkg);
        }
      }
    }
    match self.versions.get(&nv.version) {
      Some(version_info) => Ok(version_info),
      None => Err(NpmPackageVersionNotFound(nv.clone())),
    }
  }
}

fn unescape_json_string_content(raw: &str) -> Result<String, String> {
  if !raw.as_bytes().contains(&b'\\') {
    return Ok(raw.to_string());
  }
  let mut text = String::with_capacity(raw.len() + 2);
  text.push('"');
  text.push_str(raw);
  text.push('"');
  serde_json::from_str(&text).map_err(|err| err.to_string())
}

#[derive(Debug, Default)]
pub struct NpmPackageVersionInfos {
  inner: NpmPackageVersionInfosInner,
}

#[derive(Debug)]
enum NpmPackageVersionInfosInner {
  Materialized(HashMap<Version, NpmPackageVersionInfo>),
  Lazy(NpmLazyPackageVersionInfos),
}

impl Default for NpmPackageVersionInfosInner {
  fn default() -> Self {
    Self::Materialized(HashMap::new())
  }
}

#[derive(Debug)]
struct NpmLazyPackageVersionInfos {
  source: Arc<String>,
  entries: HashMap<Version, NpmLazyPackageVersionInfo>,
  trust_evidence: HashMap<Version, TrustEvidence>,
}

#[derive(Debug)]
struct NpmLazyPackageVersionInfo {
  range: Range<usize>,
  value: OnceLock<Option<NpmPackageVersionInfo>>,
}

impl Clone for NpmPackageVersionInfos {
  fn clone(&self) -> Self {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.clone().into()
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => {
        let entries = lazy
          .entries
          .iter()
          .map(|(version, entry)| {
            let value = OnceLock::new();
            if let Some(info) = entry.value.get() {
              let _ = value.set(info.as_ref().cloned());
            }
            (
              version.clone(),
              NpmLazyPackageVersionInfo {
                range: entry.range.clone(),
                value,
              },
            )
          })
          .collect();
        Self {
          inner: NpmPackageVersionInfosInner::Lazy(
            NpmLazyPackageVersionInfos {
              source: lazy.source.clone(),
              entries,
              trust_evidence: lazy.trust_evidence.clone(),
            },
          ),
        }
      }
    }
  }
}

impl From<HashMap<Version, NpmPackageVersionInfo>> for NpmPackageVersionInfos {
  fn from(value: HashMap<Version, NpmPackageVersionInfo>) -> Self {
    Self {
      inner: NpmPackageVersionInfosInner::Materialized(value),
    }
  }
}

impl Serialize for NpmPackageVersionInfos {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(self.len()))?;
    for (version, info) in self.iter() {
      map.serialize_entry(version, info)?;
    }
    map.end()
  }
}

impl<'de> Deserialize<'de> for NpmPackageVersionInfos {
  fn deserialize<D: Deserializer<'de>>(
    deserializer: D,
  ) -> Result<Self, D::Error> {
    Ok(
      HashMap::<Version, NpmPackageVersionInfo>::deserialize(deserializer)?
        .into(),
    )
  }
}

impl NpmPackageVersionInfos {
  fn lazy(
    source: Arc<String>,
    version_ranges: HashMap<Version, Range<usize>>,
    trust_evidence: HashMap<Version, TrustEvidence>,
  ) -> Self {
    Self {
      inner: NpmPackageVersionInfosInner::Lazy(NpmLazyPackageVersionInfos {
        source,
        trust_evidence,
        entries: version_ranges
          .into_iter()
          .map(|(version, range)| {
            (
              version,
              NpmLazyPackageVersionInfo {
                range,
                value: OnceLock::new(),
              },
            )
          })
          .collect(),
      }),
    }
  }

  fn lazy_source_bytes(&self) -> Option<&[u8]> {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(_) => None,
      NpmPackageVersionInfosInner::Lazy(lazy) => Some(lazy.source.as_bytes()),
    }
  }

  pub fn get(&self, version: &Version) -> Option<&NpmPackageVersionInfo> {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.get(version)
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => lazy
        .entries
        .get(version)
        .and_then(|entry| lazy.parse_entry(entry)),
    }
  }

  pub fn get_mut(
    &mut self,
    version: &Version,
  ) -> Option<&mut NpmPackageVersionInfo> {
    self.ensure_materialized();
    match &mut self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.get_mut(version)
      }
      NpmPackageVersionInfosInner::Lazy(_) => unreachable!(),
    }
  }

  pub fn insert(
    &mut self,
    version: Version,
    info: NpmPackageVersionInfo,
  ) -> Option<NpmPackageVersionInfo> {
    self.ensure_materialized();
    match &mut self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.insert(version, info)
      }
      NpmPackageVersionInfosInner::Lazy(_) => unreachable!(),
    }
  }

  pub fn remove(&mut self, version: &Version) -> Option<NpmPackageVersionInfo> {
    self.ensure_materialized();
    match &mut self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.remove(version)
      }
      NpmPackageVersionInfosInner::Lazy(_) => unreachable!(),
    }
  }

  pub fn contains_key(&self, version: &Version) -> bool {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.contains_key(version)
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => {
        lazy.entries.contains_key(version)
      }
    }
  }

  pub fn is_empty(&self) -> bool {
    self.len() == 0
  }

  pub fn len(&self) -> usize {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => versions.len(),
      NpmPackageVersionInfosInner::Lazy(lazy) => lazy.entries.len(),
    }
  }

  pub fn keys(&self) -> Box<dyn Iterator<Item = &Version> + '_> {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        Box::new(versions.keys())
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => Box::new(lazy.entries.keys()),
    }
  }

  pub fn trust_evidence(
    &self,
  ) -> Box<dyn Iterator<Item = (&Version, &TrustEvidence)> + '_> {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(_) => {
        Box::new(std::iter::empty())
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => {
        Box::new(lazy.trust_evidence.iter())
      }
    }
  }

  pub fn values(
    &self,
  ) -> Box<dyn Iterator<Item = &NpmPackageVersionInfo> + '_> {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        Box::new(versions.values())
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => Box::new(
        lazy
          .entries
          .values()
          .filter_map(|entry| lazy.parse_entry(entry)),
      ),
    }
  }

  pub fn values_mut(
    &mut self,
  ) -> std::collections::hash_map::ValuesMut<'_, Version, NpmPackageVersionInfo>
  {
    self.ensure_materialized();
    match &mut self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        versions.values_mut()
      }
      NpmPackageVersionInfosInner::Lazy(_) => unreachable!(),
    }
  }

  pub fn iter(
    &self,
  ) -> Box<dyn Iterator<Item = (&Version, &NpmPackageVersionInfo)> + '_> {
    match &self.inner {
      NpmPackageVersionInfosInner::Materialized(versions) => {
        Box::new(versions.iter())
      }
      NpmPackageVersionInfosInner::Lazy(lazy) => {
        Box::new(lazy.entries.iter().filter_map(|(version, entry)| {
          lazy.parse_entry(entry).map(|info| (version, info))
        }))
      }
    }
  }

  fn ensure_materialized(&mut self) {
    let NpmPackageVersionInfosInner::Lazy(lazy) = &self.inner else {
      return;
    };
    let versions = lazy
      .entries
      .iter()
      .filter_map(|(version, entry)| {
        lazy
          .parse_entry(entry)
          .map(|info| (version.clone(), info.clone()))
      })
      .collect();
    self.inner = NpmPackageVersionInfosInner::Materialized(versions);
  }
}

impl NpmLazyPackageVersionInfos {
  fn parse_entry<'a>(
    &self,
    entry: &'a NpmLazyPackageVersionInfo,
  ) -> Option<&'a NpmPackageVersionInfo> {
    entry
      .value
      .get_or_init(|| {
        serde_json::from_str(&self.source[entry.range.clone()])
          .inspect_err(|err| {
            log::debug!("Failed deserializing lazy npm version info: {err}");
          })
          .ok()
      })
      .as_ref()
  }
}

impl Index<&Version> for NpmPackageVersionInfos {
  type Output = NpmPackageVersionInfo;

  fn index(&self, index: &Version) -> &Self::Output {
    self.get(index).unwrap()
  }
}

/// An iterator over all the package versions that takes into account the
/// linked packages and the newest dependency date.
pub struct NpmPackageVersionInfosIterator<'a> {
  iterator: Box<dyn Iterator<Item = &'a NpmPackageVersionInfo> + 'a>,
  info: &'a NpmPackageInfo,
  newest_dependency_date: Option<NewestDependencyDate>,
}

impl<'a> NpmPackageVersionInfosIterator<'a> {
  pub fn new(
    info: &'a NpmPackageInfo,
    link_packages: Option<&'a Vec<NpmPackageVersionInfo>>,
    newest_dependency_date: Option<NewestDependencyDate>,
  ) -> Self {
    let iterator: Box<dyn Iterator<Item = &'a NpmPackageVersionInfo> + 'a> =
      match link_packages {
        Some(link_version_infos) => Box::new(link_version_infos.iter().chain(
          info.versions.values().filter(move |v| {
            // assumes the user won't have a large amount of linked versions
            !link_version_infos.iter().any(|l| l.version == v.version)
          }),
        )),
        None => Box::new(info.versions.values()),
      };
    Self {
      iterator,
      newest_dependency_date,
      info,
    }
  }
}

impl<'a> Iterator for NpmPackageVersionInfosIterator<'a> {
  type Item = &'a NpmPackageVersionInfo;

  fn next(&mut self) -> Option<Self::Item> {
    self.iterator.by_ref().find(|&next| {
      self
        .newest_dependency_date
        .and_then(|newest_dependency_date| {
          // assume versions not in the time hashmap are really old
          self
            .info
            .time
            .get(&next.version)
            .map(|publish_date| newest_dependency_date.matches(*publish_date))
        })
        .unwrap_or(true)
    })
  }
}

#[derive(Debug, Clone, Error, deno_error::JsError)]
#[class(type)]
#[error(
  "Error in {parent_nv} parsing version requirement for dependency \"{key}\": \"{value}\""
)]
pub struct NpmDependencyEntryError {
  /// Name and version of the package that has this dependency.
  pub parent_nv: PackageNv,
  /// Bare specifier.
  pub key: String,
  /// Value of the dependency.
  pub value: String,
  #[source]
  pub source: NpmDependencyEntryErrorSource,
}

#[derive(Debug, Clone, Error)]
pub enum NpmDependencyEntryErrorSource {
  #[error(transparent)]
  NpmVersionReqParseError(#[from] NpmVersionReqParseError),
  #[error("Package specified a dependency outside of npm ({}). Deno does not install these for security reasons. The npm package should be improved to have all its dependencies on npm.

To work around this, you can use a package.json and install the dependencies via `npm install`.", .specifier)]
  RemoteDependency { specifier: String },
  /// A `file:` or `link:` dependency that references a local path.
  /// These are typically used during development and should be bundled
  /// in the published package tarball. We silently skip them.
  #[error("Unsupported local dependency: {specifier}")]
  LocalDependency { specifier: String },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum NpmDependencyEntryKind {
  Dep,
  Peer,
  OptionalPeer,
}

impl NpmDependencyEntryKind {
  pub fn is_optional_peer(&self) -> bool {
    matches!(self, NpmDependencyEntryKind::OptionalPeer)
  }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NpmDependencyEntry {
  pub kind: NpmDependencyEntryKind,
  pub bare_specifier: StackString,
  pub name: PackageName,
  pub version_req: VersionReq,
  /// When the dependency is also marked as a peer dependency,
  /// use this entry to resolve the dependency when it can't
  /// be resolved as a peer dependency.
  pub peer_dep_version_req: Option<VersionReq>,
}

impl PartialOrd for NpmDependencyEntry {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for NpmDependencyEntry {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    // sort the dependencies alphabetically by name then by version descending
    match self.name.cmp(&other.name) {
      // sort by newest to oldest
      Ordering::Equal => other
        .version_req
        .version_text()
        .cmp(self.version_req.version_text()),
      ordering => ordering,
    }
  }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct NpmPeerDependencyMeta {
  #[serde(default)]
  #[serde(deserialize_with = "deserializers::null_default")]
  pub optional: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum NpmPackageVersionBinEntry {
  String(String),
  Map(HashMap<String, String>),
}

/// Whether an `exports` key names an import subpath the LSP can complete
/// (`"."` or `"./..."`, excluding single-`*` glob patterns), as opposed to a
/// condition name (`"import"`, `"node"`, ...) or other entry.
fn is_export_completion_key(key: &str) -> bool {
  key == "."
    || (key.starts_with("./") && key.chars().filter(|c| *c == '*').count() != 1)
}

/// The export subpath keys from a package's `exports` field (e.g. `"."`,
/// `"./feature"`), retained solely for npm import-specifier completion in the
/// LSP.
///
/// This is never deserialized as part of the normal packument parse (the
/// `exports` field on [`NpmPackageVersionInfo`] is `skip_deserializing`).
/// Retaining even the keys for the `exports` of every version of every npm
/// package regressed `deno run` memory and leaked per `--watch` reload (see
/// denoland/deno#35664), and only the LSP reads them. The LSP fills them in on
/// demand via [`NpmPackageInfo::fill_export_keys`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NpmPackageExportKeys(pub Vec<String>);

impl NpmPackageExportKeys {
  /// Extract the completion-relevant subpath keys from a `package.json`
  /// `exports` object.
  pub fn from_exports_object(exports: &serde_json::Map<String, Value>) -> Self {
    NpmPackageExportKeys(
      exports
        .keys()
        .filter(|k| is_export_completion_key(k))
        .cloned()
        .collect(),
    )
  }

  pub fn as_slice(&self) -> &[String] {
    &self.0
  }
}

impl Serialize for NpmPackageExportKeys {
  fn serialize<S: serde::Serializer>(
    &self,
    serializer: S,
  ) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    // Serialize back into an object shape so a round-trip re-parses through
    // the deserializer below and yields the same keys.
    let mut map = serializer.serialize_map(Some(self.0.len()))?;
    for key in &self.0 {
      map.serialize_entry(key, &true)?;
    }
    map.end()
  }
}

impl<'de> Deserialize<'de> for NpmPackageExportKeys {
  fn deserialize<D: serde::Deserializer<'de>>(
    deserializer: D,
  ) -> Result<Self, D::Error> {
    struct ExportKeysVisitor;

    impl<'de> serde::de::Visitor<'de> for ExportKeysVisitor {
      type Value = NpmPackageExportKeys;

      fn expecting(
        &self,
        formatter: &mut std::fmt::Formatter,
      ) -> std::fmt::Result {
        formatter.write_str("a package.json `exports` value")
      }

      fn visit_str<E>(self, _v: &str) -> Result<Self::Value, E> {
        // `"exports": "./index.js"` — a bare root export.
        Ok(NpmPackageExportKeys(vec![".".to_string()]))
      }

      fn visit_map<M: serde::de::MapAccess<'de>>(
        self,
        mut map: M,
      ) -> Result<Self::Value, M::Error> {
        let mut keys = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
          // Skip the value entirely — we never need to materialize it.
          map.next_value::<serde::de::IgnoredAny>()?;
          if is_export_completion_key(&key) {
            keys.push(key);
          }
        }
        Ok(NpmPackageExportKeys(keys))
      }

      fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut seq: A,
      ) -> Result<Self::Value, A::Error> {
        // A fallback-target array carries no subpath keys.
        while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
        Ok(NpmPackageExportKeys::default())
      }

      fn visit_bool<E>(self, _v: bool) -> Result<Self::Value, E> {
        Ok(NpmPackageExportKeys::default())
      }
      fn visit_i64<E>(self, _v: i64) -> Result<Self::Value, E> {
        Ok(NpmPackageExportKeys::default())
      }
      fn visit_u64<E>(self, _v: u64) -> Result<Self::Value, E> {
        Ok(NpmPackageExportKeys::default())
      }
      fn visit_f64<E>(self, _v: f64) -> Result<Self::Value, E> {
        Ok(NpmPackageExportKeys::default())
      }
      fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(NpmPackageExportKeys::default())
      }
      fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(NpmPackageExportKeys::default())
      }
    }

    deserializer.deserialize_any(ExportKeysVisitor)
  }
}

/// Keep `npm_cache::slim_package_info_bytes` in sync when adding fields here.
/// Cached packuments persist only the resolution-relevant subset of version
/// fields. The cache round-trip test documents the intentional exceptions:
/// `scripts` collapses to `hasInstallScript`, and `exports` is filled only for
/// the LSP on demand.
#[derive(Debug, Default, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NpmPackageVersionInfo {
  pub version: Version,
  /// Skipped during the normal parse — only the LSP consumes these keys, and
  /// retaining them for every version of every package regressed `deno run`
  /// memory (see denoland/deno#35664). The LSP fills them in via
  /// [`NpmPackageInfo::fill_export_keys`].
  #[serde(
    default,
    skip_deserializing,
    skip_serializing_if = "Option::is_none"
  )]
  pub exports: Option<NpmPackageExportKeys>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub dist: Option<NpmPackageVersionDistInfo>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub bin: Option<NpmPackageVersionBinEntry>,
  // Bare specifier to version (ex. `"typescript": "^3.0.1") or possibly
  // package and version (ex. `"typescript-3.0.1": "npm:typescript@3.0.1"`).
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  #[serde(deserialize_with = "deserializers::hashmap")]
  pub dependencies: HashMap<StackString, StackString>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  #[serde(deserialize_with = "deserializers::vector")]
  pub bundle_dependencies: Vec<StackString>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  #[serde(deserialize_with = "deserializers::vector")]
  pub bundled_dependencies: Vec<StackString>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  #[serde(deserialize_with = "deserializers::hashmap")]
  pub optional_dependencies: HashMap<StackString, StackString>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  #[serde(deserialize_with = "deserializers::hashmap")]
  pub peer_dependencies: HashMap<StackString, StackString>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  #[serde(deserialize_with = "deserializers::hashmap")]
  pub peer_dependencies_meta: HashMap<StackString, NpmPeerDependencyMeta>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  #[serde(deserialize_with = "deserializers::vector")]
  pub os: Vec<SmallStackString>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  #[serde(deserialize_with = "deserializers::vector")]
  pub cpu: Vec<SmallStackString>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  #[serde(deserialize_with = "deserializers::hashmap")]
  pub scripts: HashMap<SmallStackString, String>,
  /// From the abbreviated install manifest format. When `true`, this version
  /// has preinstall/install/postinstall lifecycle scripts. This field is only
  /// present in abbreviated packuments where the full `scripts` map is not
  /// available.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub has_install_script: Option<bool>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[serde(deserialize_with = "deserializers::string")]
  pub deprecated: Option<String>,
  /// The `_npmUser` field from the full packument. Identifies who published
  /// the version and carries the `trustedPublisher` (OIDC trusted publishing)
  /// and `approver` (staged publish) trust signals. Only present in the full
  /// packument.
  #[serde(
    default,
    rename = "_npmUser",
    skip_serializing_if = "Option::is_none"
  )]
  pub npm_user: Option<NpmUser>,
}

/// A presence marker for a registry field whose mere existence is the signal
/// we care about (`_npmUser.approver`, `_npmUser.trustedPublisher`,
/// `dist.attestations.provenance`). The full packument carries large objects
/// here, but [`NpmPackageVersionInfo::get_trust_evidence`] only checks whether
/// they are present, so this discards the contents on deserialize and
/// re-serializes compactly as `true`. That keeps the cached packument small
/// even though `min-release-age` makes Deno fetch the full packument by
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Present;

impl Serialize for Present {
  fn serialize<S: serde::Serializer>(
    &self,
    serializer: S,
  ) -> Result<S::Ok, S::Error> {
    serializer.serialize_bool(true)
  }
}

impl<'de> Deserialize<'de> for Present {
  fn deserialize<D: serde::Deserializer<'de>>(
    deserializer: D,
  ) -> Result<Self, D::Error> {
    serde::de::IgnoredAny::deserialize(deserializer)?;
    Ok(Present)
  }
}

impl NpmPackageVersionInfo {
  /// The strongest publishing-trust evidence this version exposes, derived
  /// from registry metadata signals. Used by the `no-downgrade` trust policy.
  ///
  /// Mirrors pnpm's
  /// [`getTrustEvidence`](https://github.com/pnpm/pnpm/blob/main/resolving/npm-resolver/src/trustChecks.ts).
  /// The tiers are mutually exclusive: a staged publish (`_npmUser.approver`)
  /// is strongest, then trusted publishing (`_npmUser.trustedPublisher`)
  /// *backed by* a provenance attestation, then a provenance attestation on
  /// its own. A `trustedPublisher` flag without a provenance attestation is
  /// not counted: on its own it is just metadata a future staged-publish flow
  /// could mint, so it only counts as the stronger signal when the version
  /// also shipped provenance.
  pub fn get_trust_evidence(&self) -> Option<TrustEvidence> {
    let npm_user = self.npm_user.as_ref();
    if npm_user.is_some_and(|u| u.approver.is_some()) {
      return Some(TrustEvidence::StagedPublish);
    }
    let has_provenance = self
      .dist
      .as_ref()
      .and_then(|d| d.attestations.as_ref())
      .is_some_and(|a| a.provenance.is_some());
    let has_trusted_publisher =
      npm_user.is_some_and(|u| u.trusted_publisher.is_some());
    if has_trusted_publisher && has_provenance {
      return Some(TrustEvidence::TrustedPublisher);
    }
    if has_provenance {
      return Some(TrustEvidence::Provenance);
    }
    None
  }

  /// Helper for getting the bundle dependencies.
  ///
  /// Unfortunately due to limitations in serde, it's not
  /// easy to have a way to deserialize an alias without it
  /// throwing when the data has both fields, so we store both
  /// on the struct.
  pub fn bundle_dependencies(&self) -> &[StackString] {
    if self.bundle_dependencies.is_empty() {
      // only use the alias if the main field is empty
      &self.bundled_dependencies
    } else {
      &self.bundle_dependencies
    }
  }

  pub fn dependencies_as_entries(
    &self,
    // name of the package used to improve error messages
    package_name: &str,
  ) -> Result<Vec<NpmDependencyEntry>, Box<NpmDependencyEntryError>> {
    fn parse_dep_entry_inner(
      (key, value): (&StackString, &StackString),
      kind: NpmDependencyEntryKind,
    ) -> Result<NpmDependencyEntry, NpmDependencyEntryErrorSource> {
      let (name, version_req) =
        parse_dep_entry_name_and_raw_version(key, value)?;
      let version_req = VersionReq::parse_from_npm(version_req)?;
      Ok(NpmDependencyEntry {
        kind,
        bare_specifier: key.clone(),
        name: PackageName::from_str(name),
        version_req,
        peer_dep_version_req: None,
      })
    }

    fn parse_dep_entry(
      parent_nv: (&str, &Version),
      key_value: (&StackString, &StackString),
      kind: NpmDependencyEntryKind,
    ) -> Result<Option<NpmDependencyEntry>, Box<NpmDependencyEntryError>> {
      match parse_dep_entry_inner(key_value, kind) {
        Ok(entry) => Ok(Some(entry)),
        Err(NpmDependencyEntryErrorSource::LocalDependency { .. }) => Ok(None),
        Err(source) => Err(Box::new(NpmDependencyEntryError {
          parent_nv: PackageNv {
            name: parent_nv.0.into(),
            version: parent_nv.1.clone(),
          },
          key: key_value.0.to_string(),
          value: key_value.1.to_string(),
          source,
        })),
      }
    }

    let normalized_dependencies = if self
      .optional_dependencies
      .keys()
      .all(|k| self.dependencies.contains_key(k))
      && self.bundle_dependencies().is_empty()
    {
      Cow::Borrowed(&self.dependencies)
    } else {
      // Most package information has the optional dependencies duplicated
      // in the dependencies list, but some don't. In those cases, add
      // the optonal dependencies into the map of dependencies
      Cow::Owned(
        self
          .optional_dependencies
          .iter()
          // prefer what's in the dependencies map
          .chain(self.dependencies.iter())
          // exclude bundle dependencies
          .filter(|(k, _)| !self.bundle_dependencies().iter().any(|b| b == *k))
          .map(|(k, v)| (k.clone(), v.clone()))
          .collect(),
      )
    };

    let mut result = HashMap::with_capacity(
      normalized_dependencies.len() + self.peer_dependencies.len(),
    );
    let nv = (package_name, &self.version);
    for entry in &self.peer_dependencies {
      let is_optional = self
        .peer_dependencies_meta
        .get(entry.0)
        .map(|d| d.optional)
        .unwrap_or(false);
      let kind = match is_optional {
        true => NpmDependencyEntryKind::OptionalPeer,
        false => NpmDependencyEntryKind::Peer,
      };
      if let Some(entry) = parse_dep_entry(nv, entry, kind)? {
        result.insert(entry.bare_specifier.clone(), entry);
      }
    }
    for entry in normalized_dependencies.iter() {
      let entry = parse_dep_entry(nv, entry, NpmDependencyEntryKind::Dep)?;
      if let Some(entry) = entry {
        // people may define a dependency as a peer dependency as well,
        // so in those cases, attempt to resolve as a peer dependency,
        // but then use this dependency version requirement otherwise
        if let Some(peer_dep_entry) = result.get_mut(&entry.bare_specifier) {
          peer_dep_entry.peer_dep_version_req = Some(entry.version_req);
        } else {
          result.insert(entry.bare_specifier.clone(), entry);
        }
      }
    }
    Ok(result.into_values().collect())
  }
}

/// Gets the name and raw version constraint for a registry info or
/// package.json dependency entry taking into account npm package aliases.
fn parse_dep_entry_name_and_raw_version<'a>(
  key: &'a str,
  value: &'a str,
) -> Result<(&'a str, &'a str), NpmDependencyEntryErrorSource> {
  let (name, version_req) =
    if let Some(package_and_version) = value.strip_prefix("npm:") {
      if let Some((name, version)) = package_and_version.rsplit_once('@') {
        // if empty, then the name was scoped and there's no version
        if name.is_empty() {
          (package_and_version, "*")
        } else {
          (name, version)
        }
      } else {
        (package_and_version, "*")
      }
    } else {
      (key, value)
    };
  if version_req.starts_with("https://")
    || version_req.starts_with("http://")
    || version_req.starts_with("git:")
    || version_req.starts_with("github:")
    || version_req.starts_with("git+")
  {
    Err(NpmDependencyEntryErrorSource::RemoteDependency {
      specifier: version_req.to_string(),
    })
  } else if version_req.starts_with("file:") || version_req.starts_with("link:")
  {
    Err(NpmDependencyEntryErrorSource::LocalDependency {
      specifier: version_req.to_string(),
    })
  } else {
    Ok((name, version_req))
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpmPackageVersionDistInfoIntegrity<'a> {
  /// A string in the form `sha1-<hash>` where the hash is base64 encoded.
  Integrity {
    algorithm: &'a str,
    base64_hash: &'a str,
  },
  /// The integrity could not be determined because it did not contain a dash.
  UnknownIntegrity(&'a str),
  /// The legacy sha1 hex hash (ex. "62afbee2ffab5e0db139450767a6125cbea50fa2").
  LegacySha1Hex(&'a str),
  /// No integrity was found.
  None,
}

impl NpmPackageVersionDistInfoIntegrity<'_> {
  pub fn for_lockfile(&self) -> Option<Cow<'_, str>> {
    match self {
      NpmPackageVersionDistInfoIntegrity::Integrity {
        algorithm,
        base64_hash,
      } => Some(Cow::Owned(format!("{}-{}", algorithm, base64_hash))),
      NpmPackageVersionDistInfoIntegrity::UnknownIntegrity(integrity) => {
        Some(Cow::Borrowed(integrity))
      }
      NpmPackageVersionDistInfoIntegrity::LegacySha1Hex(hex) => {
        Some(Cow::Borrowed(hex))
      }
      NpmPackageVersionDistInfoIntegrity::None => None,
    }
  }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmPackageVersionDistInfo {
  /// URL to the tarball.
  pub tarball: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub(crate) shasum: Option<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub(crate) integrity: Option<String>,
  /// Cryptographic attestations for this version (provenance, publish).
  /// Only present in the full packument.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub attestations: Option<NpmAttestations>,
}

/// The `_npmUser` object from the full packument. Only the presence of
/// `trustedPublisher` and `approver` are trust signals; the `name` and other
/// fields are dropped on deserialize to keep the cached packument small.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmUser {
  /// Present when the version was published via npm trusted publishing
  /// (OIDC). Its contents identify the CI provider and workflow.
  #[serde(
    default,
    rename = "trustedPublisher",
    skip_serializing_if = "Option::is_none"
  )]
  pub trusted_publisher: Option<Present>,
  /// Present when the version went through npm staged publishing: a maintainer
  /// approved it with a live 2FA challenge before it became installable. This
  /// is the strongest publishing-trust signal.
  /// See https://docs.npmjs.com/staged-publishing/
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub approver: Option<Present>,
}

/// The `dist.attestations` object from the full packument.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NpmAttestations {
  /// SLSA provenance attestation linking the package to the source commit and
  /// build. Present when the version was published with `--provenance`.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub provenance: Option<Present>,
}

/// The strongest publishing-trust evidence a package version exposes, derived
/// from registry metadata. Variants are declared weakest-first so the derived
/// `Ord` matches the trust rank. The `no-downgrade` trust policy refuses to
/// resolve a version whose evidence is weaker than the strongest evidence on
/// any earlier-published version of the same package.
///
/// "No evidence" is represented as `Option::None` rather than a variant here,
/// matching pnpm's `undefined`; compare ranks via
/// `Option::map_or(0, TrustEvidence::rank)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TrustEvidence {
  /// `dist.attestations.provenance` is set (published with `--provenance`).
  Provenance,
  /// `_npmUser.trustedPublisher` is set alongside a provenance attestation:
  /// published via OIDC-backed trusted publishing.
  TrustedPublisher,
  /// `_npmUser.approver` is set: a staged publish requiring a 2FA approval.
  /// The strongest signal.
  StagedPublish,
}

impl TrustEvidence {
  /// The numeric trust rank, mirroring pnpm's `TRUST_RANK` weights. Higher is
  /// more trusted; "no evidence" is rank `0`.
  pub fn rank(self) -> u8 {
    match self {
      TrustEvidence::Provenance => 1,
      TrustEvidence::TrustedPublisher => 2,
      TrustEvidence::StagedPublish => 3,
    }
  }

  /// Human-readable description for diagnostics.
  pub fn pretty(self) -> &'static str {
    match self {
      TrustEvidence::Provenance => "provenance attestation",
      TrustEvidence::TrustedPublisher => "trusted publisher",
      TrustEvidence::StagedPublish => "staged publish",
    }
  }
}

impl NpmPackageVersionDistInfo {
  pub fn integrity(&self) -> NpmPackageVersionDistInfoIntegrity<'_> {
    match &self.integrity {
      Some(integrity) => match integrity.split_once('-') {
        Some((algorithm, base64_hash)) => {
          NpmPackageVersionDistInfoIntegrity::Integrity {
            algorithm,
            base64_hash,
          }
        }
        None => NpmPackageVersionDistInfoIntegrity::UnknownIntegrity(
          integrity.as_str(),
        ),
      },
      None => match &self.shasum {
        Some(shasum) => {
          NpmPackageVersionDistInfoIntegrity::LegacySha1Hex(shasum)
        }
        None => NpmPackageVersionDistInfoIntegrity::None,
      },
    }
  }
}

/// Error that occurs when loading the package info from the npm registry fails.
#[derive(Debug, Error, Clone, deno_error::JsError)]
pub enum NpmRegistryPackageInfoLoadError {
  #[class(type)]
  #[error("npm package '{package_name}' does not exist.")]
  PackageNotExists { package_name: String },
  #[class(inherit)]
  #[error(transparent)]
  LoadError(Arc<dyn deno_error::JsErrorClass>),
}

/// A trait for getting package information from the npm registry.
///
/// An implementer may want to override the default implementation of
/// [`mark_force_reload`] method if it has a cache mechanism.
#[async_trait(?Send)]
pub trait NpmRegistryApi {
  /// Gets the package information from the npm registry.
  ///
  /// Note: The implementer should handle requests for the same npm
  /// package name concurrently and try not to make the same request
  /// to npm at the same time.
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError>;

  /// Marks that new requests for package information should retrieve it
  /// from the npm registry
  ///
  /// Returns true if both of the following conditions are met:
  /// - the implementer has a cache mechanism
  /// - "force reload" flag is successfully set for the first time
  fn mark_force_reload(&self) -> bool {
    false
  }
}

/// A simple in-memory implementation of the NpmRegistryApi
/// that can be used for testing purposes. This does not use
/// `#[cfg(test)]` because that is not supported across crates.
///
/// Note: This test struct is not thread safe for setup
/// purposes. Construct everything on the same thread.
#[derive(Clone, Default, Debug)]
pub struct TestNpmRegistryApi {
  package_infos: Rc<RefCell<HashMap<String, Arc<NpmPackageInfo>>>>,
}

#[async_trait::async_trait(?Send)]
impl deno_lockfile::NpmPackageInfoProvider for TestNpmRegistryApi {
  async fn get_npm_package_info(
    &self,
    values: &[PackageNv],
  ) -> Result<
    Vec<deno_lockfile::Lockfile5NpmInfo>,
    Box<dyn std::error::Error + Send + Sync>,
  > {
    let mut infos = Vec::new();
    let linked_packages = HashMap::new();
    for nv in values {
      let info = self
        .package_info(nv.name.as_str())
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
      let version_info = info.version_info(nv, &linked_packages).unwrap();
      let lockfile_info = deno_lockfile::Lockfile5NpmInfo {
        tarball_url: version_info
          .dist
          .as_ref()
          .map(|dist| dist.tarball.clone()),
        optional_dependencies: Default::default(),
        cpu: version_info.cpu.iter().map(|s| s.to_string()).collect(),
        os: version_info.os.iter().map(|s| s.to_string()).collect(),
        deprecated: version_info.deprecated.is_some(),
        bin: version_info.bin.is_some(),
        scripts: version_info.has_install_script.unwrap_or(false)
          || version_info.scripts.contains_key("preinstall")
          || version_info.scripts.contains_key("install")
          || version_info.scripts.contains_key("postinstall"),
        optional_peers: version_info
          .peer_dependencies
          .iter()
          .filter_map(|(k, v)| {
            if version_info
              .peer_dependencies_meta
              .get(k)
              .is_some_and(|m| m.optional)
            {
              Some((k.to_string(), v.to_string()))
            } else {
              None
            }
          })
          .collect(),
      };
      infos.push(lockfile_info);
    }
    Ok(infos)
  }
}

impl TestNpmRegistryApi {
  pub fn add_package_info(&self, name: &str, info: NpmPackageInfo) {
    let previous = self
      .package_infos
      .borrow_mut()
      .insert(name.to_string(), Arc::new(info));
    assert!(previous.is_none());
  }

  pub fn ensure_package(&self, name: &str) {
    if !self.package_infos.borrow().contains_key(name) {
      self.add_package_info(
        name,
        NpmPackageInfo {
          name: name.into(),
          ..Default::default()
        },
      );
    }
  }

  pub fn with_package(&self, name: &str, f: impl FnOnce(&mut NpmPackageInfo)) {
    self.ensure_package(name);
    let mut infos = self.package_infos.borrow_mut();
    let mut info = infos.get_mut(name).unwrap().as_ref().clone();
    f(&mut info);
    infos.insert(name.to_string(), Arc::new(info));
  }

  pub fn add_dist_tag(&self, package_name: &str, tag: &str, version: &str) {
    self.with_package(package_name, |package| {
      package
        .dist_tags
        .insert(tag.to_string(), Version::parse_from_npm(version).unwrap());
    })
  }

  pub fn ensure_package_version(&self, name: &str, version: &str) {
    self.ensure_package_version_with_integrity(name, version, None)
  }

  pub fn ensure_package_version_with_integrity(
    &self,
    name: &str,
    version: &str,
    integrity: Option<&str>,
  ) {
    self.ensure_package(name);
    self.with_package(name, |info| {
      let version = Version::parse_from_npm(version).unwrap();
      if !info.versions.contains_key(&version) {
        info.versions.insert(
          version.clone(),
          NpmPackageVersionInfo {
            version,
            dist: Some(NpmPackageVersionDistInfo {
              integrity: integrity.map(|s| s.to_string()),
              ..Default::default()
            }),
            ..Default::default()
          },
        );
      }
    })
  }

  pub fn with_version_info(
    &self,
    package: (&str, &str),
    f: impl FnOnce(&mut NpmPackageVersionInfo),
  ) {
    let (name, version) = package;
    self.ensure_package_version(name, version);
    self.with_package(name, |info| {
      let version = Version::parse_from_npm(version).unwrap();
      let version_info = info.versions.get_mut(&version).unwrap();
      f(version_info);
    });
  }

  pub fn add_dependency(&self, package: (&str, &str), entry: (&str, &str)) {
    self.with_version_info(package, |version| {
      version.dependencies.insert(entry.0.into(), entry.1.into());
    })
  }

  pub fn add_bundle_dependency(
    &self,
    package: (&str, &str),
    entry: (&str, &str),
  ) {
    self.with_version_info(package, |version| {
      version.dependencies.insert(entry.0.into(), entry.1.into());
      version.bundle_dependencies.push(entry.0.into());
    })
  }

  pub fn add_dep_and_optional_dep(
    &self,
    package: (&str, &str),
    entry: (&str, &str),
  ) {
    self.with_version_info(package, |version| {
      version.dependencies.insert(entry.0.into(), entry.1.into());
      version
        .optional_dependencies
        .insert(entry.0.into(), entry.1.into());
    })
  }

  pub fn add_optional_dep(&self, package: (&str, &str), entry: (&str, &str)) {
    self.with_version_info(package, |version| {
      version
        .optional_dependencies
        .insert(entry.0.into(), entry.1.into());
    })
  }

  pub fn add_peer_dependency(
    &self,
    package: (&str, &str),
    entry: (&str, &str),
  ) {
    self.with_version_info(package, |version| {
      version
        .peer_dependencies
        .insert(entry.0.into(), entry.1.into());
    });
  }

  pub fn add_optional_peer_dependency(
    &self,
    package: (&str, &str),
    entry: (&str, &str),
  ) {
    self.with_version_info(package, |version| {
      version
        .peer_dependencies
        .insert(entry.0.into(), entry.1.into());
      version
        .peer_dependencies_meta
        .insert(entry.0.into(), NpmPeerDependencyMeta { optional: true });
    });
  }
}

#[async_trait(?Send)]
impl NpmRegistryApi for TestNpmRegistryApi {
  async fn package_info(
    &self,
    name: &str,
  ) -> Result<Arc<NpmPackageInfo>, NpmRegistryPackageInfoLoadError> {
    let infos = self.package_infos.borrow();
    Ok(infos.get(name).cloned().ok_or_else(|| {
      NpmRegistryPackageInfoLoadError::PackageNotExists {
        package_name: name.into(),
      }
    })?)
  }
}

mod deserializers {
  use std::collections::HashMap;
  use std::fmt;

  use serde::Deserialize;
  use serde::Deserializer;
  use serde::de;
  use serde::de::DeserializeOwned;
  use serde::de::MapAccess;
  use serde::de::SeqAccess;
  use serde::de::Visitor;

  /// Deserializes empty or null values to the default value (npm allows uploading
  /// `null` for values and serde doesn't automatically make that the default).
  ///
  /// Code from: https://github.com/serde-rs/serde/issues/1098#issuecomment-760711617
  pub fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
  where
    T: Default + Deserialize<'de>,
    D: serde::Deserializer<'de>,
  {
    let opt = Option::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
  }

  pub fn hashmap<'de, K, V, D>(
    deserializer: D,
  ) -> Result<HashMap<K, V>, D::Error>
  where
    K: DeserializeOwned + Eq + std::hash::Hash,
    V: DeserializeOwned,
    D: Deserializer<'de>,
  {
    deserializer.deserialize_option(HashMapVisitor::<K, V> {
      marker: std::marker::PhantomData,
    })
  }

  pub fn string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_option(OptionalStringVisitor)
  }

  pub fn vector<'de, T, D>(deserializer: D) -> Result<Vec<T>, D::Error>
  where
    T: DeserializeOwned,
    D: Deserializer<'de>,
  {
    deserializer.deserialize_option(VectorVisitor::<T> {
      marker: std::marker::PhantomData,
    })
  }

  struct HashMapVisitor<K, V> {
    marker: std::marker::PhantomData<fn() -> HashMap<K, V>>,
  }

  impl<'de, K, V> Visitor<'de> for HashMapVisitor<K, V>
  where
    K: DeserializeOwned + Eq + std::hash::Hash,
    V: DeserializeOwned,
  {
    type Value = HashMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a map")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
      D: Deserializer<'de>,
    {
      deserializer.deserialize_any(self)
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
      M: MapAccess<'de>,
    {
      let mut hashmap = HashMap::new();

      // deserialize to a serde_json::Value first to ensure serde_json
      // skips over the entry, then deserialize to an actual value
      while let Some(entry) =
        map.next_entry::<serde_json::Value, serde_json::Value>()?
      {
        if let Ok(key) = serde_json::from_value(entry.0)
          && let Ok(value) = serde_json::from_value(entry.1)
        {
          hashmap.insert(key, value);
        }
      }

      Ok(hashmap)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
      A: SeqAccess<'de>,
    {
      while seq.next_element::<de::IgnoredAny>()?.is_some() {}
      Ok(HashMap::new())
    }

    fn visit_bool<E>(self, _v: bool) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }

    fn visit_i64<E>(self, _v: i64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }

    fn visit_u64<E>(self, _v: u64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }

    fn visit_f64<E>(self, _v: f64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }

    fn visit_string<E>(self, _v: String) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }

    fn visit_str<E>(self, _v: &str) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(HashMap::new())
    }
  }

  struct OptionalStringVisitor;

  impl<'de> Visitor<'de> for OptionalStringVisitor {
    type Value = Option<String>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("string or null")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
      M: MapAccess<'de>,
    {
      while map
        .next_entry::<de::IgnoredAny, de::IgnoredAny>()?
        .is_some()
      {}
      Ok(None)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
      A: SeqAccess<'de>,
    {
      while seq.next_element::<de::IgnoredAny>()?.is_some() {}
      Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
      D: Deserializer<'de>,
    {
      deserializer.deserialize_any(self)
    }

    fn visit_bool<E>(self, _v: bool) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_i64<E>(self, _v: i64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_u64<E>(self, _v: u64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_f64<E>(self, _v: f64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }

    fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Some(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Some(v.to_string()))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(None)
    }
  }

  struct VectorVisitor<T> {
    marker: std::marker::PhantomData<fn() -> Vec<T>>,
  }

  impl<'de, T> Visitor<'de> for VectorVisitor<T>
  where
    T: DeserializeOwned,
  {
    type Value = Vec<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a sequence or null")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
      D: Deserializer<'de>,
    {
      deserializer.deserialize_any(self)
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
      A: SeqAccess<'de>,
    {
      let mut vec = Vec::new();

      while let Some(value) = seq.next_element::<serde_json::Value>()? {
        if let Ok(value) = serde_json::from_value(value) {
          vec.push(value);
        }
      }

      Ok(vec)
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
      M: MapAccess<'de>,
    {
      while map
        .next_entry::<de::IgnoredAny, de::IgnoredAny>()?
        .is_some()
      {}
      Ok(Vec::new())
    }

    fn visit_bool<E>(self, _v: bool) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_i64<E>(self, _v: i64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_u64<E>(self, _v: u64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_f64<E>(self, _v: f64) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_string<E>(self, _v: String) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_str<E>(self, _v: &str) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
      E: de::Error,
    {
      Ok(Vec::new())
    }
  }
}

#[cfg(test)]
mod test {
  use std::collections::HashMap;

  use deno_semver::Version;
  use pretty_assertions::assert_eq;
  use serde_json;

  use super::*;

  #[test]
  fn deserializes_minimal_pkg_info() {
    let text = r#"{ "version": "1.0.0", "dist": { "tarball": "value" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info,
      NpmPackageVersionInfo {
        version: Version::parse_from_npm("1.0.0").unwrap(),
        dist: Some(NpmPackageVersionDistInfo {
          tarball: "value".to_string(),
          shasum: None,
          integrity: None,
          attestations: None,
        }),
        ..Default::default()
      }
    );
  }

  #[test]
  fn from_packument_slice_indexes_versions_and_time() {
    let text = br#"{
      "name": "@scope/pkg",
      "_deno.etag": "etag-1",
      "dist-tags": { "latest": "1.1.0" },
      "versions": {
        "1.0.0": {
          "version": "1.0.0",
          "dependencies": { "dep": "^1.0.0" }
        },
        "1.1.0": {
          "version": "1.1.0",
          "dist": {
            "tarball": "https://example.com/pkg.tgz",
            "attestations": { "provenance": { "predicateType": "x" } }
          }
        }
      },
      "time": {
        "created": "2025-01-01T00:00:00.000Z",
        "modified": "2025-01-03T00:00:00.000Z",
        "1.0.0": "2025-01-01T00:00:00.000Z",
        "1.1.0": "2025-01-02T00:00:00.000Z"
      }
    }"#;

    let (info, etag) =
      NpmPackageInfo::from_packument_slice_with_etag(text).unwrap();
    let version = Version::parse_from_npm("1.1.0").unwrap();

    assert_eq!(info.name, "@scope/pkg");
    assert_eq!(etag, Some("etag-1".to_string()));
    assert_eq!(info.dist_tags["latest"], version);
    assert_eq!(
      info.time[&version],
      "2025-01-02T00:00:00.000Z"
        .parse::<chrono::DateTime<chrono::Utc>>()
        .unwrap()
    );
    assert_eq!(
      info.versions.get(&version).unwrap().get_trust_evidence(),
      Some(TrustEvidence::Provenance)
    );
  }

  #[test]
  fn from_packument_bytes_reuses_lazy_source_allocation() {
    let bytes = br#"{
      "name": "pkg",
      "dist-tags": { "latest": "1.0.0" },
      "versions": { "1.0.0": { "version": "1.0.0" } },
      "time": { "1.0.0": "2024-01-02T00:00:00.000Z" }
    }"#
      .to_vec();
    let bytes_ptr = bytes.as_ptr();

    let info = NpmPackageInfo::from_packument_bytes(bytes).unwrap();

    assert_eq!(
      info
        .lazy_packument_source_bytes()
        .map(|bytes| bytes.as_ptr()),
      Some(bytes_ptr)
    );
  }

  #[test]
  fn from_packument_slice_unescapes_quoted_deno_etag() {
    let text = br#"{
      "name": "pkg",
      "_deno.etag": "\"etag-1\"",
      "dist-tags": { "latest": "1.0.0" },
      "versions": {
        "1.0.0": { "version": "1.0.0" }
      }
    }"#;

    let (_, etag) =
      NpmPackageInfo::from_packument_slice_with_etag(text).unwrap();

    assert_eq!(etag, Some("\"etag-1\"".to_string()));
  }

  #[test]
  fn lazy_packument_version_parse_failure_does_not_panic() {
    let text = br#"{
      "name": "pkg",
      "versions": {
        "1.0.0": { "version": 1 }
      },
      "dist-tags": { "latest": "1.0.0" }
    }"#;

    let info = NpmPackageInfo::from_packument_slice(text).unwrap();
    let version = Version::parse_from_npm("1.0.0").unwrap();

    assert!(info.versions.contains_key(&version));
    assert!(info.versions.get(&version).is_none());
  }

  #[test]
  fn trust_evidence_ranking() {
    fn trust_of(json: &str) -> Option<TrustEvidence> {
      let info: NpmPackageVersionInfo = serde_json::from_str(json).unwrap();
      info.get_trust_evidence()
    }

    // plain token publish: no signals
    assert_eq!(trust_of(r#"{ "version": "1.0.0" }"#), None);

    // provenance attestation only
    assert_eq!(
      trust_of(
        r#"{ "version": "1.0.0", "dist": { "tarball": "t", "attestations": { "provenance": { "predicateType": "x" } } } }"#,
      ),
      Some(TrustEvidence::Provenance)
    );

    // trusted publishing WITHOUT a provenance attestation is not counted: on
    // its own the flag is just metadata, mirroring pnpm's getTrustEvidence.
    assert_eq!(
      trust_of(
        r#"{ "version": "1.0.0", "_npmUser": { "name": "ci", "trustedPublisher": { "id": "github" } } }"#,
      ),
      None
    );

    // trusted publishing backed by a provenance attestation
    assert_eq!(
      trust_of(
        r#"{ "version": "1.0.0", "_npmUser": { "trustedPublisher": { "id": "github" } }, "dist": { "tarball": "t", "attestations": { "provenance": {} } } }"#,
      ),
      Some(TrustEvidence::TrustedPublisher)
    );

    // staged publish (human-approved via 2FA) is strongest
    assert_eq!(
      trust_of(
        r#"{ "version": "1.0.0", "_npmUser": { "approver": { "name": "maintainer" } } }"#,
      ),
      Some(TrustEvidence::StagedPublish)
    );

    // ranks are ordered: provenance < trusted publisher < staged publish
    assert!(TrustEvidence::Provenance < TrustEvidence::TrustedPublisher);
    assert!(TrustEvidence::TrustedPublisher < TrustEvidence::StagedPublish);
    assert_eq!(TrustEvidence::Provenance.rank(), 1);
    assert_eq!(TrustEvidence::TrustedPublisher.rank(), 2);
    assert_eq!(TrustEvidence::StagedPublish.rank(), 3);
  }

  #[test]
  fn trust_signals_serialize_compactly() {
    // The full packument carries large objects in `_npmUser.approver`,
    // `_npmUser.trustedPublisher` and `dist.attestations.provenance`, but we
    // only care that they exist. Re-serializing (as the registry cache does)
    // must collapse them to compact markers and still preserve the trust
    // evidence, so caching the full packument by default stays cheap.
    let info: NpmPackageVersionInfo = serde_json::from_str(
      r#"{
        "version": "1.0.0",
        "_npmUser": { "name": "ci", "trustedPublisher": { "id": "github", "oidcConfigId": "abc" } },
        "dist": { "tarball": "t", "attestations": { "url": "https://example/att", "provenance": { "predicateType": "https://slsa.dev/provenance/v1" } } }
      }"#,
    )
    .unwrap();

    let serialized = serde_json::to_string(&info).unwrap();
    assert!(
      serialized.contains(r#""trustedPublisher":true"#),
      "{serialized}"
    );
    assert!(serialized.contains(r#""provenance":true"#), "{serialized}");
    // none of the dropped sub-fields survive
    assert!(!serialized.contains("oidcConfigId"), "{serialized}");
    assert!(!serialized.contains("predicateType"), "{serialized}");
    assert!(!serialized.contains("\"name\""), "{serialized}");

    // and the trust evidence round-trips through the slimmed form
    let reparsed: NpmPackageVersionInfo =
      serde_json::from_str(&serialized).unwrap();
    assert_eq!(reparsed.get_trust_evidence(), info.get_trust_evidence());
    assert_eq!(
      reparsed.get_trust_evidence(),
      Some(TrustEvidence::TrustedPublisher)
    );
  }

  #[test]
  fn deserializes_serializes_time() {
    let text = r#"{ "name": "package", "versions": {}, "time": { "created": "2015-11-07T19:15:58.747Z", "1.0.0": "2015-11-07T19:15:58.747Z" } }"#;
    let info: NpmPackageInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.time,
      HashMap::from([(
        Version::parse_from_npm("1.0.0").unwrap(),
        "2015-11-07T19:15:58.747Z".parse().unwrap(),
      )])
    );
    assert_eq!(
      serde_json::to_string(&info).unwrap(),
      r#"{"name":"package","versions":{},"dist-tags":{},"time":{"1.0.0":"2015-11-07T19:15:58.747Z"}}"#
    );
  }

  #[test]
  fn deserializes_pkg_info_with_deprecated() {
    let text = r#"{
      "version": "1.0.0",
      "dist": { "tarball": "value", "shasum": "test" },
      "dependencies": ["key","value"],
      "deprecated": "aa"
    }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info,
      NpmPackageVersionInfo {
        version: Version::parse_from_npm("1.0.0").unwrap(),
        dist: Some(NpmPackageVersionDistInfo {
          tarball: "value".to_string(),
          shasum: Some("test".to_string()),
          integrity: None,
          attestations: None,
        }),
        dependencies: HashMap::new(),
        deprecated: Some("aa".to_string()),
        ..Default::default()
      }
    );
  }

  #[test]
  fn deserializes_pkg_info_with_deprecated_invalid() {
    let values = [
      r#"["aa"]"#,
      r#"{ "prop": "aa" }"#,
      "1",
      "1.0",
      "true",
      "null",
    ];
    for value in values {
      let text = format!(
        r#"{{
          "version": "1.0.0",
          "dist": {{ "tarball": "value", "shasum": "test" }},
          "dependencies": ["key","value"],
          "deprecated": {}
        }}"#,
        value
      );
      let info: NpmPackageVersionInfo = serde_json::from_str(&text).unwrap();
      assert_eq!(
        info,
        NpmPackageVersionInfo {
          version: Version::parse_from_npm("1.0.0").unwrap(),
          dist: Some(NpmPackageVersionDistInfo {
            tarball: "value".to_string(),
            shasum: Some("test".to_string()),
            integrity: None,
            attestations: None,
          }),
          dependencies: HashMap::new(),
          deprecated: None,
          ..Default::default()
        }
      );
    }
  }

  #[test]
  fn deserializes_bin_entry() {
    // string
    let text = r#"{ "version": "1.0.0", "bin": "bin-value", "dist": { "tarball": "value", "shasum": "test" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.bin,
      Some(NpmPackageVersionBinEntry::String("bin-value".to_string()))
    );

    // map
    let text = r#"{ "version": "1.0.0", "bin": { "a": "a-value", "b": "b-value" }, "dist": { "tarball": "value", "shasum": "test" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.bin,
      Some(NpmPackageVersionBinEntry::Map(HashMap::from([
        ("a".to_string(), "a-value".to_string()),
        ("b".to_string(), "b-value".to_string()),
      ])))
    );
  }

  #[test]
  fn deserializes_null_entries() {
    let text = r#"{ "version": "1.0.0", "dist": { "tarball": "value", "shasum": "test" }, "dependencies": null, "optionalDependencies": null, "peerDependencies": null, "peerDependenciesMeta": null, "os": null, "cpu": null, "scripts": null }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert!(info.dependencies.is_empty());
    assert!(info.optional_dependencies.is_empty());
    assert!(info.peer_dependencies.is_empty());
    assert!(info.peer_dependencies_meta.is_empty());
    assert!(info.os.is_empty());
    assert!(info.cpu.is_empty());
    assert!(info.scripts.is_empty());
  }

  #[test]
  fn deserializes_bundle_dependencies_aliases() {
    let text = r#"{
      "version": "1.0.0",
      "dist": { "tarball": "value", "shasum": "test" },
      "bundleDependencies": ["a", "b"],
      "bundledDependencies": ["b", "c"]
    }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    let combined: Vec<String> = info
      .bundle_dependencies
      .iter()
      .chain(info.bundled_dependencies.iter())
      .map(|s| s.to_string())
      .collect();
    assert_eq!(
      combined,
      Vec::from([
        "a".to_string(),
        "b".to_string(),
        "b".to_string(),
        "c".to_string(),
      ])
    );
    assert_eq!(
      info.bundle_dependencies(),
      Vec::from(["a".to_string(), "b".to_string()])
    );
  }

  #[test]
  fn deserializes_invalid_kind() {
    #[track_caller]
    fn assert_empty(text: &str) {
      let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
      assert!(info.dependencies.is_empty());
      assert!(info.optional_dependencies.is_empty());
      assert!(info.peer_dependencies.is_empty());
      assert!(info.peer_dependencies_meta.is_empty());
      assert!(info.os.is_empty());
      assert!(info.cpu.is_empty());
      assert!(info.scripts.is_empty());
    }

    // wrong collection kind
    assert_empty(
      r#"{
        "version": "1.0.0",
        "dist": { "tarball": "value", "shasum": "test" },
        "dependencies": [],
        "optionalDependencies": [],
        "peerDependencies": [],
        "peerDependenciesMeta": [],
        "os": {},
        "cpu": {},
        "scripts": []
      }"#,
    );

    // booleans
    assert_empty(
      r#"{
        "version": "1.0.0",
        "dist": { "tarball": "value", "shasum": "test" },
        "dependencies": false,
        "optionalDependencies": true,
        "peerDependencies": false,
        "peerDependenciesMeta": true,
        "os": false,
        "cpu": true,
        "scripts": false
      }"#,
    );

    // strings
    assert_empty(
      r#"{
        "version": "1.0.0",
        "dist": { "tarball": "value", "shasum": "test" },
        "dependencies": "",
        "optionalDependencies": "",
        "peerDependencies": "",
        "peerDependenciesMeta": "",
        "os": "",
        "cpu": "",
        "scripts": ""
      }"#,
    );

    // numbers
    assert_empty(
      r#"{
        "version": "1.0.0",
        "dist": { "tarball": "value", "shasum": "test" },
        "dependencies": 1.23,
        "optionalDependencies": 5,
        "peerDependencies": -2,
        "peerDependenciesMeta": -2.23,
        "os": -63.34,
        "cpu": 12,
        "scripts": -1234.34
      }"#,
    );
  }

  #[test]
  fn deserializes_invalid_collection_items() {
    let text = r#"{
      "version": "1.0.0",
      "dist": { "tarball": "value", "shasum": "test" },
      "dependencies": {
        "value": 123,
        "value1": 123.2,
        "value2": -123,
        "value3": -123.2,
        "value4": true,
        "value5": false,
        "value6": null,
        "value8": {
          "value7": 123,
          "value8": 123.2,
          "value9": -123
        },
        "value9": [
          1,
          2,
          3
        ],
        "value10": "valid"
      },
      "os": [
        123,
        123.2,
        -123,
        -123.2,
        true,
        false,
        null,
        [1, 2, 3],
        {
          "prop": 2
        },
        "valid"
      ]
    }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.dependencies,
      HashMap::from([("value10".into(), "valid".into())])
    );
    assert_eq!(info.os, Vec::from(["valid".to_string()]));
  }

  #[test]
  fn itegrity() {
    // integrity
    let text =
      r#"{ "tarball": "", "integrity": "sha512-testing", "shasum": "here" }"#;
    let info: NpmPackageVersionDistInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.integrity(),
      super::NpmPackageVersionDistInfoIntegrity::Integrity {
        algorithm: "sha512",
        base64_hash: "testing"
      }
    );

    // no integrity
    let text = r#"{ "tarball": "", "shasum": "here" }"#;
    let info: NpmPackageVersionDistInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.integrity(),
      super::NpmPackageVersionDistInfoIntegrity::LegacySha1Hex("here")
    );

    // no dash
    let text = r#"{ "tarball": "", "integrity": "test", "shasum": "here" }"#;
    let info: NpmPackageVersionDistInfo = serde_json::from_str(text).unwrap();
    assert_eq!(
      info.integrity(),
      super::NpmPackageVersionDistInfoIntegrity::UnknownIntegrity("test")
    );
  }

  #[test]
  fn test_parse_dep_entry_name_and_raw_version() {
    let cases = [
      ("test", "^1.2", ("test", "^1.2")),
      ("test", "1.x - 2.6", ("test", "1.x - 2.6")),
      ("test", "npm:package@^1.2", ("package", "^1.2")),
      ("test", "npm:package", ("package", "*")),
      ("test", "npm:@scope/package", ("@scope/package", "*")),
      ("test", "npm:@scope/package@1", ("@scope/package", "1")),
    ];
    for (key, value, expected_result) in cases {
      let key = StackString::from(key);
      let value = StackString::from(value);
      let (name, version) =
        parse_dep_entry_name_and_raw_version(&key, &value).unwrap();
      assert_eq!((name, version), expected_result);
    }
  }

  #[test]
  fn test_parse_dep_entry_name_and_raw_version_error() {
    let err = parse_dep_entry_name_and_raw_version(
      &StackString::from("test"),
      &StackString::from("git:somerepo"),
    )
    .unwrap_err();
    match err {
      NpmDependencyEntryErrorSource::RemoteDependency { specifier } => {
        assert_eq!(specifier, "git:somerepo")
      }
      _ => unreachable!(),
    }
  }

  #[test]
  fn remote_deps_as_entries() {
    for specifier in [
      "https://example.com/something.tgz",
      "git://github.com/example/example",
      "git+ssh://github.com/example/example",
    ] {
      let deps = NpmPackageVersionInfo {
        dependencies: HashMap::from([("a".into(), specifier.into())]),
        ..Default::default()
      };
      let err = deps.dependencies_as_entries("pkg-name").unwrap_err();
      match err.source {
        NpmDependencyEntryErrorSource::RemoteDependency {
          specifier: err_specifier,
        } => assert_eq!(err_specifier, specifier),
        _ => unreachable!(),
      }
    }
  }

  #[test]
  fn test_parse_dep_entry_name_and_raw_version_local_dep() {
    for specifier in ["file:./rtt-plugin", "file:rust-client", "link:../foo"] {
      let err = parse_dep_entry_name_and_raw_version(
        &StackString::from("test"),
        &StackString::from(specifier),
      )
      .unwrap_err();
      match err {
        NpmDependencyEntryErrorSource::LocalDependency {
          specifier: err_specifier,
        } => assert_eq!(err_specifier, specifier),
        _ => unreachable!(),
      }
    }
  }

  #[test]
  fn local_deps_as_entries_are_skipped() {
    for specifier in [
      "file:./rtt-plugin",
      "file:rust-client",
      "file:components/tryghost-parse-email-address-6.22.0.tgz",
      "link:../foo",
    ] {
      let deps = NpmPackageVersionInfo {
        dependencies: HashMap::from([
          ("a".into(), "^1.0.0".into()),
          ("local-dep".into(), specifier.into()),
        ]),
        ..Default::default()
      };
      let entries = deps.dependencies_as_entries("pkg-name").unwrap();
      // The local dependency should be skipped, only "a" remains
      assert_eq!(entries.len(), 1);
      assert_eq!(entries[0].bare_specifier.as_str(), "a");
    }
  }

  #[test]
  fn example_deserialization_fail() {
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub struct SerializedCachedPackageInfo {
      #[serde(flatten)]
      pub info: NpmPackageInfo,
      /// Custom property that includes the etag.
      #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "_denoETag"
      )]
      pub etag: Option<String>,
    }

    let text = r#"{
      "name": "ts-morph",
      "versions": {
        "10.0.2": {
          "version": "10.0.2",
          "dist": {
            "tarball": "https://registry.npmjs.org/ts-morph/-/ts-morph-10.0.2.tgz",
            "shasum": "292418207db467326231b2be92828b5e295e7946",
            "integrity": "sha512-TVuIfEqtr9dW25K3Jajqpqx7t/zLRFxKu2rXQZSDjTm4MO4lfmuj1hn8WEryjeDDBFcNOCi+yOmYUYR4HucrAg=="
          },
          "bin": null,
          "dependencies": {
            "code-block-writer": "^10.1.1",
            "@ts-morph/common": "~0.9.0"
          },
          "deprecated": null
        }
      },
      "dist-tags": { "rc": "2.0.4-rc", "latest": "25.0.1" }
    }"#;
    let result = serde_json::from_str::<SerializedCachedPackageInfo>(text);
    assert!(result.is_ok());
  }

  #[test]
  fn minimize_serialization_version_info() {
    let data = NpmPackageVersionInfo {
      version: Version::parse_from_npm("1.0.0").unwrap(),
      exports: Default::default(),
      dist: Default::default(),
      bin: Default::default(),
      dependencies: Default::default(),
      bundle_dependencies: Default::default(),
      bundled_dependencies: Default::default(),
      optional_dependencies: Default::default(),
      peer_dependencies: Default::default(),
      peer_dependencies_meta: Default::default(),
      os: Default::default(),
      cpu: Default::default(),
      scripts: Default::default(),
      has_install_script: Default::default(),
      deprecated: Default::default(),
      npm_user: Default::default(),
    };
    let text = serde_json::to_string(&data).unwrap();
    assert_eq!(text, r#"{"version":"1.0.0"}"#);
  }

  #[test]
  fn export_keys_deserialize_keeps_only_subpath_keys() {
    // Registry `exports` is deserialized down to the completion subpath keys
    // only — the (potentially large) nested value is never retained.
    let keys: NpmPackageExportKeys =
      serde_json::from_value(serde_json::json!({
        ".": "./index.js",
        "./client": "./client.js",
        "./server": { "types": "./server.d.ts", "default": "./server.js" },
        "./features/*": "./features/*.js",
        "import": "./index.mjs"
      }))
      .unwrap();
    let mut sorted = keys.0.clone();
    sorted.sort();
    assert_eq!(sorted, vec![".", "./client", "./server"]);

    // String / array / absent forms.
    assert_eq!(
      serde_json::from_value::<NpmPackageExportKeys>(serde_json::json!(
        "./index.js"
      ))
      .unwrap()
      .0,
      vec!["."]
    );
    assert!(
      serde_json::from_value::<NpmPackageExportKeys>(serde_json::json!([
        "./a.js", "./b.js"
      ]))
      .unwrap()
      .0
      .is_empty()
    );

    // Round-trips through serialize -> deserialize.
    let text = serde_json::to_string(&keys).unwrap();
    let mut round_tripped = serde_json::from_str::<NpmPackageExportKeys>(&text)
      .unwrap()
      .0;
    round_tripped.sort();
    assert_eq!(round_tripped, sorted);
  }

  #[test]
  fn version_info_exports_skipped_by_default() {
    // The normal parse (e.g. `deno run`) discards `exports` entirely — nothing
    // is retained, not even the subpath keys.
    let text = r#"{ "version": "1.0.0", "exports": { ".": "./index.js", "./client": "./client.js" } }"#;
    let info: NpmPackageVersionInfo = serde_json::from_str(text).unwrap();
    assert_eq!(info.exports, None);
  }

  #[test]
  fn fill_export_keys_populates_from_packument() {
    let text = r#"{
      "name": "pkg",
      "versions": {
        "1.0.0": { "version": "1.0.0", "exports": { ".": "./index.js", "./client": "./client.js", "import": "./index.mjs" } },
        "2.0.0": { "version": "2.0.0" }
      }
    }"#;
    let mut info: NpmPackageInfo = serde_json::from_str(text).unwrap();
    // Skipped during the normal parse.
    assert_eq!(
      info.versions[&Version::parse_from_npm("1.0.0").unwrap()].exports,
      None
    );

    info.fill_export_keys(text.as_bytes()).unwrap();

    let mut keys = info.versions[&Version::parse_from_npm("1.0.0").unwrap()]
      .exports
      .clone()
      .unwrap()
      .0;
    keys.sort();
    assert_eq!(keys, vec![".", "./client"]);
    // A version without `exports` stays empty.
    assert_eq!(
      info.versions[&Version::parse_from_npm("2.0.0").unwrap()].exports,
      None
    );
  }

  #[test]
  fn minimize_serialization_dist() {
    let data = NpmPackageVersionDistInfo {
      tarball: "test".to_string(),
      shasum: None,
      integrity: None,
      attestations: None,
    };
    let text = serde_json::to_string(&data).unwrap();
    assert_eq!(text, r#"{"tarball":"test"}"#);
  }
}
