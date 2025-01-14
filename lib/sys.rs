// Copyright 2018-2025 the Deno authors. MIT license.

use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;

pub trait DenoLibSys:
  FsCanonicalize
  + FsCreateDirAll
  + FsMetadata
  + FsOpen
  + FsRemoveFile
  + FsRename
  + ThreadSleep
  + SystemRandom
  + Clone
  + std::fmt::Debug
{
}

impl DenoLibSys for sys_traits::impls::RealSys {}
