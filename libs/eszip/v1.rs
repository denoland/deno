// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;

use serde::Deserialize;
use serde::Serialize;
use url::Url;

use crate::Module;
use crate::ModuleInner;
use crate::ModuleKind;
use crate::ParseError;

const ESZIP_V1_GRAPH_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EszipV1 {
  version: u32,
  modules: Arc<Mutex<HashMap<Url, ModuleInfo>>>,
}

impl EszipV1 {
  pub fn from_modules(modules: HashMap<Url, ModuleInfo>) -> Self {
    Self {
      version: ESZIP_V1_GRAPH_VERSION,
      modules: Arc::new(Mutex::new(modules)),
    }
  }

  pub fn parse(data: &[u8]) -> Result<EszipV1, ParseError> {
    let eszip: EszipV1 =
      serde_json::from_slice(data).map_err(ParseError::InvalidV1Json)?;
    if eszip.version != ESZIP_V1_GRAPH_VERSION {
      return Err(ParseError::InvalidV1Version(eszip.version));
    }
    Ok(eszip)
  }

  pub fn into_bytes(self) -> Vec<u8> {
    serde_json::to_vec(&self).unwrap()
  }

  pub fn get_module(&self, specifier: &str) -> Option<Module> {
    let mut specifier = &Url::parse(specifier).ok()?;
    let mut visited = HashSet::new();
    let modules = self.modules.lock().unwrap();
    loop {
      visited.insert(specifier);
      let module = modules.get(specifier)?;
      match module {
        ModuleInfo::Redirect(redirect) => {
          specifier = redirect;
          if visited.contains(specifier) {
            return None;
          }
        }
        ModuleInfo::Source(..) => {
          let module = Module {
            specifier: specifier.to_string(),
            kind: ModuleKind::JavaScript,
            inner: ModuleInner::V1(EszipV1 {
              version: self.version,
              modules: self.modules.clone(),
            }),
          };
          return Some(module);
        }
      }
    }
  }

  pub fn get_import_map(&self, _specifier: &str) -> Option<Module> {
    // V1 never contains an import map in it. This method exists to make it
    // consistent with V2's interface.
    None
  }

  /// Get source code of the module.
  pub(crate) fn get_module_source(&self, specifier: &str) -> Option<Arc<[u8]>> {
    let specifier = &Url::parse(specifier).ok()?;
    let modules = self.modules.lock().unwrap();
    let module = modules.get(specifier).unwrap();
    match module {
      ModuleInfo::Redirect(_) => panic!("Redirects should be resolved"),
      ModuleInfo::Source(module) => {
        let source = module.transpiled.as_ref().unwrap_or(&module.source);
        Some(source.clone().into())
      }
    }
  }

  /// Removes the module from the modules map and returns the source code.
  pub(crate) fn take(&self, specifier: &str) -> Option<Arc<[u8]>> {
    let specifier = &Url::parse(specifier).ok()?;
    let mut modules = self.modules.lock().unwrap();
    // Note: we don't have a need to preserve the module in the map for v1, so we can
    // remove the module from the map. In v2, we need to preserve the module in the map
    // to be able to get source map for the module.
    let module = modules.remove(specifier)?;
    match module {
      ModuleInfo::Redirect(_) => panic!("Redirects should be resolved"),
      ModuleInfo::Source(module_source) => {
        let source = module_source.transpiled.unwrap_or(module_source.source);
        Some(source.into())
      }
    }
  }

  fn specifiers(&self) -> Vec<Url> {
    let modules = self.modules.lock().unwrap();
    modules.keys().cloned().collect()
  }
}

/// Get an iterator over all the modules in this eszip archive.
///
/// Note that the iterator will iterate over the specifiers' "snapshot" of the
/// archive. If a new module is added to the archive after the iterator is
/// created via `into_iter()`, that module will not be iterated over.
impl IntoIterator for EszipV1 {
  type Item = (String, Module);
  type IntoIter = std::vec::IntoIter<Self::Item>;

