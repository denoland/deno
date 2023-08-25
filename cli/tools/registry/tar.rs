// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

use deno_ast::ParsedSource;
use deno_core::error::AnyError;
use deno_core::url::Url;
use deno_graph::DefaultModuleAnalyzer;
use deno_graph::MediaType;
use deno_graph::TypeScriptReference;
use hyper::body::Bytes;
use import_map::parse_from_json;
use import_map::ImportMap;
use import_map::ImportMapWithDiagnostics;
use std::io::Write;
use std::path::PathBuf;
use tar::Header;

pub struct Unfurler {
  import_map: ImportMap,
}

impl Unfurler {
  pub fn new(
    import_map_base: Url,
    import_map_text: String,
  ) -> Result<Self, AnyError> {
    let ImportMapWithDiagnostics {
      diagnostics: _,
      import_map,
    } = parse_from_json(&import_map_base, &import_map_text)?;
    // todo: surface diagnostics?
    Ok(Self { import_map })
  }

  fn unfurl(
    &self,
    specifier: String,
    data: Vec<u8>,
  ) -> Result<Vec<u8>, AnyError> {
    let url = Url::parse(&specifier)?;
    let media_type = MediaType::from_specifier(&url);
    match media_type {
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
        // continue
      }
      MediaType::SourceMap
      | MediaType::Unknown
      | MediaType::Json
      | MediaType::Wasm
      | MediaType::TsBuildInfo => {
        // not unfurlable data
        return Ok(data);
      }
    }

    let text = String::from_utf8(data)?;
    let parsed_source = deno_ast::parse_module(deno_ast::ParseParams {
      specifier,
      text_info: deno_ast::SourceTextInfo::from_string(text),
      media_type,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })?;
    let mut text_changes = Vec::new();
    let module_info = DefaultModuleAnalyzer::module_info(&parsed_source);
    let mut analyze_specifier =
      |specifier: &str, range: &deno_graph::PositionRange| {
        let resolved = self.import_map.resolve(specifier, &url);
        if let Ok(resolved) = resolved {
          let new_text = if resolved.scheme() == "file" {
            format!("./{}", url.make_relative(&resolved).unwrap())
          } else {
            resolved.to_string()
          };
          text_changes.push(deno_ast::TextChange {
            range: to_range(&parsed_source, range),
            new_text,
          });
        }
      };
    for dep in &module_info.dependencies {
      analyze_specifier(&dep.specifier, &dep.specifier_range);
    }
    for ts_ref in &module_info.ts_references {
      let specifier_with_range = match ts_ref {
        TypeScriptReference::Path(range) => range,
        TypeScriptReference::Types(range) => range,
      };
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
      );
    }
    for specifier_with_range in &module_info.jsdoc_imports {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
      );
    }
    if let Some(specifier_with_range) = &module_info.jsx_import_source {
      analyze_specifier(
        &specifier_with_range.text,
        &specifier_with_range.range,
      );
    }
    Ok(
      deno_ast::apply_text_changes(
        parsed_source.text_info().text_str(),
        text_changes,
      )
      .into_bytes(),
    )
  }
}

fn to_range(
  parsed_source: &ParsedSource,
  range: &deno_graph::PositionRange,
) -> std::ops::Range<usize> {
  let mut range = range
    .as_source_range(parsed_source.text_info())
    .as_byte_range(parsed_source.text_info().range().start);
  let text = &parsed_source.text_info().text_str()[range.clone()];
  if text.starts_with('"') || text.starts_with('\'') {
    range.start += 1;
  }
  if text.ends_with('"') || text.ends_with('\'') {
    range.end -= 1;
  }
  range
}

pub fn create_tarball(
  dir: PathBuf,
  unfurler: Unfurler,
) -> Result<Bytes, AnyError> {
  let mut tar = Tar::new();
  let dir_url = Url::from_directory_path(&dir).unwrap();

  // TODO(bartlomieju): this should be helper function and it should also
  // exclude test/bench files when publishing.
  for file in walkdir::WalkDir::new(dir).follow_links(false) {
    let file = file?;

    if file.file_type().is_dir() {
      continue;
    }

    let path = file.path();

    let url = Url::from_file_path(path).unwrap();
    // TODO(bartlomieju): use the same functionality as in `deno test`/
    // `deno bench` to match these
    if url.as_str().contains("_test") || url.as_str().contains("_bench") {
      continue;
    }

    let relative_path = dir_url.make_relative(&url).unwrap();
    let data = std::fs::read(path)?;
    let content = unfurler.unfurl(url.to_string(), data)?;
    tar.add_file(relative_path, &content)?;
  }

  let v = tar.finish()?;
  Ok(Bytes::from(v))
}

struct Tar {
  builder: tar::Builder<Vec<u8>>,
}

impl Tar {
  pub fn new() -> Tar {
    Self {
      builder: tar::Builder::new(Vec::new()),
    }
  }

  pub fn add_file(
    &mut self,
    path: String,
    data: &[u8],
  ) -> Result<(), AnyError> {
    let mut header = Header::new_gnu();
    header.set_size(data.len() as u64);
    self.builder.append_data(&mut header, &path, data)?;
    Ok(())
  }

  fn finish(mut self) -> Result<Vec<u8>, AnyError> {
    self.builder.finish()?;
    let bytes = self.builder.into_inner()?;
    let mut gz_bytes = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(
      &mut gz_bytes,
      flate2::Compression::default(),
    );
    encoder.write_all(&bytes)?;
    encoder.finish()?;
    Ok(gz_bytes)
  }
}
