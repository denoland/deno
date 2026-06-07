// Copyright 2018-2026 the Deno authors. MIT license.

use std::ops::Range;

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
      if let Some(new_section) =
        rewrite_import_section(&bytes[body_start..body_end], unfurl)?
      {
        output.push(WASM_IMPORT_SECTION_ID);
        let section_len = u32::try_from(new_section.len())
          .context("Wasm import section too large")?;
        write_var_u32(section_len, &mut output);
        output.extend_from_slice(&new_section);
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
  unfurl: &mut dyn FnMut(&str) -> Option<String>,
) -> Result<Option<Vec<u8>>, AnyError> {
  let mut records = Vec::new();
  let mut offset = 0;
  let group_count = read_var_u32_at(body, &mut offset)
    .context("Failed to parse Wasm import count")?;
  for _ in 0..group_count {
    collect_import_records(body, &mut offset, &mut records)?;
  }
  if offset != body.len() {
    bail!("Unexpected trailing bytes in Wasm import section");
  }

  let mut replacements = Vec::with_capacity(records.len());
  let mut changed = false;
  for record in &records {
    let unfurled = unfurl(record.module);
    changed |= unfurled.is_some();
    replacements.push(unfurled);
  }

  if !changed {
    return Ok(None);
  }

  let mut section = Vec::with_capacity(body.len());
  let import_count =
    u32::try_from(records.len()).context("Too many Wasm imports")?;
  write_var_u32(import_count, &mut section);
  for (record, replacement) in records.iter().zip(replacements.iter()) {
    write_wasm_string(
      replacement.as_deref().unwrap_or(record.module),
      &mut section,
    )?;
    section.extend_from_slice(&record.suffix);
  }

  Ok(Some(section))
}

struct ImportRecord<'a> {
  module: &'a str,
  suffix: Vec<u8>,
}

fn collect_import_records<'a>(
  body: &'a [u8],
  offset: &mut usize,
  records: &mut Vec<ImportRecord<'a>>,
) -> Result<(), AnyError> {
  let module = read_wasm_string(body, *offset)
    .context("Failed to parse Wasm import module name")?;
  *offset = module.range.end;
  let name = read_wasm_string(body, *offset)
    .context("Failed to parse Wasm import name")?;
  *offset = name.range.end;

  match (name.value, body.get(*offset).copied()) {
    ("", Some(0x7f)) => {
      *offset += 1;
      let item_count = read_var_u32_at(body, offset)
        .context("Failed to parse compact Wasm import item count")?;
      for _ in 0..item_count {
        let item_start = *offset;
        let _name = read_wasm_string(body, *offset)
          .context("Failed to parse compact Wasm import item name")?;
        *offset = _name.range.end;
        skip_type_ref(body, offset)
          .context("Failed to parse compact Wasm import item type")?;
        records.push(ImportRecord {
          module: module.value,
          suffix: body[item_start..*offset].to_vec(),
        });
      }
    }
    ("", Some(0x7e)) => {
      *offset += 1;
      let type_start = *offset;
      skip_type_ref(body, offset)
        .context("Failed to parse compact Wasm import type")?;
      let type_end = *offset;
      let name_count = read_var_u32_at(body, offset)
        .context("Failed to parse compact Wasm import name count")?;
      for _ in 0..name_count {
        let name_start = *offset;
        let _name = read_wasm_string(body, *offset)
          .context("Failed to parse compact Wasm import name")?;
        *offset = _name.range.end;
        let mut suffix =
          Vec::with_capacity((*offset - name_start) + (type_end - type_start));
        suffix.extend_from_slice(&body[name_start..*offset]);
        suffix.extend_from_slice(&body[type_start..type_end]);
        records.push(ImportRecord {
          module: module.value,
          suffix,
        });
      }
    }
    _ => {
      skip_type_ref(body, offset)
        .context("Failed to parse Wasm import type")?;
      records.push(ImportRecord {
        module: module.value,
        suffix: body[module.range.end..*offset].to_vec(),
      });
    }
  }
  Ok(())
}

fn read_byte(bytes: &[u8], offset: &mut usize) -> Result<u8, AnyError> {
  let byte = bytes
    .get(*offset)
    .copied()
    .context("Unexpected end of Wasm")?;
  *offset += 1;
  Ok(byte)
}

fn read_var_u32_at(bytes: &[u8], offset: &mut usize) -> Result<u32, AnyError> {
  let (value, len) = read_var_u32(&bytes[*offset..])?;
  *offset += len;
  Ok(value)
}

fn skip_var_u64(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  for i in 0..10 {
    let byte = read_byte(bytes, offset)?;
    if i == 9 && byte & 0xfe != 0 {
      bail!("LEB128 integer too large");
    }
    if byte & 0x80 == 0 {
      return Ok(());
    }
  }
  bail!("LEB128 integer too large");
}

