// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

use crate::source_map_bundler::SourceMapBundler;
use crate::WrittenFile;

use deno_core::ErrBox;
use regex::Regex;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::result;

type Result<V> = result::Result<V, ErrBox>;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Module {
  source: Option<String>,
  map: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Bundle {
  pub modules: HashMap<String, Module>,
  pub cache_dirty: bool,
  pub maybe_cache: Option<PathBuf>,
}

fn count_newlines(s: &str) -> usize {
  bytecount::count(s.as_bytes(), b'\n')
}

impl Bundle {
  pub fn new(maybe_cache: Option<PathBuf>) -> Self {
    let mut bundle = Bundle {
      modules: HashMap::new(),
      cache_dirty: false,
      maybe_cache,
    };
    bundle.read_cache().expect("unable to read bundle cache");
    bundle
  }

  /// Enumerate the written files in a vector and add them to the internal
  /// cache, which is then used to generate the bundle
  pub fn insert_written(&mut self, written_files: Vec<WrittenFile>) {
    for file in written_files.iter() {
      let source_map_pragma_re =
        Regex::new(r"/{2}#\s+sourceMappingURL\s?=.+\n*$").unwrap();
      let is_map = file.url.ends_with(".map");
      let module =
        self
          .modules
          .entry(file.module_name.clone())
          .or_insert(Module {
            source: None,
            map: None,
          });
      if is_map {
        module.map = Some(file.source_code.clone());
      } else {
        let source = source_map_pragma_re
          .replace(&file.source_code, "")
          .to_string();
        module.source = Some(source);
      }
      self.cache_dirty = true;
    }
  }

  /// Read the cache from file into memory, it one is supplied and it exists.
  fn read_cache(&mut self) -> Result<()> {
    if let Some(path) = self.maybe_cache.clone() {
      if path.is_file() {
        let j = std::fs::read_to_string(path)?;
        let source: HashMap<String, Module> = serde_json::from_str(&j)?;
        self.modules.clone_from(&source)
      }
    }
    Ok(())
  }

  /// If a cache file name is supplied, write the cache to disk.
  fn write_cache(self) -> Result<()> {
    if self.cache_dirty {
      if let Some(path) = self.maybe_cache.clone() {
        let contents = serde_json::to_string_pretty(&self.modules)?;
        std::fs::write(path, contents)?;
      }
    }
    Ok(())
  }

  /// Write out the bundle modules to a single JavaScript file and a single
  /// source map file.
  pub fn write_bundle(
    self,
    file_name: &Path,
    maybe_preamble: Option<&str>,
    maybe_postscript: Option<&str>,
  ) -> Result<()> {
    let mut source_code = String::new();
    if let Some(preamble) = maybe_preamble {
      source_code.push_str(preamble);
    }
    let mut line_offset = count_newlines(&source_code);
    let mut source_map_bundle =
      SourceMapBundler::new(file_name.file_name().unwrap().to_str());
    for (module_name, module) in self.modules.iter() {
      if let Some(source) = &module.source {
        source_code.push_str(source);
      } else {
        panic!(format!("module \"${}\" is missing its source", module_name));
      }
      if let Some(source_map) = &module.map {
        source_map_bundle
          .append_from_str(source_map, line_offset)
          .expect("unable to append source_map");
      } else {
        panic!(format!(
          "module \"${}\" is missing its source map",
          module_name
        ));
      }
      line_offset = count_newlines(&source_code);
    }
    if let Some(postscript) = maybe_postscript {
      source_code.push_str(postscript);
    }
    let mut map_file_name = file_name.to_path_buf();
    map_file_name.set_extension("js.map");
    let source_map_pragma = format!(
      "\n//# sourceMappingURL={}\n",
      map_file_name.file_name().unwrap().to_string_lossy()
    );
    source_code.push_str(&source_map_pragma);
    std::fs::write(file_name, source_code)
      .expect("unable to write bundle source");
    let mut contents: Vec<u8> = vec![];
    source_map_bundle
      .into_sourcemap()
      .to_writer(&mut contents)
      .expect("unable to output source map");
    std::fs::write(map_file_name, contents)
      .expect("unable to write bundle map");
    self.write_cache().expect("unable to write bundle cache");
    Ok(())
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use std::env;
  use tempfile::TempDir;

  #[test]
  fn test_bundle_insert_written() {
    let mut bundle = Bundle::new(None);
    bundle.insert_written(vec![
      WrittenFile {
        module_name: "internal:///main.ts".to_string(),
        source_code:
          "console.log(\"hello world\");\n//# sourceMappingURL=main.js.map\n"
            .to_string(),
        url: "internal:///main.js".to_string(),
      },
      WrittenFile {
        module_name: "internal:///main.ts".to_string(),
        source_code: "{}".to_string(),
        url: "internal:///main.js.map".to_string(),
      },
    ]);
    assert_eq!(
      bundle.modules.get("internal:///main.ts").unwrap().source,
      Some("console.log(\"hello world\");\n".to_string())
    );
    assert_eq!(
      bundle.modules.get("internal:///main.ts").unwrap().map,
      Some("{}".to_string())
    );
  }

  #[test]
  fn test_bundle_write_bundle() {
    let temp_dir = TempDir::new().unwrap();
    let o = temp_dir.path().to_owned();
    let file_name = o.join("TEST_BUNDLE.js");
    let mut bundle = Bundle::new(None);
    bundle.insert_written(vec![
      WrittenFile {
        module_name: "internal:///main.ts".to_string(),
        source_code:
          "import * as a from \"./a.ts\";\n\nconsole.log(\"hello world\");\n"
            .to_string(),
        url: "internal:///main.js".to_string(),
      },
      WrittenFile {
        module_name: "internal:///main.ts".to_string(),
        source_code: "{}".to_string(),
        url: "internal:///main.js.map".to_string(),
      },
      WrittenFile {
        module_name: "internal:///a.ts".to_string(),
        source_code: "export const b = \"b\";\n".to_string(),
        url: "internal:///a.js".to_string(),
      },
      WrittenFile {
        module_name: "internal:///a.ts".to_string(),
        source_code: "{}".to_string(),
        url: "internal:///a.js.map".to_string(),
      },
    ]);
    bundle.write_bundle(&file_name, None, None).unwrap();
    let mut map_file_name = file_name.clone();
    map_file_name.set_extension("js.map");
    assert!(file_name.is_file());
    assert!(map_file_name.is_file());
  }

  #[test]
  fn test_bundle_with_cache() {
    let c = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let maybe_cache = Some(c.join("tests/test.cache"));
    let bundle = Bundle::new(maybe_cache);
    assert_eq!(bundle.modules.len(), 2);
    assert!(bundle.modules.contains_key("internal:///main.ts"));
    assert!(bundle.modules.contains_key("internal:///a.ts"));
  }
}
