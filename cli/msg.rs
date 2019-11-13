// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Warning! The values in this enum are duplicated in js/errors.ts
// Update carefully!
#[allow(non_camel_case_types)]
#[repr(i8)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ErrorKind {
  NoError = 0,
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
  CommandFailed = 20,
  EmptyHost = 21,
  IdnaError = 22,
  InvalidPort = 23,
  InvalidIpv4Address = 24,
  InvalidIpv6Address = 25,
  InvalidDomainCharacter = 26,
  RelativeUrlWithoutBase = 27,
  RelativeUrlWithCannotBeABaseBase = 28,
  SetHostOnCannotBeABaseUrl = 29,
  Overflow = 30,
  HttpUser = 31,
  HttpClosed = 32,
  HttpCanceled = 33,
  HttpParse = 34,
  HttpOther = 35,
  TooLarge = 36,
  InvalidUri = 37,
  InvalidSeekMode = 38,
  OpNotAvailable = 39,
  WorkerInitFailed = 40,
  UnixError = 41,
  NoAsyncSupport = 42,
  NoSyncSupport = 43,
  ImportMapError = 44,
  InvalidPath = 45,
  ImportPrefixMissing = 46,
  UnsupportedFetchScheme = 47,
  TooManyRedirects = 48,
  Diagnostic = 49,
  JSError = 50,
  TypeError = 51,

  /** TODO this is a DomException type, and should be moved out of here when possible */
  DataCloneError = 52,
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
  Unknown = 5,
}

pub fn enum_name_media_type(mt: MediaType) -> &'static str {
  match mt {
    MediaType::JavaScript => "JavaScript",
    MediaType::JSX => "JSX",
    MediaType::TypeScript => "TypeScript",
    MediaType::TSX => "TSX",
    MediaType::Json => "Json",
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
  Bundle = 1,
}
