// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
  #[error("invalid eszip v1: {0}")]
  InvalidV1Json(serde_json::Error),
  #[error("invalid eszip v1 version: got {0}, expected 1")]
  InvalidV1Version(u32),
  #[error("invalid eszip v2")]
  InvalidV2,
  #[error("invalid eszip v2 header hash")]
  InvalidV2HeaderHash,
  #[error("invalid specifier in eszip v2 header at offset {0}")]
  InvalidV2Specifier(usize),
  #[error("invalid entry kind {0} in eszip v2 header at offset {0}")]
  InvalidV2EntryKind(u8, usize),
  #[error("invalid module kind {0} in eszip v2 header at offset {0}")]
  InvalidV2ModuleKind(u8, usize),
  #[error("invalid eszip v2 header: {0}")]
  InvalidV2Header(&'static str),
  #[error("invalid eszip v2 source offset ({0})")]
  InvalidV2SourceOffset(usize),
  #[error("invalid eszip v2 source hash (specifier {0})")]
  InvalidV2SourceHash(String),
  #[error("invalid eszip v2.1 npm snapshot hash")]
  InvalidV2NpmSnapshotHash,
  #[error("invalid eszip v2.1 npm package at index {0}. {1:#}")]
  InvalidV2NpmPackageOffset(usize, std::io::Error),
  #[error("invalid eszip v2.1 npm package '{0}'. {1:#}")]
  InvalidV2NpmPackage(String, anyhow::Error),
  #[error("invalid eszip v2.1 npm req '{0}'. {1:#}")]
  InvalidV2NpmPackageReq(String, anyhow::Error),
  #[error("invalid eszip v2.2 options header")]
  InvalidV22OptionsHeader(String),
  #[error("invalid eszip v2.2 options header hash")]
  InvalidV22OptionsHeaderHash,

  #[error(transparent)]
  Io(#[from] std::io::Error),
}
