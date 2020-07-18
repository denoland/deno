// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Warning! The values in this enum are duplicated in js/compiler.ts
// Update carefully!
use serde::Serialize;
use serde::Serializer;

#[allow(non_camel_case_types)]
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MediaType {
  JavaScript = 0,
  JSX = 1,
  TypeScript = 2,
  TSX = 3,
  Json = 4,
  Wasm = 5,
  Unknown = 6,
}

impl Serialize for MediaType {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value: i32 = match self {
      MediaType::JavaScript => 0 as i32,
      MediaType::JSX => 1 as i32,
      MediaType::TypeScript => 2 as i32,
      MediaType::TSX => 3 as i32,
      MediaType::Json => 4 as i32,
      MediaType::Wasm => 5 as i32,
      MediaType::Unknown => 6 as i32,
    };
    Serialize::serialize(&value, serializer)
  }
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
#[repr(i32)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CompilerRequestType {
  Compile = 0,
  Transpile = 1,
  Bundle = 2,
  RuntimeCompile = 3,
  RuntimeBundle = 4,
  RuntimeTranspile = 5,
}

impl Serialize for CompilerRequestType {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let value: i32 = match self {
      CompilerRequestType::Compile => 0 as i32,
      CompilerRequestType::Transpile => 1 as i32,
      CompilerRequestType::Bundle => 2 as i32,
      CompilerRequestType::RuntimeCompile => 3 as i32,
      CompilerRequestType::RuntimeBundle => 4 as i32,
      CompilerRequestType::RuntimeTranspile => 5 as i32,
    };
    Serialize::serialize(&value, serializer)
  }
}