fn skip_var_s33(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  for _ in 0..5 {
    let byte = read_byte(bytes, offset)?;
    if byte & 0x80 == 0 {
      return Ok(());
    }
  }
  bail!("LEB128 integer too large");
}

fn skip_type_ref(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  match read_byte(bytes, offset)? {
    0x00 | 0x20 => {
      read_var_u32_at(bytes, offset)?;
    }
    0x01 => skip_table_type(bytes, offset)?,
    0x02 => skip_memory_type(bytes, offset)?,
    0x03 => skip_global_type(bytes, offset)?,
    0x04 => skip_tag_type(bytes, offset)?,
    kind => bail!("Invalid Wasm import kind 0x{kind:02x}"),
  }
  Ok(())
}

fn skip_table_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  skip_ref_type(bytes, offset)?;
  let flags = read_byte(bytes, offset)?;
  if flags & !0b111 != 0 {
    bail!("Invalid Wasm table limits flags");
  }
  skip_limit_bounds(bytes, offset, flags & 0b001 != 0, flags & 0b100 != 0)
}

fn skip_memory_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  let flags = read_byte(bytes, offset)?;
  if flags & !0b1111 != 0 {
    bail!("Invalid Wasm memory limits flags");
  }
  skip_limit_bounds(bytes, offset, flags & 0b0001 != 0, flags & 0b0100 != 0)?;
  if flags & 0b1000 != 0 {
    read_var_u32_at(bytes, offset)?;
  }
  Ok(())
}

fn skip_limit_bounds(
  bytes: &[u8],
  offset: &mut usize,
  has_max: bool,
  is_64: bool,
) -> Result<(), AnyError> {
  if is_64 {
    skip_var_u64(bytes, offset)?;
    if has_max {
      skip_var_u64(bytes, offset)?;
    }
  } else {
    read_var_u32_at(bytes, offset)?;
    if has_max {
      read_var_u32_at(bytes, offset)?;
    }
  }
  Ok(())
}

fn skip_global_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  skip_val_type(bytes, offset)?;
  let flags = read_byte(bytes, offset)?;
  if flags > 0b11 {
    bail!("Invalid Wasm global flags");
  }
  Ok(())
}

fn skip_tag_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  let attribute = read_byte(bytes, offset)?;
  if attribute != 0 {
    bail!("Invalid Wasm tag attribute");
  }
  read_var_u32_at(bytes, offset)?;
  Ok(())
}

fn skip_val_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  match bytes
    .get(*offset)
    .copied()
    .context("Unexpected end of Wasm")?
  {
    0x7b..=0x7f => {
      *offset += 1;
      Ok(())
    }
    _ => skip_ref_type(bytes, offset),
  }
}

fn skip_ref_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  match bytes
    .get(*offset)
    .copied()
    .context("Unexpected end of Wasm")?
  {
    0x63 | 0x64 => {
      *offset += 1;
      skip_heap_type(bytes, offset)
    }
    _ => skip_heap_type(bytes, offset),
  }
}

fn skip_heap_type(bytes: &[u8], offset: &mut usize) -> Result<(), AnyError> {
  match bytes
    .get(*offset)
    .copied()
    .context("Unexpected end of Wasm")?
  {
    0x65 => {
      *offset += 1;
      skip_abstract_heap_type(bytes, offset)
    }
    0x62 => {
      *offset += 1;
      read_var_u32_at(bytes, offset)?;
      Ok(())
    }
    _ => skip_var_s33(bytes, offset),
  }
}

fn skip_abstract_heap_type(
  bytes: &[u8],
  offset: &mut usize,
) -> Result<(), AnyError> {
  match read_byte(bytes, offset)? {
    0x68..=0x75 => Ok(()),
    ty => bail!("Invalid Wasm abstract heap type 0x{ty:02x}"),
  }
}

