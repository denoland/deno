// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Warning! The values in this enum are duplicated in js/errors.ts
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ErrorKind {
  NotFound = 1,
  PermissionDenied = 2,
  ConnectionRefused = 3,
  ConnectionReset = 4,
  ConnectionAborted = 5,
  NotConnected = 6,
  AddrInUse = 7,
  AddrNotAvailable = 8,
  BrokenPipe = 9,
  AlreadyExists = 10,
  WouldBlock = 11,
  InvalidInput = 12,
  InvalidData = 13,
  TimedOut = 14,
  Interrupted = 15,
  WriteZero = 16,
  Other = 17,
  UnexpectedEof = 18,
  BadResource = 19,
  UrlParse = 20,
  Http = 21,
  TooLarge = 22,
  InvalidSeekMode = 23,
  UnixError = 24,
  InvalidPath = 25,
  ImportPrefixMissing = 26,
  Diagnostic = 27,
  JSError = 28,
}

// Warning! The values in this enum are duplicated in js/compiler.ts
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i8)]
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
