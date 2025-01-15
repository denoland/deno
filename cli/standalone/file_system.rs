// Copyright 2018-2025 the Deno authors. MIT license.

use std::borrow::Cow;
use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;

use deno_lib::standalone::virtual_fs::VfsFileSubDataKind;
use deno_runtime::deno_fs::AccessCheckCb;
use deno_runtime::deno_fs::FileSystem;
use deno_runtime::deno_fs::FsDirEntry;
use deno_runtime::deno_fs::FsFileType;
use deno_runtime::deno_fs::OpenOptions;
use deno_runtime::deno_fs::RealFs;
use deno_runtime::deno_io::fs::File;
use deno_runtime::deno_io::fs::FsError;
use deno_runtime::deno_io::fs::FsResult;
use deno_runtime::deno_io::fs::FsStat;
use sys_traits::boxed::BoxedFsDirEntry;
use sys_traits::boxed::BoxedFsMetadataValue;
use sys_traits::boxed::FsMetadataBoxed;
use sys_traits::boxed::FsReadDirBoxed;
use sys_traits::FsCopy;
use sys_traits::FsMetadata;

use super::virtual_fs::FileBackedVfs;
use super::virtual_fs::FileBackedVfsDirEntry;
use super::virtual_fs::FileBackedVfsFile;
use super::virtual_fs::FileBackedVfsMetadata;