/// Reads an unsigned LEB128 (variable length) `u32`, returning the value and
/// the number of bytes consumed.
fn read_var_u32(bytes: &[u8]) -> Result<(u32, usize), AnyError> {
  let mut result: u32 = 0;
  let mut shift = 0;
  for (i, byte) in bytes.iter().enumerate() {
    if i >= 5 || (i == 4 && byte & 0xf0 != 0) {
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

fn write_var_u32(mut value: u32, output: &mut Vec<u8>) {
  loop {
    let mut byte = (value & 0x7f) as u8;
    value >>= 7;
    if value != 0 {
      byte |= 0x80;
    }
    output.push(byte);
    if value == 0 {
      break;
    }
  }
}

struct WasmString<'a> {
  value: &'a str,
  range: Range<usize>,
}

fn read_wasm_string(
  bytes: &[u8],
  offset: usize,
) -> Result<WasmString<'_>, AnyError> {
  if offset > bytes.len() {
    bail!("Wasm string offset out of bounds");
  }
  let (len, len_len) = read_var_u32(&bytes[offset..])?;
  let string_start = offset + len_len;
  let string_end = string_start
    .checked_add(len as usize)
    .filter(|end| *end <= bytes.len())
    .context("Wasm string length out of bounds")?;
  let value = std::str::from_utf8(&bytes[string_start..string_end])
    .context("Wasm string is not valid UTF-8")?;
  Ok(WasmString {
    value,
    range: offset..string_end,
  })
}

fn write_wasm_string(
  value: &str,
  output: &mut Vec<u8>,
) -> Result<(), AnyError> {
  let len = u32::try_from(value.len()).context("Wasm string too long")?;
  write_var_u32(len, output);
  output.extend_from_slice(value.as_bytes());
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  fn section(id: u8, body: &[u8], output: &mut Vec<u8>) {
    output.push(id);
    write_var_u32(body.len() as u32, output);
    output.extend_from_slice(body);
  }

  fn test_wasm_string(value: &str, output: &mut Vec<u8>) {
    write_wasm_string(value, output).unwrap();
  }

  // (module
  //   (import "./foo.js" "bar" (func))
  //   (import "jsr:@scope/pkg" "baz" (global i32))
  // )
  fn build_wasm(module_a: &str, module_b: &str) -> Vec<u8> {
    let mut wasm = b"\0asm\x01\0\0\0".to_vec();

    let types = [0x01, 0x60, 0x00, 0x00];
    section(1, &types, &mut wasm);

    let mut imports = Vec::new();
    write_var_u32(2, &mut imports);
    test_wasm_string(module_a, &mut imports);
    test_wasm_string("bar", &mut imports);
    imports.extend_from_slice(&[0x00, 0x00]);
    test_wasm_string(module_b, &mut imports);
    test_wasm_string("baz", &mut imports);
    imports.extend_from_slice(&[0x03, 0x7f, 0x00]);
    section(WASM_IMPORT_SECTION_ID, &imports, &mut wasm);

    wasm
  }

  // (module
  //   ;; Compact1: same module, different import names/types.
  //   (import "@scope/pkg" "a" (func))
  //   (import "@scope/pkg" "b" (global i32))
  //   ;; Compact2: same module and type, different import names.
  //   (import "chalk" "c" (func))
  //   (import "chalk" "d" (func))
  // )
  fn build_compact_wasm() -> Vec<u8> {
    let mut wasm = b"\0asm\x01\0\0\0".to_vec();

    let types = [0x01, 0x60, 0x00, 0x00];
    section(1, &types, &mut wasm);

    let mut imports = Vec::new();
    write_var_u32(2, &mut imports);

    test_wasm_string("@scope/pkg", &mut imports);
    test_wasm_string("", &mut imports);
    imports.push(0x7f);
    write_var_u32(2, &mut imports);
    test_wasm_string("a", &mut imports);
    imports.extend_from_slice(&[0x00, 0x00]);
    test_wasm_string("b", &mut imports);
    imports.extend_from_slice(&[0x03, 0x7f, 0x00]);

    test_wasm_string("chalk", &mut imports);
    test_wasm_string("", &mut imports);
    imports.push(0x7e);
    imports.extend_from_slice(&[0x00, 0x00]);
    write_var_u32(2, &mut imports);
    test_wasm_string("c", &mut imports);
    test_wasm_string("d", &mut imports);

    section(WASM_IMPORT_SECTION_ID, &imports, &mut wasm);

    wasm
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
  fn rewrites_compact_import_specifiers() {
    let wasm = build_compact_wasm();
    let output = unfurl_wasm(&wasm, &mut |specifier| match specifier {
      "@scope/pkg" => Some("jsr:@scope/pkg@1".to_string()),
      "chalk" => Some("npm:chalk@5".to_string()),
      _ => None,
    })
    .unwrap();

    assert_eq!(
      imports_of(&output),
      vec![
        ("jsr:@scope/pkg@1".to_string(), "a".to_string()),
        ("jsr:@scope/pkg@1".to_string(), "b".to_string()),
        ("npm:chalk@5".to_string(), "c".to_string()),
        ("npm:chalk@5".to_string(), "d".to_string()),
      ]
    );
    wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all())
      .validate_all(&output)
      .unwrap();
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
    assert_eq!(
      read_var_u32(&[0xff, 0xff, 0xff, 0xff, 0x0f]).unwrap(),
      (u32::MAX, 5)
    );
    assert!(read_var_u32(&[0xff, 0xff, 0xff, 0xff, 0x10]).is_err());
    assert!(read_var_u32(&[0xff, 0xff, 0xff, 0xff, 0x8f]).is_err());
  }
}
