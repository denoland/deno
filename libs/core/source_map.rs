// Copyright 2018-2026 the Deno authors. MIT license.

//! This mod provides functions to remap a `JsError` based on a source map.

use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

pub use sourcemap::SourceMap;

use crate::ModuleLoader;
use crate::ModuleName;
use crate::resolve_url;

#[derive(Debug, PartialEq)]
pub enum SourceMapApplication {
  /// No mapping was applied, the location is unchanged.
  Unchanged,
  /// Line and column were mapped to a new location.
  LineAndColumn {
    line_number: u32,
    column_number: u32,
  },
  /// Line, column and file name were mapped to a new location.
  LineAndColumnAndFileName {
    file_name: String,
    line_number: u32,
    column_number: u32,
  },
}

pub type SourceMapData = Cow<'static, [u8]>;

/// A tiny bounded cache with least-recently-used eviction.
///
/// The source-map caches used to be unbounded `HashMap`s that also memoized
/// *negative* lookups, so a long-lived process throwing errors from many
/// distinct files grew them forever. Eviction is a linear scan for the oldest
/// entry, which is fine at these capacities and only runs when full.
struct LruCache<K, V> {
  entries: HashMap<K, (V, u64)>,
  capacity: usize,
  clock: u64,
}

impl<K: std::hash::Hash + Eq + Clone, V> LruCache<K, V> {
  fn new(capacity: usize) -> Self {
    debug_assert!(capacity > 0);
    Self {
      entries: HashMap::with_capacity(capacity.min(16)),
      capacity,
      clock: 0,
    }
  }

  fn get<Q>(&mut self, key: &Q) -> Option<&V>
  where
    K: std::borrow::Borrow<Q>,
    Q: std::hash::Hash + Eq + ?Sized,
  {
    self.clock += 1;
    let tick = self.clock;
    let entry = self.entries.get_mut(key)?;
    entry.1 = tick;
    Some(&entry.0)
  }

  fn insert(&mut self, key: K, value: V) {
    self.clock += 1;
    if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
      let oldest = self
        .entries
        .iter()
        .min_by_key(|(_, (_, tick))| *tick)
        .map(|(k, _)| k.clone());
      if let Some(oldest) = oldest {
        self.entries.remove(&oldest);
      }
    }
    self.entries.insert(key, (value, self.clock));
  }
}

/// How many decoded source maps to keep. Stack traces walk a handful of
/// distinct files, so this is generous.
const MAX_CACHED_SOURCE_MAPS: usize = 64;
/// How many `(file, line)` source lines to keep.
const MAX_CACHED_SOURCE_LINES: usize = 256;

pub struct SourceMapper {
  /// Decoded source maps, bounded and positive-only: a file with no source
  /// map is not memoized, so a pathological error loop re-asks the loader
  /// instead of pinning an entry per file forever.
  maps: LruCache<String, Arc<SourceMap>>,
  source_lines: LruCache<(String, i64), String>,

  loader: Rc<dyn ModuleLoader>,

  ext_source_maps: HashMap<ModuleName, SourceMapData>,
  /// Source map URL as reported by V8 at compile time, resolved against the
  /// module URL. May be a `data:` URL carrying the map inline — it is kept
  /// *undecoded* here and only parsed if a stack trace actually needs it.
  source_map_urls: HashMap<ModuleName, String>,
}

impl SourceMapper {
  pub fn new(loader: Rc<dyn ModuleLoader>) -> Self {
    Self {
      maps: LruCache::new(MAX_CACHED_SOURCE_MAPS),
      source_lines: LruCache::new(MAX_CACHED_SOURCE_LINES),
      ext_source_maps: Default::default(),
      source_map_urls: Default::default(),
      loader,
    }
  }

  /// Add a source map for particular `ext:` module.
  pub(crate) fn add_ext_source_map(
    &mut self,
    module_name: ModuleName,
    source_map_data: SourceMapData,
  ) {
    self.ext_source_maps.insert(module_name, source_map_data);
  }

  pub(crate) fn take_ext_source_maps(
    &mut self,
  ) -> HashMap<ModuleName, SourceMapData> {
    std::mem::take(&mut self.ext_source_maps)
  }

  /// Records the source map URL V8 reported for a module. This is either an
  /// external URL (already resolved against the module URL) or an inline
  /// `data:` URL. Either way it is stored verbatim and decoded lazily — most
  /// modules never appear in a stack trace, and decoding eagerly used to
  /// keep a fully parsed [`SourceMap`] alive for every compiled module.
  pub(crate) fn add_source_map_url(
    &mut self,
    module_name: ModuleName,
    source_map_url: String,
  ) {
    self.source_map_urls.insert(module_name, source_map_url);
  }

  /// Returns the decoded source map for `file_name`, decoding (and caching)
  /// it on first use. Misses are deliberately *not* cached.
  fn source_map_for(&mut self, file_name: &str) -> Option<Arc<SourceMap>> {
    if let Some(source_map) = self.maps.get(file_name) {
      return Some(source_map.clone());
    }
    let source_map = Arc::new(self.decode_source_map(file_name)?);
    self.maps.insert(file_name.to_owned(), source_map.clone());
    Some(source_map)
  }

