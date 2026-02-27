// Copyright 2018-2025 the Deno authors. MIT license.

//! This mod provides functions to remap a `JsError` based on a source map.

use crate::ModuleLoader;
use crate::ModuleName;
use crate::resolve_url;
pub use sourcemap::SourceMap;
use std::borrow::Cow;
use std::collections::HashMap;
use std::rc::Rc;
use std::str;
use std::sync::Arc;

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

pub struct SourceMapper {
  // TODO(bartlomieju): I feel like these two should be cleared when Isolate
  // reaches "near heap limit" to free up some space. This needs to be confirmed though.
  maps: HashMap<String, Option<Arc<SourceMap>>>,
  source_lines: HashMap<(String, i64), Option<String>>,

  loader: Rc<dyn ModuleLoader>,

  ext_source_maps: HashMap<ModuleName, SourceMapData>,
  source_map_urls: HashMap<ModuleName, String>,
}

impl SourceMapper {
  pub fn new(loader: Rc<dyn ModuleLoader>) -> Self {
    Self {
      maps: Default::default(),
      source_lines: Default::default(),
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

  /// Add a source map extracted from V8 for a module.
  pub(crate) fn add_source_map(
    &mut self,
    module_name: ModuleName,
    source_map: SourceMap,
  ) {
    self
      .maps
      .insert(module_name.to_string(), Some(Arc::new(source_map)));
  }

  pub(crate) fn add_source_map_url(
    &mut self,
    module_name: ModuleName,
    source_map_url: String,
  ) {
    self.source_map_urls.insert(module_name, source_map_url);
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

    let maybe_source_map =
      self.maps.entry(file_name.to_owned()).or_insert_with(|| {
        None
          // Try ext: source maps (inline)
          .or_else(|| {
            SourceMap::from_slice(self.ext_source_maps.get(file_name)?)
              .ok()
              .map(Arc::new)
          })
          // Try external source maps via ModuleLoader
          .or_else(|| {
            // Check if we have an external source map URL for this file
            let source_map_url = self.source_map_urls.get(file_name)?;
            // Request the external source map from the loader
            let source_map_data =
              self.loader.load_external_source_map(source_map_url)?;
            SourceMap::from_slice(&source_map_data).ok().map(Arc::new)
          })
          // Try loader's inline source maps
          .or_else(|| {
            SourceMap::from_slice(&self.loader.get_source_map(file_name)?)
              .ok()
              .map(Arc::new)
          })
      });

    let Some(source_map) = maybe_source_map.as_ref() else {
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
    if let Some(maybe_source_line) =
      self.source_lines.get(&(file_name.to_string(), line_number))
    {
      return maybe_source_line.clone();
    }

    let maybe_source_line = self
      .loader
      .get_source_mapped_source_line(file_name, (line_number - 1) as usize)
      .filter(|s| s.len() <= Self::MAX_SOURCE_LINE_LENGTH);

    // Cache and return
    self.source_lines.insert(
      (file_name.to_string(), line_number),
      maybe_source_line.clone(),
    );
    maybe_source_line
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
  use crate::error::ModuleLoaderError;
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
    ) -> Result<ModuleSpecifier, ModuleLoaderError> {
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
