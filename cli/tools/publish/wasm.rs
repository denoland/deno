// Copyright 2018-2026 the Deno authors. MIT license.

use deno_core::anyhow::Context;
use deno_core::anyhow::bail;
use deno_core::error::AnyError;

/// The Wasm import section has a section id of `2`.
const WASM_IMPORT_SECTION_ID: u8 = 0x02;

/// Rewrites the module specifiers found in the import section of a Wasm binary
/// using the provided `unfurl` callback.
///
/// A WebAssembly module's imports each reference a "module name" which Deno
/// treats as an ES module specifier (e.g. `import ... from "./other.js"`). When
/// publishing to JSR these specifiers need to be unfurled the same way they are
/// in JavaScript and TypeScript source files.
///
/// `unfurl` is called once per import with the import's module name and returns
/// the rewritten specifier, or `None` to leave it unchanged.
///
/// Only the import section is re-encoded; every other section is copied
/// verbatim so the rest of the binary is left byte-for-byte identical. If the
/// bytes are not a core Wasm module, or no specifier changed, the original
/// bytes are returned unchanged.
pub fn unfurl_wasm(
  bytes: &[u8],
  unfurl: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Vec<u8>, AnyError> {
  // Magic (`\0asm`) followed by the version. We only handle core modules
  // (version 1); anything else (e.g. the component model) is left untouched.
  if bytes.len() < 8
    || &bytes[0..4] != b"\0asm"
    || bytes[4..8] != [0x01, 0x00, 0x00, 0x00]
  {
    return Ok(bytes.to_vec());
  }

  let mut output = Vec::with_capacity(bytes.len());
  output.extend_from_slice(&bytes[0..8]);

  let mut changed = false;
  let mut offset = 8;
  while offset < bytes.len() {
    let section_start = offset;
    let id = bytes[offset];
    offset += 1;
    let (size, size_len) = read_var_u32(&bytes[offset..])
      .context("Failed to parse Wasm section length")?;
    offset += size_len;
    let body_start = offset;
    let body_end = body_start
      .checked_add(size as usize)
      .filter(|end| *end <= bytes.len())
      .context("Wasm section length out of bounds")?;

    if id == WASM_IMPORT_SECTION_ID {
      if let Some(new_section) = rewrite_import_section(
        &bytes[body_start..body_end],
        body_start,
        unfurl,
      )? {
        output.push(WASM_IMPORT_SECTION_ID);
        wasm_encoder::Encode::encode(&new_section, &mut output);
        changed = true;
      } else {
        output.extend_from_slice(&bytes[section_start..body_end]);
      }
    } else {
      output.extend_from_slice(&bytes[section_start..body_end]);
    }

    offset = body_end;
  }

  if changed {
    Ok(output)
  } else {
    Ok(bytes.to_vec())
  }
}

/// Re-encodes the import section body, rewriting module specifiers via
/// `unfurl`. Returns `None` if no specifier changed.
fn rewrite_import_section(
  body: &[u8],
  body_offset: usize,
  unfurl: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Option<wasm_encoder::ImportSection>, AnyError> {
  let reader = wasmparser::ImportSectionReader::new(
    wasmparser::BinaryReader::new(body, body_offset),
  )
  .context("Failed to parse Wasm import section")?;

  let mut section = wasm_encoder::ImportSection::new();
  let mut changed = false;
  for import in reader.into_imports() {
    let import = import.context("Failed to parse Wasm import")?;
    let unfurled = unfurl(import.module);
    let module = match &unfurled {
      Some(module) => {
        changed = true;
        module.as_str()
      }
      None => import.module,
    };
    let ty: wasm_encoder::EntityType = import.ty.try_into().map_err(|e| {
      deno_core::anyhow::anyhow!("Unsupported Wasm import type: {e}")
    })?;
    section.import(module, import.name, ty);
  }

  if changed { Ok(Some(section)) } else { Ok(None) }
}

/// Reads an unsigned LEB128 (variable length) `u32`, returning the value and
/// the number of bytes consumed.
fn read_var_u32(bytes: &[u8]) -> Result<(u32, usize), AnyError> {
  let mut result: u32 = 0;
  let mut shift = 0;
  for (i, byte) in bytes.iter().enumerate() {
    if shift >= 32 {
      bail!("LEB128 integer too large");
    }
    result |= ((byte & 0x7f) as u32) << shift;
    if byte & 0x80 == 0 {
      return Ok((result, i + 1));
    }
    shift += 7;
  }
  bail!("Unexpected end of LEB128 integer");
}

#[cfg(test)]
mod tests {
  use super::*;

  // (module
  //   (import "./foo.js" "bar" (func))
  //   (import "jsr:@scope/pkg" "baz" (global i32))
  // )
  fn build_wasm(module_a: &str, module_b: &str) -> Vec<u8> {
    let mut types = wasm_encoder::TypeSection::new();
    types.ty().function(vec![], vec![]);
    let mut imports = wasm_encoder::ImportSection::new();
    imports.import(module_a, "bar", wasm_encoder::EntityType::Function(0));
    imports.import(
      module_b,
      "baz",
      wasm_encoder::GlobalType {
        val_type: wasm_encoder::ValType::I32,
        mutable: false,
        shared: false,
      },
    );
    let mut module = wasm_encoder::Module::new();
    module.section(&types);
    module.section(&imports);
    module.finish()
  }

  fn imports_of(bytes: &[u8]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
      if let wasmparser::Payload::ImportSection(reader) = payload.unwrap() {
        for import in reader.into_imports() {
          let import = import.unwrap();
          found.push((import.module.to_string(), import.name.to_string()));
        }
      }
    }
    found
  }

  #[test]
  fn rewrites_import_specifiers() {
    let wasm = build_wasm("./foo.ts", "@scope/pkg");
    let output = unfurl_wasm(&wasm, &mut |specifier| match specifier {
      "./foo.ts" => Some("./foo.js".to_string()),
      "@scope/pkg" => Some("jsr:@scope/pkg@1".to_string()),
      _ => None,
    })
    .unwrap();

    assert_eq!(
      imports_of(&output),
      vec![
        ("./foo.js".to_string(), "bar".to_string()),
        ("jsr:@scope/pkg@1".to_string(), "baz".to_string()),
      ]
    );
    // still a valid module
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
      .validate_all(&output)
      .unwrap();
  }

  #[test]
  fn leaves_unchanged_when_nothing_unfurled() {
    let wasm = build_wasm("./foo.ts", "@scope/pkg");
    let output = unfurl_wasm(&wasm, &mut |_| None).unwrap();
    assert_eq!(output, wasm);
  }

  #[test]
  fn ignores_non_wasm_bytes() {
    let bytes = b"not a wasm module".to_vec();
    let output = unfurl_wasm(&bytes, &mut |_| Some("x".to_string())).unwrap();
    assert_eq!(output, bytes);
  }

  #[test]
  fn read_var_u32_multibyte() {
    assert_eq!(read_var_u32(&[0x00]).unwrap(), (0, 1));
    assert_eq!(read_var_u32(&[0x7f]).unwrap(), (127, 1));
    assert_eq!(read_var_u32(&[0x80, 0x01]).unwrap(), (128, 2));
    assert_eq!(read_var_u32(&[0xe5, 0x8e, 0x26]).unwrap(), (624485, 3));
  }
}