  fn decode_source_map(&self, file_name: &str) -> Option<SourceMap> {
    // Inline `ext:` source maps.
    if let Some(data) = self.ext_source_maps.get(file_name)
      && let Ok(source_map) = SourceMap::from_slice(data)
    {
      return Some(source_map);
    }

    // The URL V8 reported at compile time: either inline data or external.
    if let Some(source_map_url) = self.source_map_urls.get(file_name) {
      if source_map_url.starts_with("data:") {
        if let Ok(sourcemap::DecodedMap::Regular(source_map)) =
          sourcemap::decode_data_url(source_map_url)
        {
          return Some(source_map);
        }
      } else if let Some(data) =
        self.loader.load_external_source_map(source_map_url)
        && let Ok(source_map) = SourceMap::from_slice(&data)
      {
        return Some(source_map);
      }
    }

    // The loader's own inline source maps.
    if let Some(data) = self.loader.get_source_map(file_name)
      && let Ok(source_map) = SourceMap::from_slice(&data)
    {
      return Some(source_map);
    }

    None
  }

  /// Apply a source map to the passed location. If there is no source map for
  /// this location, or if the location remains unchanged after mapping, the
  /// changed values are returned.
  ///
  /// Line and column numbers are 1-based.
  pub fn apply_source_map(
    &mut self,
    file_name: &str,
    line_number: u32,
    column_number: u32,
  ) -> SourceMapApplication {
    // Lookup expects 0-based line and column numbers, but ours are 1-based.
    let line_number = line_number - 1;
    let column_number = column_number - 1;

    let Some(source_map) = self.source_map_for(file_name) else {
      return SourceMapApplication::Unchanged;
    };

    let Some(token) = source_map.lookup_token(line_number, column_number)
    else {
      return SourceMapApplication::Unchanged;
    };

    let new_line_number = token.get_src_line() + 1;
    let new_column_number = token.get_src_col() + 1;

    let new_file_name = match token.get_source() {
      Some(source_file_name) => {
        if source_file_name == file_name {
          None
        } else {
          // The `source_file_name` written by tsc in the source map is
          // sometimes only the basename of the URL, or has unwanted `<`/`>`
          // around it. Try to parse it as a URL first. If that fails,
          // try to resolve it as a relative path from the module URL.
          match resolve_url(source_file_name) {
            Ok(m) if m.scheme() == "blob" => None,
            Ok(m) => Some(m.to_string()),
            Err(_) => resolve_url(file_name)
              .ok()
              .and_then(|base_url| base_url.join(source_file_name).ok())
              .and_then(|resolved| {
                let resolved_str = resolved.to_string();
                // Only rewrite file name if the source file actually exists.
                // This prevents npm packages with source maps pointing to
                // non-distributed source files from breaking stack traces.
                match self.loader.source_map_source_exists(&resolved_str) {
                  Some(true) => Some(resolved_str),
                  _ => None,
                }
              }),
          }
        }
      }
      None => None,
    };

    match new_file_name {
      None => SourceMapApplication::LineAndColumn {
        line_number: new_line_number,
        column_number: new_column_number,
      },
      Some(file_name) => SourceMapApplication::LineAndColumnAndFileName {
        file_name,
        line_number: new_line_number,
        column_number: new_column_number,
      },
    }
  }

  const MAX_SOURCE_LINE_LENGTH: usize = 150;

  pub fn get_source_line(
    &mut self,
    file_name: &str,
    line_number: i64,
  ) -> Option<String> {
    let key = (file_name.to_string(), line_number);
    if let Some(source_line) = self.source_lines.get(&key) {
      return Some(source_line.clone());
    }

    let source_line = self
      .loader
      .get_source_mapped_source_line(file_name, (line_number - 1) as usize)
      .filter(|s| s.len() <= Self::MAX_SOURCE_LINE_LENGTH)?;

    self.source_lines.insert(key, source_line.clone());
    Some(source_line)
  }
}

#[cfg(test)]
mod tests {
  use url::Url;

  use super::*;
  use crate::ModuleCodeString;
  use crate::ModuleLoadReferrer;
  use crate::ModuleLoadResponse;
  use crate::ModuleSpecifier;
  use crate::ResolutionKind;
  use crate::ascii_str;
  use crate::modules::ModuleLoadOptions;

  struct SourceMapLoaderContent {
    source_map: Option<ModuleCodeString>,
  }

  #[derive(Default)]
  pub struct SourceMapLoader {
    map: HashMap<ModuleSpecifier, SourceMapLoaderContent>,
    existing_files: std::cell::RefCell<std::collections::HashSet<String>>,
  }

  impl SourceMapLoader {
    fn add_existing_file(&self, file_name: &str) {
      self
        .existing_files
        .borrow_mut()
        .insert(file_name.to_string());
    }
  }

