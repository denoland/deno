// Copyright 2018-2026 the Deno authors. MIT license.

//! Minimal, dependency-free PE (Portable Executable) import-table reader.
//!
//! This exists to give a clear diagnostic when a Windows `.node` addon links
//! *directly* against the Node.js binary (`node.exe`) via a regular import.
//! Such addons depend on the V8 C++ ABI, Node internal APIs and/or libuv that
//! are exported by the `node.exe` executable — none of which Deno provides —
//! and they fail to load into any host that isn't literally named `node.exe`
//! with the opaque Windows loader error `LoadLibraryExW failed`.
//!
//! Delay-loaded imports are intentionally ignored: a delay-loaded `node.exe`
//! import is the node-gyp blessed pattern whose `win_delay_load_hook` redirects
//! symbol resolution to the host process at runtime, so it works fine in Deno.
//!
//! The parser is fully bounds-checked and never panics on malformed input.

/// Returns `true` if the PE image in `bytes` has a *regular* (non delay-load)
/// import of `node.exe`, meaning it expects the Node.js executable to satisfy
/// its symbols at load time. Returns `false` for anything that isn't a
/// well-formed PE image.
pub fn imports_node_executable(bytes: &[u8]) -> bool {
  imported_dll_names(bytes)
    .map(|names| names.iter().any(|n| n.eq_ignore_ascii_case("node.exe")))
    .unwrap_or(false)
}

/// Reads the names of the DLLs referenced by the regular import directory of a
/// PE image. Returns `None` if the bytes are not a well-formed PE image.
pub fn imported_dll_names(bytes: &[u8]) -> Option<Vec<String>> {
  // DOS header: must start with "MZ"; e_lfanew (offset of the PE header) lives
  // at offset 0x3C.
  if bytes.get(0..2)? != b"MZ" {
    return None;
  }
  let pe = read_u32(bytes, 0x3C)? as usize;
  if bytes.get(pe..pe + 4)? != b"PE\0\0" {
    return None;
  }

  // COFF file header immediately follows the PE signature.
  let coff = pe + 4;
  let num_sections = read_u16(bytes, coff + 2)? as usize;
  let size_opt_header = read_u16(bytes, coff + 16)? as usize;

  // Optional header. Its magic selects PE32 vs PE32+, which changes where the
  // data directory array begins.
  let opt = coff + 20;
  let data_dir = match read_u16(bytes, opt)? {
    0x10b => opt + 96,  // PE32
    0x20b => opt + 112, // PE32+
    _ => return None,
  };

  // Data directory entry 1 is the import directory (8 bytes: RVA, size).
  let import_rva = read_u32(bytes, data_dir + 8)? as usize;
  if import_rva == 0 {
    return Some(Vec::new());
  }

  // Section headers follow the optional header and are needed to translate
  // relative virtual addresses (RVAs) into file offsets.
  let sections = parse_sections(bytes, opt + size_opt_header, num_sections)?;

  let import_off = rva_to_offset(&sections, import_rva)?;
  let mut names = Vec::new();
  // Walk the array of 20-byte import descriptors. The array is terminated by an
  // all-zero descriptor. The upper bound is a safety guard against malformed or
  // cyclic data.
  for i in 0..4096usize {
    let desc = import_off.checked_add(i.checked_mul(20)?)?;
    let original_first_thunk = read_u32(bytes, desc)?;
    let name_rva = read_u32(bytes, desc + 12)?;
    let first_thunk = read_u32(bytes, desc + 16)?;
    if original_first_thunk == 0 && name_rva == 0 && first_thunk == 0 {
      break;
    }
    if name_rva != 0
      && let Some(off) = rva_to_offset(&sections, name_rva as usize)
      && let Some(name) = read_cstr(bytes, off)
    {
      names.push(name);
    }
  }
  Some(names)
}

struct Section {
  virtual_address: u32,
  virtual_size: u32,
  size_raw: u32,
  ptr_raw: u32,
}

fn parse_sections(
  bytes: &[u8],
  off: usize,
  count: usize,
) -> Option<Vec<Section>> {
  let mut sections = Vec::with_capacity(count);
  for i in 0..count {
    let s = off.checked_add(i.checked_mul(40)?)?;
    sections.push(Section {
      virtual_size: read_u32(bytes, s + 8)?,
      virtual_address: read_u32(bytes, s + 12)?,
      size_raw: read_u32(bytes, s + 16)?,
      ptr_raw: read_u32(bytes, s + 20)?,
    });
  }
  Some(sections)
}

