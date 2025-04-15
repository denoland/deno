// Copyright 2018-2025 the Deno authors. MIT license.

use deno_node::ExtNodeSys;
use sys_traits::FsCanonicalize;
use sys_traits::FsCreateDirAll;
use sys_traits::FsMetadata;
use sys_traits::FsOpen;
use sys_traits::FsRead;
use sys_traits::FsReadDir;
use sys_traits::FsRemoveFile;
use sys_traits::FsRename;
use sys_traits::SystemRandom;
use sys_traits::ThreadSleep;

pub trait DenoLibSys:
  FsCanonicalize
  + FsCreateDirAll
  + FsReadDir
  + FsMetadata
  + FsOpen
  + FsRemoveFile
  + FsRename
  + FsRead
  + ThreadSleep
  + SystemRandom
  + ExtNodeSys
  + Clone
  + Send
  + Sync
  + std::fmt::Debug
  + 'static
{
}

impl<
    T: FsCanonicalize
      + FsCreateDirAll
      + FsReadDir
      + FsMetadata
      + FsOpen
      + FsRemoveFile
      + FsRename
      + FsRead
      + ThreadSleep
      + SystemRandom
      + ExtNodeSys
      + Clone
      + Send
      + Sync
      + std::fmt::Debug
      + 'static,
  > DenoLibSys for T
{
}