  impl ModuleLoader for SourceMapLoader {
    fn resolve(
      &self,
      _specifier: &str,
      _referrer: &str,
      _kind: ResolutionKind,
    ) -> crate::ModuleResolveResponse {
      unreachable!()
    }

    fn load(
      &self,
      _module_specifier: &ModuleSpecifier,
      _maybe_referrer: Option<&ModuleLoadReferrer>,
      _options: ModuleLoadOptions,
    ) -> ModuleLoadResponse {
      unreachable!()
    }

    fn get_source_map(&self, file_name: &str) -> Option<Cow<'_, [u8]>> {
      let url = Url::parse(file_name).unwrap();
      let content = self.map.get(&url)?;
      content
        .source_map
        .as_ref()
        .map(|s| Cow::Borrowed(s.as_bytes()))
    }

    fn get_source_mapped_source_line(
      &self,
      _file_name: &str,
      _line_number: usize,
    ) -> Option<String> {
      Some("fake source line".to_string())
    }

    fn source_map_source_exists(&self, source_url: &str) -> Option<bool> {
      Some(self.existing_files.borrow().contains(source_url))
    }
  }

  #[test]
  fn test_source_mapper() {
    let mut loader = SourceMapLoader::default();
    loader.map.insert(
      Url::parse("file:///b.js").unwrap(),
      SourceMapLoaderContent { source_map: None },
    );
    loader.map.insert(
      Url::parse("file:///a.ts").unwrap(),
      SourceMapLoaderContent {
        source_map: Some(ascii_str!(r#"{"version":3,"sources":["file:///a.ts"],"sourcesContent":["export function a(): string {\n  return \"a\";\n}\n"],"names":[],"mappings":"AAAA,OAAO,SAAS;EACd,OAAO;AACT"}"#).into()),
      },
    );

    let mut source_mapper = SourceMapper::new(Rc::new(loader));

    // Non-existent file
    let application =
      source_mapper.apply_source_map("file:///doesnt_exist.js", 1, 1);
    assert_eq!(application, SourceMapApplication::Unchanged);

    // File with no source map
    let application = source_mapper.apply_source_map("file:///b.js", 1, 1);
    assert_eq!(application, SourceMapApplication::Unchanged);

    // File with a source map
    let application = source_mapper.apply_source_map("file:///a.ts", 1, 21);
    assert_eq!(
      application,
      SourceMapApplication::LineAndColumn {
        line_number: 1,
        column_number: 17
      }
    );

    let line = source_mapper.get_source_line("file:///a.ts", 1).unwrap();
    assert_eq!(line, "fake source line");
    // Get again to hit a cache
    let line = source_mapper.get_source_line("file:///a.ts", 1).unwrap();
    assert_eq!(line, "fake source line");
  }

  #[test]
  fn test_source_map_relative_path_nonexistent_file() {
    // This is important for npm packages that ship source maps pointing to
    // source files that aren't distributed.
    let mut loader = SourceMapLoader::default();
    loader.map.insert(
      Url::parse("file:///project/dist/bundle.js").unwrap(),
      SourceMapLoaderContent {
        // Source map with relative path "../src/index.ts" that doesn't exist
        source_map: Some(ascii_str!(r#"{"version":3,"sources":["../src/index.ts"],"sourcesContent":["console.log('hello');\n"],"names":[],"mappings":"AAAA,QAAQ,IAAI"}"#).into()),
      },
    );

    let mut source_mapper = SourceMapper::new(Rc::new(loader));

    // The source file "../src/index.ts" resolved to "file:///project/src/index.ts"
    // doesn't exist, so we should only get line/column mapping without file rename
    let application =
      source_mapper.apply_source_map("file:///project/dist/bundle.js", 1, 1);
    assert_eq!(
      application,
      SourceMapApplication::LineAndColumn {
        line_number: 1,
        column_number: 1
      }
    );
  }

  #[test]
  fn test_source_map_relative_path_existing_file() {
    // Test that relative paths pointing to existing files DO rewrite the file name
    let mut loader = SourceMapLoader::default();
    loader.map.insert(
      Url::parse("file:///project/dist/bundle.js").unwrap(),
      SourceMapLoaderContent {
        // Source map with relative path "../src/index.ts"
        source_map: Some(ascii_str!(r#"{"version":3,"sources":["../src/index.ts"],"sourcesContent":["console.log('hello');\n"],"names":[],"mappings":"AAAA,QAAQ,IAAI"}"#).into()),
      },
    );
    loader.add_existing_file("file:///project/src/index.ts");

    let mut source_mapper = SourceMapper::new(Rc::new(loader));

    let application =
      source_mapper.apply_source_map("file:///project/dist/bundle.js", 1, 1);
    assert_eq!(
      application,
      SourceMapApplication::LineAndColumnAndFileName {
        file_name: "file:///project/src/index.ts".to_string(),
        line_number: 1,
        column_number: 1
      }
    );
  }
}