  fn into_iter(self) -> Self::IntoIter {
    let specifiers = self.specifiers();
    let mut v = Vec::with_capacity(specifiers.len());
    for specifier in specifiers {
      let Some(module) = self.get_module(specifier.as_str()) else {
        continue;
      };
      v.push((specifier.to_string(), module));
    }

    v.into_iter()
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ModuleInfo {
  Redirect(Url),
  Source(ModuleSource),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleSource {
  pub source: Arc<str>,
  pub transpiled: Option<Arc<str>>,
  pub content_type: Option<String>,
  pub deps: Vec<Url>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use pretty_assertions::assert_eq;

  #[test]
  fn file_format_parse() {
    let data = include_bytes!("./testdata/basic.json");
    let eszip = EszipV1::parse(data).unwrap();
    assert_eq!(eszip.version, 1);
    assert_eq!(eszip.modules.lock().unwrap().len(), 1);
    let specifier = "https://gist.githubusercontent.com/lucacasonato/f3e21405322259ca4ed155722390fda2/raw/e25acb49b681e8e1da5a2a33744b7a36d538712d/hello.js";
    let module = eszip.get_module(specifier).unwrap();
    assert_eq!(module.specifier, specifier);
    let inner = module.inner;
    let bytes = match inner {
      crate::ModuleInner::V1(eszip) => {
        eszip.get_module_source(specifier).unwrap()
      }
      crate::ModuleInner::V2(_) => unreachable!(),
    };
    assert_eq!(&*bytes, b"addEventListener(\"fetch\", (event)=>{\n    event.respondWith(new Response(\"Hello World\", {\n        headers: {\n            \"content-type\": \"text/plain\"\n        }\n    }));\n});\n//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbIjxodHRwczovL2dpc3QuZ2l0aHVidXNlcmNvbnRlbnQuY29tL2x1Y2FjYXNvbmF0by9mM2UyMTQwNTMyMjI1OWNhNGVkMTU1NzIyMzkwZmRhMi9yYXcvZTI1YWNiNDliNjgxZThlMWRhNWEyYTMzNzQ0YjdhMzZkNTM4NzEyZC9oZWxsby5qcz4iXSwic291cmNlc0NvbnRlbnQiOlsiYWRkRXZlbnRMaXN0ZW5lcihcImZldGNoXCIsIChldmVudCkgPT4ge1xuICBldmVudC5yZXNwb25kV2l0aChuZXcgUmVzcG9uc2UoXCJIZWxsbyBXb3JsZFwiLCB7XG4gICAgaGVhZGVyczogeyBcImNvbnRlbnQtdHlwZVwiOiBcInRleHQvcGxhaW5cIiB9LFxuICB9KSk7XG59KTsiXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IkFBQUEsZ0JBQUEsRUFBQSxLQUFBLElBQUEsS0FBQTtBQUNBLFNBQUEsQ0FBQSxXQUFBLEtBQUEsUUFBQSxFQUFBLFdBQUE7QUFDQSxlQUFBO2FBQUEsWUFBQSxJQUFBLFVBQUEifQ==");
  }

  #[tokio::test]
  async fn get_transpiled_for_ts() {
    let data = include_bytes!("./testdata/dotland.json");
    let eszip = EszipV1::parse(data).unwrap();
    assert_eq!(eszip.version, 1);

    let module = eszip.get_module("file:///src/worker/handler.ts").unwrap();
    assert_eq!(module.specifier, "file:///src/worker/handler.ts");
    let bytes = module.source().await.unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    assert!(!text.contains("import type { ConnInfo }"));
  }

  #[tokio::test]
  async fn eszipv1_iterator_yields_all_modules() {
    let data = include_bytes!("./testdata/dotland.json");
    let eszip = EszipV1::parse(data).unwrap();
    assert_eq!(eszip.version, 1);

    let expected_modules: HashSet<String> = [
      "file:///src/util/registry_utils.ts".to_string(),
      "file:///src/worker/handler.ts".to_string(),
      "file:///src/worker/main.ts".to_string(),
      "file:///src/worker/registry.ts".to_string(),
      "file:///src/worker/registry_config.ts".to_string(),
      "file:///src/worker/suggestions.ts".to_string(),
      "file:///src/worker/vscode.ts".to_string(),
      "https://cdn.esm.sh/v64/twas@2.1.2/deno/twas.js".to_string(),
      "https://deno.land/std@0.108.0/async/deadline.ts".to_string(),
      "https://deno.land/std@0.108.0/async/debounce.ts".to_string(),
      "https://deno.land/std@0.108.0/async/deferred.ts".to_string(),
      "https://deno.land/std@0.108.0/async/delay.ts".to_string(),
      "https://deno.land/std@0.108.0/async/mod.ts".to_string(),
      "https://deno.land/std@0.108.0/async/mux_async_iterator.ts".to_string(),
      "https://deno.land/std@0.108.0/async/pool.ts".to_string(),
      "https://deno.land/std@0.108.0/async/tee.ts".to_string(),
      "https://deno.land/std@0.108.0/http/server.ts".to_string(),
      "https://deno.land/std@0.120.0/async/deadline.ts".to_string(),
      "https://deno.land/std@0.120.0/async/debounce.ts".to_string(),
      "https://deno.land/std@0.120.0/async/deferred.ts".to_string(),
      "https://deno.land/std@0.120.0/async/delay.ts".to_string(),
      "https://deno.land/std@0.120.0/async/mod.ts".to_string(),
      "https://deno.land/std@0.120.0/async/mux_async_iterator.ts".to_string(),
      "https://deno.land/std@0.120.0/async/pool.ts".to_string(),
      "https://deno.land/std@0.120.0/async/tee.ts".to_string(),
      "https://deno.land/std@0.120.0/fmt/colors.ts".to_string(),
      "https://deno.land/std@0.120.0/http/http_status.ts".to_string(),
      "https://deno.land/x/fuse@v6.4.1/dist/fuse.esm.js".to_string(),
      "https://deno.land/x/g_a@0.1.2/mod.ts".to_string(),
      "https://deno.land/x/oak_commons@0.1.1/negotiation.ts".to_string(),
      "https://deno.land/x/oak_commons@0.1.1/negotiation/common.ts".to_string(),
      "https://deno.land/x/oak_commons@0.1.1/negotiation/encoding.ts"
        .to_string(),
      "https://deno.land/x/oak_commons@0.1.1/negotiation/language.ts"
        .to_string(),
      "https://deno.land/x/oak_commons@0.1.1/negotiation/mediaType.ts"
        .to_string(),
      "https://deno.land/x/path_to_regexp@v6.2.0/index.ts".to_string(),
      "https://deno.land/x/pretty_bytes@v1.0.5/mod.ts".to_string(),
      "https://esm.sh/twas@2.1.2".to_string(),
    ]
    .into_iter()
    .collect();
    let actual_modules = eszip
      .into_iter()
      .map(|(module_specifier, _)| module_specifier)
      .collect();

    assert_eq!(expected_modules, actual_modules);
  }
}