fn rva_to_offset(sections: &[Section], rva: usize) -> Option<usize> {
  for s in sections {
    let va = s.virtual_address as usize;
    // Be lenient: use whichever of the virtual/raw sizes is larger so we can
    // still resolve names that sit in the slack of a section.
    let size = s.virtual_size.max(s.size_raw) as usize;
    if rva >= va && rva < va.checked_add(size)? {
      return (rva - va).checked_add(s.ptr_raw as usize);
    }
  }
  None
}

fn read_cstr(bytes: &[u8], off: usize) -> Option<String> {
  let slice = bytes.get(off..)?;
  let end = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
  Some(String::from_utf8_lossy(&slice[..end]).into_owned())
}

fn read_u16(bytes: &[u8], off: usize) -> Option<u16> {
  let b = bytes.get(off..off + 2)?;
  Some(u16::from_le_bytes([b[0], b[1]]))
}

fn read_u32(bytes: &[u8], off: usize) -> Option<u32> {
  let b = bytes.get(off..off + 4)?;
  Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Builds a minimal but well-formed PE32+ image whose import directory
  /// references each of `dlls`. A single section maps RVA == file offset
  /// (both based at 0x1000) to keep the fixture simple.
  fn build_pe(dlls: &[&str]) -> Vec<u8> {
    let mut buf = vec![0u8; 0x2000];
    // DOS header.
    buf[0..2].copy_from_slice(b"MZ");
    let e_lfanew: u32 = 0x80;
    buf[0x3C..0x40].copy_from_slice(&e_lfanew.to_le_bytes());

    let pe = e_lfanew as usize;
    buf[pe..pe + 4].copy_from_slice(b"PE\0\0");

    let coff = pe + 4;
    // Machine = IMAGE_FILE_MACHINE_AMD64.
    buf[coff..coff + 2].copy_from_slice(&0x8664u16.to_le_bytes());
    // NumberOfSections = 1.
    buf[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
    // SizeOfOptionalHeader (PE32+ standard/windows fields + 16 data dirs).
    let size_opt: u16 = 0xF0;
    buf[coff + 16..coff + 18].copy_from_slice(&size_opt.to_le_bytes());

    let opt = coff + 20;
    // Magic = PE32+.
    buf[opt..opt + 2].copy_from_slice(&0x20bu16.to_le_bytes());

    let data_dir = opt + 112;
    let import_rva: u32 = 0x1000;
    let descriptors_size = ((dlls.len() + 1) * 20) as u32;
    buf[data_dir + 8..data_dir + 12].copy_from_slice(&import_rva.to_le_bytes());
    buf[data_dir + 12..data_dir + 16]
      .copy_from_slice(&descriptors_size.to_le_bytes());

    // Single section: VirtualAddress == PointerToRawData == 0x1000.
    let sec = opt + size_opt as usize;
    buf[sec..sec + 6].copy_from_slice(b".idata");
    buf[sec + 8..sec + 12].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 12..sec + 16].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 16..sec + 20].copy_from_slice(&0x1000u32.to_le_bytes());
    buf[sec + 20..sec + 24].copy_from_slice(&0x1000u32.to_le_bytes());

    // Import descriptors followed by the null-terminated name strings.
    let descriptors_off = 0x1000usize;
    let mut names_off = descriptors_off + (dlls.len() + 1) * 20;
    for (i, dll) in dlls.iter().enumerate() {
      let desc = descriptors_off + i * 20;
      buf[desc + 12..desc + 16]
        .copy_from_slice(&(names_off as u32).to_le_bytes());
      buf[names_off..names_off + dll.len()].copy_from_slice(dll.as_bytes());
      names_off += dll.len() + 1; // keep the trailing NUL (already zeroed)
    }
    buf
  }

  #[test]
  fn detects_regular_node_exe_import() {
    let pe = build_pe(&["node.exe", "KERNEL32.dll", "WS2_32.dll"]);
    assert!(imports_node_executable(&pe));
    let names = imported_dll_names(&pe).unwrap();
    assert_eq!(names, vec!["node.exe", "KERNEL32.dll", "WS2_32.dll"]);
  }

  #[test]
  fn node_exe_match_is_case_insensitive() {
    assert!(imports_node_executable(&build_pe(&["NODE.EXE"])));
    assert!(imports_node_executable(&build_pe(&["Node.Exe"])));
  }

  #[test]
  fn ignores_addons_without_node_exe_import() {
    let pe = build_pe(&["KERNEL32.dll", "WS2_32.dll", "VCRUNTIME140.dll"]);
    assert!(!imports_node_executable(&pe));
  }

  #[test]
  fn malformed_input_is_not_flagged() {
    assert!(!imports_node_executable(b""));
    assert!(!imports_node_executable(b"MZ"));
    assert!(!imports_node_executable(&[0u8; 256]));
    assert!(!imports_node_executable(&[0xFFu8; 4096]));
    assert!(imported_dll_names(b"not a pe file").is_none());
  }
}
