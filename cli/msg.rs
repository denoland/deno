// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Warning! The values in this enum are duplicated in js/compiler.ts
// Update carefully!
use serde::Serialize;

#[allow(non_camel_case_types)]
#[repr(i8)]
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  TSX = 3,
  Json = 4,
  Wasm = 5,
  Unknown = 6,
}

pub fn enum_name_media_type(mt: MediaType) -> &'static str {
  match mt {
    MediaType::JavaScript => "JavaScript",
    MediaType::JSX => "JSX",
    MediaType::TypeScript => "TypeScript",
    MediaType::TSX => "TSX",
    MediaType::Json => "Json",
    MediaType::Wasm => "Wasm",
    MediaType::Unknown => "Unknown",
  }
}

// Warning! The values in this enum are duplicated in js/compiler.ts
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CompilerRequestType {
  Compile = 0,
  RuntimeCompile = 1,
  RuntimeTranspile = 2,
}
