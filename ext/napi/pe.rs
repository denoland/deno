// Copyright 2018-2026 the Deno authors. MIT license.

//! Detects Windows `.node` addons that link *directly* against the Node.js
//! binary (`node.exe`) via a regular import.
//!
//! Such addons depend on the V8 C++ ABI, Node internal APIs and/or libuv that
//! are exported by the `node.exe` executable — none of which Deno provides —
//! and they fail to load into any host that isn't literally named `node.exe`
//! with the opaque Windows loader error `LoadLibraryExW failed`.
//!
//! Delay-loaded imports are intentionally ignored: a delay-loaded `node.exe`
//! import is the node-gyp blessed pattern whose `win_delay_load_hook` redirects
//! symbol resolution to the host process at runtime, so it works fine in Deno.
//! `object`'s import table only covers the regular import directory, so that
//! distinction comes for free.

use object::FileKind;
use object::LittleEndian as LE;
use object::pe;
use object::read::pe::ImageNtHeaders;
use object::read::pe::PeFile;

/// Returns `true` if the PE image in `bytes` has a regular (non delay-load)
/// import of `node.exe`, meaning it expects the Node.js executable to satisfy
/// its symbols at load time. Returns `false` for anything that isn't a
/// well-formed PE image.
pub fn imports_node_executable(bytes: &[u8]) -> bool {
  imported_dll_names(bytes).is_some_and(|names| {
    names
      .iter()
      .any(|name| name.eq_ignore_ascii_case(b"node.exe"))
  })
}

/// Reads the names of the DLLs referenced by the regular import directory of a
/// PE image. Returns `None` if the bytes are not a well-formed PE image.
fn imported_dll_names(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
  match FileKind::parse(bytes).ok()? {
    FileKind::Pe32 => dll_names::<pe::ImageNtHeaders32>(bytes),
    FileKind::Pe64 => dll_names::<pe::ImageNtHeaders64>(bytes),
    _ => None,
  }
}

fn dll_names<Nt: ImageNtHeaders>(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
  let file = PeFile::<Nt>::parse(bytes).ok()?;
  let import_table = file.import_table().ok()??;
  let mut descriptors = import_table.descriptors().ok()?;
  let mut names = Vec::new();
  while let Some(descriptor) = descriptors.next().ok()? {
    if let Ok(name) = import_table.name(descriptor.name.get(LE)) {
      names.push(name.to_vec());
    }
  }
  Some(names)
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
    // NumberOfRvaAndSizes = 16 (so the import data directory is valid).
    buf[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());

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
    assert_eq!(
      names,
      vec![
        b"node.exe".to_vec(),
        b"KERNEL32.dll".to_vec(),
        b"WS2_32.dll".to_vec(),
      ]
    );
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
