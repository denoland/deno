// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use dashmap::DashMap;
use deno_media_type::MediaType;
use url::Url;

use crate::fs::DenoResolverFs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SloppyImportsFsEntry {
  File,
  Dir,
}

impl SloppyImportsFsEntry {
  pub fn from_fs_stat(
    stat: &deno_runtime::deno_io::fs::FsStat,
  ) -> Option<SloppyImportsFsEntry> {
    if stat.is_file {
      Some(SloppyImportsFsEntry::File)
    } else if stat.is_directory {
      Some(SloppyImportsFsEntry::Dir)
    } else {
      None
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SloppyImportsResolution {
  /// Ex. `./file.js` to `./file.ts`
  JsToTs(Url),
  /// Ex. `./file` to `./file.ts`
  NoExtension(Url),
  /// Ex. `./dir` to `./dir/index.ts`
  Directory(Url),
}

impl SloppyImportsResolution {
  pub fn as_specifier(&self) -> &Url {
    match self {
      Self::JsToTs(specifier) => specifier,
      Self::NoExtension(specifier) => specifier,
      Self::Directory(specifier) => specifier,
    }
  }

  pub fn into_specifier(self) -> Url {
    match self {
      Self::JsToTs(specifier) => specifier,
      Self::NoExtension(specifier) => specifier,
      Self::Directory(specifier) => specifier,
    }
  }

  pub fn as_suggestion_message(&self) -> String {
    format!("Maybe {}", self.as_base_message())
  }

  pub fn as_quick_fix_message(&self) -> String {
    let message = self.as_base_message();
    let mut chars = message.chars();
    format!(
      "{}{}.",
      chars.next().unwrap().to_uppercase(),
      chars.as_str()
    )
  }

  fn as_base_message(&self) -> String {
    match self {
      SloppyImportsResolution::JsToTs(specifier) => {
        let media_type = MediaType::from_specifier(specifier);
        format!("change the extension to '{}'", media_type.as_ts_extension())
      }
      SloppyImportsResolution::NoExtension(specifier) => {
        let media_type = MediaType::from_specifier(specifier);
        format!("add a '{}' extension", media_type.as_ts_extension())
      }
      SloppyImportsResolution::Directory(specifier) => {
        let file_name = specifier
          .path()
          .rsplit_once('/')
          .map(|(_, file_name)| file_name)
          .unwrap_or(specifier.path());
        format!("specify path to '{}' file in directory instead", file_name)
      }
    }
  }
}

/// The kind of resolution currently being done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SloppyImportsResolutionMode {
  /// Resolving for code that will be executed.
  Execution,
  /// Resolving for code that will be used for type information.
  Types,
}

impl SloppyImportsResolutionMode {
  pub fn is_types(&self) -> bool {
    *self == SloppyImportsResolutionMode::Types
  }
}

#[derive(Debug)]
pub struct SloppyImportsResolver<Fs: DenoResolverFs> {
  fs: Fs,
  cache: Option<DashMap<PathBuf, Option<SloppyImportsFsEntry>>>,
}

impl<Fs: DenoResolverFs> SloppyImportsResolver<Fs> {
  pub fn new(fs: Fs) -> Self {
    Self {
      fs,
      cache: Some(Default::default()),
    }
  }

  pub fn new_without_stat_cache(fs: Fs) -> Self {
    Self { fs, cache: None }
  }

  pub fn resolve(
    &self,
    specifier: &Url,
    mode: SloppyImportsResolutionMode,
  ) -> Option<SloppyImportsResolution> {
    fn path_without_ext(
      path: &Path,
      media_type: MediaType,
    ) -> Option<Cow<str>> {
      let old_path_str = path.to_string_lossy();
      match media_type {
        MediaType::Unknown => Some(old_path_str),
        _ => old_path_str
          .strip_suffix(media_type.as_ts_extension())
          .map(|s| Cow::Owned(s.to_string())),
      }
    }

    fn media_types_to_paths(
      path_no_ext: &str,
      original_media_type: MediaType,
      probe_media_type_types: Vec<MediaType>,
      reason: SloppyImportsResolutionReason,
    ) -> Vec<(PathBuf, SloppyImportsResolutionReason)> {
      probe_media_type_types
        .into_iter()
        .filter(|media_type| *media_type != original_media_type)
        .map(|media_type| {
          (
            PathBuf::from(format!(
              "{}{}",
              path_no_ext,
              media_type.as_ts_extension()
            )),
            reason,
          )
        })
        .collect::<Vec<_>>()
    }

    if specifier.scheme() != "file" {
      return None;
    }

    let path = url_to_file_path(specifier).ok()?;

    #[derive(Clone, Copy)]
    enum SloppyImportsResolutionReason {
      JsToTs,
      NoExtension,
      Directory,
    }

    let probe_paths: Vec<(PathBuf, SloppyImportsResolutionReason)> =
      match self.stat_sync(&path) {
        Some(SloppyImportsFsEntry::File) => {
          if mode.is_types() {
            let media_type = MediaType::from_specifier(specifier);
            // attempt to resolve the .d.ts file before the .js file
            let probe_media_type_types = match media_type {
              MediaType::JavaScript => {
                vec![(MediaType::Dts), MediaType::JavaScript]
              }
              MediaType::Mjs => {
                vec![MediaType::Dmts, MediaType::Dts, MediaType::Mjs]
              }
              MediaType::Cjs => {
                vec![MediaType::Dcts, MediaType::Dts, MediaType::Cjs]
              }
              _ => return None,
            };
            let path_no_ext = path_without_ext(&path, media_type)?;
            media_types_to_paths(
              &path_no_ext,
              media_type,
              probe_media_type_types,
              SloppyImportsResolutionReason::JsToTs,
            )
          } else {
            return None;
          }
        }
        entry @ None | entry @ Some(SloppyImportsFsEntry::Dir) => {
          let media_type = MediaType::from_specifier(specifier);
          let probe_media_type_types = match media_type {
            MediaType::JavaScript => (
              if mode.is_types() {
                vec![MediaType::TypeScript, MediaType::Tsx, MediaType::Dts]
              } else {
                vec![MediaType::TypeScript, MediaType::Tsx]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::Jsx => {
              (vec![MediaType::Tsx], SloppyImportsResolutionReason::JsToTs)
            }
            MediaType::Mjs => (
              if mode.is_types() {
                vec![MediaType::Mts, MediaType::Dmts, MediaType::Dts]
              } else {
                vec![MediaType::Mts]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::Cjs => (
              if mode.is_types() {
                vec![MediaType::Cts, MediaType::Dcts, MediaType::Dts]
              } else {
                vec![MediaType::Cts]
              },
              SloppyImportsResolutionReason::JsToTs,
            ),
            MediaType::TypeScript
            | MediaType::Mts
            | MediaType::Cts
            | MediaType::Dts
            | MediaType::Dmts
            | MediaType::Dcts
            | MediaType::Tsx
            | MediaType::Json
            | MediaType::Wasm
            | MediaType::TsBuildInfo
            | MediaType::SourceMap => {
              return None;
            }
            MediaType::Unknown => (
              if mode.is_types() {
                vec![
                  MediaType::TypeScript,
                  MediaType::Tsx,
                  MediaType::Mts,
                  MediaType::Dts,
                  MediaType::Dmts,
                  MediaType::Dcts,
                  MediaType::JavaScript,
                  MediaType::Jsx,
                  MediaType::Mjs,
                ]
              } else {
                vec![
                  MediaType::TypeScript,
                  MediaType::JavaScript,
                  MediaType::Tsx,
                  MediaType::Jsx,
                  MediaType::Mts,
                  MediaType::Mjs,
                ]
              },
              SloppyImportsResolutionReason::NoExtension,
            ),
          };
          let mut probe_paths = match path_without_ext(&path, media_type) {
            Some(path_no_ext) => media_types_to_paths(
              &path_no_ext,
              media_type,
              probe_media_type_types.0,
              probe_media_type_types.1,
            ),
            None => vec![],
          };

          if matches!(entry, Some(SloppyImportsFsEntry::Dir)) {
            // try to resolve at the index file
            if mode.is_types() {
              probe_paths.push((
                path.join("index.ts"),
                SloppyImportsResolutionReason::Directory,
              ));

              probe_paths.push((
                path.join("index.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.d.ts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.d.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.js"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mjs"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.tsx"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.jsx"),
                SloppyImportsResolutionReason::Directory,
              ));
            } else {
              probe_paths.push((
                path.join("index.ts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mts"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.tsx"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.js"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.mjs"),
                SloppyImportsResolutionReason::Directory,
              ));
              probe_paths.push((
                path.join("index.jsx"),
                SloppyImportsResolutionReason::Directory,
              ));
            }
          }
          if probe_paths.is_empty() {
            return None;
          }
          probe_paths
        }
      };

    for (probe_path, reason) in probe_paths {
      if self.stat_sync(&probe_path) == Some(SloppyImportsFsEntry::File) {
        if let Ok(specifier) = Url::from_file_path(probe_path) {
          match reason {
            SloppyImportsResolutionReason::JsToTs => {
              return Some(SloppyImportsResolution::JsToTs(specifier));
            }
            SloppyImportsResolutionReason::NoExtension => {
              return Some(SloppyImportsResolution::NoExtension(specifier));
            }
            SloppyImportsResolutionReason::Directory => {
              return Some(SloppyImportsResolution::Directory(specifier));
            }
          }
        }
      }
    }

    None
  }

  fn stat_sync(&self, path: &Path) -> Option<SloppyImportsFsEntry> {
    if let Some(cache) = &self.cache {
      if let Some(entry) = cache.get(path) {
        return *entry;
      }
    }

    let entry = self
      .fs
      .stat_sync(path)
      .ok()
      .and_then(|stat| SloppyImportsFsEntry::from_fs_stat(&stat));
    if let Some(cache) = &self.cache {
      cache.insert(path.to_owned(), entry);
    }
    entry
  }
}

#[cfg(test)]
mod test {
  use test_util::TestContext;

  use super::*;

  #[test]
  fn test_unstable_sloppy_imports() {
    fn resolve(specifier: &Url) -> Option<SloppyImportsResolution> {
      resolve_with_mode(specifier, SloppyImportsResolutionMode::Execution)
    }

    fn resolve_types(specifier: &Url) -> Option<SloppyImportsResolution> {
      resolve_with_mode(specifier, SloppyImportsResolutionMode::Types)
    }

    fn resolve_with_mode(
      specifier: &Url,
      mode: SloppyImportsResolutionMode,
    ) -> Option<SloppyImportsResolution> {
      SloppyImportsResolver::new(Arc::new(deno_fs::RealFs))
        .resolve(specifier, mode)
    }

    let context = TestContext::default();
    let temp_dir = context.temp_dir().path();

    // scenarios like resolving ./example.js to ./example.ts
    for (ext_from, ext_to) in [("js", "ts"), ("js", "tsx"), ("mjs", "mts")] {
      let ts_file = temp_dir.join(format!("file.{}", ext_to));
      ts_file.write("");
      assert_eq!(resolve(&ts_file.url_file()), None);
      assert_eq!(
        resolve(
          &temp_dir
            .url_dir()
            .join(&format!("file.{}", ext_from))
            .unwrap()
        ),
        Some(SloppyImportsResolution::JsToTs(ts_file.url_file())),
      );
      ts_file.remove_file();
    }

    // no extension scenarios
    for ext in ["js", "ts", "js", "tsx", "jsx", "mjs", "mts"] {
      let file = temp_dir.join(format!("file.{}", ext));
      file.write("");
      assert_eq!(
        resolve(
          &temp_dir
            .url_dir()
            .join("file") // no ext
            .unwrap()
        ),
        Some(SloppyImportsResolution::NoExtension(file.url_file()))
      );
      file.remove_file();
    }

    // .ts and .js exists, .js specified (goes to specified)
    {
      let ts_file = temp_dir.join("file.ts");
      ts_file.write("");
      let js_file = temp_dir.join("file.js");
      js_file.write("");
      assert_eq!(resolve(&js_file.url_file()), None);
    }

    // only js exists, .js specified
    {
      let js_only_file = temp_dir.join("js_only.js");
      js_only_file.write("");
      assert_eq!(resolve(&js_only_file.url_file()), None);
      assert_eq!(resolve_types(&js_only_file.url_file()), None);
    }

    // resolving a directory to an index file
    {
      let routes_dir = temp_dir.join("routes");
      routes_dir.create_dir_all();
      let index_file = routes_dir.join("index.ts");
      index_file.write("");
      assert_eq!(
        resolve(&routes_dir.url_file()),
        Some(SloppyImportsResolution::Directory(index_file.url_file())),
      );
    }

    // both a directory and a file with specifier is present
    {
      let api_dir = temp_dir.join("api");
      api_dir.create_dir_all();
      let bar_file = api_dir.join("bar.ts");
      bar_file.write("");
      let api_file = temp_dir.join("api.ts");
      api_file.write("");
      assert_eq!(
        resolve(&api_dir.url_file()),
        Some(SloppyImportsResolution::NoExtension(api_file.url_file())),
      );
    }
  }

  #[test]
  fn test_sloppy_import_resolution_suggestion_message() {
    // directory
    assert_eq!(
      SloppyImportsResolution::Directory(
        Url::parse("file:///dir/index.js").unwrap()
      )
      .as_suggestion_message(),
      "Maybe specify path to 'index.js' file in directory instead"
    );
    // no ext
    assert_eq!(
      SloppyImportsResolution::NoExtension(
        Url::parse("file:///dir/index.mjs").unwrap()
      )
      .as_suggestion_message(),
      "Maybe add a '.mjs' extension"
    );
    // js to ts
    assert_eq!(
      SloppyImportsResolution::JsToTs(
        Url::parse("file:///dir/index.mts").unwrap()
      )
      .as_suggestion_message(),
      "Maybe change the extension to '.mts'"
    );
  }
}
