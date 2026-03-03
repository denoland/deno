// Copyright 2018-2026 the Deno authors. MIT license.

// WASI requires direct filesystem access for host operations.
// The FileSystem trait doesn't provide the low-level primitives needed here.
#![allow(clippy::disallowed_methods)]

use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Path;

use deno_core::GarbageCollected;
use deno_core::OpState;
use deno_core::op2;
use deno_permissions::OpenAccessKind;
use deno_permissions::PermissionsContainer;
use rand::RngCore;

const ERRNO_SUCCESS: i32 = 0;
const ERRNO_ACCES: i32 = 2;
const ERRNO_BADF: i32 = 8;
const ERRNO_FAULT: i32 = 21;
const ERRNO_EXIST: i32 = 20;
const ERRNO_INVAL: i32 = 28;
const ERRNO_IO: i32 = 29;
const ERRNO_ISDIR: i32 = 31;
const ERRNO_NOENT: i32 = 44;
const ERRNO_NOSYS: i32 = 52;
const ERRNO_NOTDIR: i32 = 54;
const ERRNO_NOTEMPTY: i32 = 55;
const ERRNO_PERM: i32 = 63;

const FILETYPE_UNKNOWN: u8 = 0;
const FILETYPE_CHARACTER_DEVICE: u8 = 2;
const FILETYPE_DIRECTORY: u8 = 3;
const FILETYPE_REGULAR_FILE: u8 = 4;
const FILETYPE_SYMBOLIC_LINK: u8 = 7;

const FDFLAGS_APPEND: u16 = 1;

const RIGHTS_FD_READ: u64 = 1 << 1;
const RIGHTS_FD_SEEK: u64 = 1 << 2;
const RIGHTS_FD_DATASYNC: u64 = 1 << 3;
const RIGHTS_FD_SYNC: u64 = 1 << 4;
const RIGHTS_FD_TELL: u64 = 1 << 5;
const RIGHTS_FD_WRITE: u64 = 1 << 6;
const RIGHTS_FD_ADVISE: u64 = 1 << 7;
const RIGHTS_FD_ALLOCATE: u64 = 1 << 8;
const RIGHTS_PATH_CREATE_DIRECTORY: u64 = 1 << 9;
const RIGHTS_PATH_OPEN: u64 = 1 << 13;
const RIGHTS_FD_READDIR: u64 = 1 << 14;
const RIGHTS_PATH_READLINK: u64 = 1 << 15;
const RIGHTS_PATH_RENAME_SOURCE: u64 = 1 << 16;
const RIGHTS_PATH_RENAME_TARGET: u64 = 1 << 17;
const RIGHTS_PATH_FILESTAT_GET: u64 = 1 << 18;
const RIGHTS_PATH_FILESTAT_SET_TIMES: u64 = 1 << 20;
const RIGHTS_FD_FILESTAT_GET: u64 = 1 << 21;
const RIGHTS_FD_FILESTAT_SET_SIZE: u64 = 1 << 22;
const RIGHTS_FD_FILESTAT_SET_TIMES: u64 = 1 << 23;
const RIGHTS_PATH_SYMLINK: u64 = 1 << 24;
const RIGHTS_PATH_REMOVE_DIRECTORY: u64 = 1 << 25;
const RIGHTS_PATH_UNLINK_FILE: u64 = 1 << 26;

const RIGHTS_DIR: u64 = RIGHTS_FD_READ
  | RIGHTS_FD_SEEK
  | RIGHTS_FD_TELL
  | RIGHTS_FD_SYNC
  | RIGHTS_FD_DATASYNC
  | RIGHTS_FD_ADVISE
  | RIGHTS_PATH_CREATE_DIRECTORY
  | RIGHTS_PATH_OPEN
  | RIGHTS_FD_READDIR
  | RIGHTS_PATH_READLINK
  | RIGHTS_PATH_RENAME_SOURCE
  | RIGHTS_PATH_RENAME_TARGET
  | RIGHTS_PATH_FILESTAT_GET
  | RIGHTS_PATH_FILESTAT_SET_TIMES
  | RIGHTS_PATH_SYMLINK
  | RIGHTS_PATH_REMOVE_DIRECTORY
  | RIGHTS_PATH_UNLINK_FILE;

const RIGHTS_FILE: u64 = RIGHTS_FD_READ
  | RIGHTS_FD_WRITE
  | RIGHTS_FD_SEEK
  | RIGHTS_FD_TELL
  | RIGHTS_FD_SYNC
  | RIGHTS_FD_DATASYNC
  | RIGHTS_FD_ADVISE
  | RIGHTS_FD_ALLOCATE
  | RIGHTS_FD_FILESTAT_GET
  | RIGHTS_FD_FILESTAT_SET_SIZE
  | RIGHTS_FD_FILESTAT_SET_TIMES;

const CLOCK_REALTIME: i32 = 0;
const CLOCK_MONOTONIC: i32 = 1;
const CLOCK_PROCESS_CPUTIME: i32 = 2;
const CLOCK_THREAD_CPUTIME: i32 = 3;

const WHENCE_SET: i32 = 0;
const WHENCE_CUR: i32 = 1;
const WHENCE_END: i32 = 2;

const OFLAGS_CREAT: u16 = 1;
const OFLAGS_DIRECTORY: u16 = 2;
const OFLAGS_EXCL: u16 = 4;
const OFLAGS_TRUNC: u16 = 8;

enum FdEntry {
  Stdin,
  Stdout,
  Stderr,
  PreopenDir {
    virtual_path: String,
    real_path: String,
  },
  File {
    file: std::fs::File,
    rights: u64,
    fdflags: u16,
  },
  Dir {
    path: std::path::PathBuf,
    rights: u64,
  },
}

struct WasiInner {
  args: Vec<String>,
  env: Vec<(String, String)>,
  fds: Vec<Option<FdEntry>>,
}

impl WasiInner {
  fn get_fd(&self, fd: i32) -> Option<&FdEntry> {
    self.fds.get(fd as usize).and_then(|e| e.as_ref())
  }

  fn get_fd_mut(&mut self, fd: i32) -> Option<&mut FdEntry> {
    self.fds.get_mut(fd as usize).and_then(|e| e.as_mut())
  }

  fn alloc_fd(&mut self, entry: FdEntry) -> i32 {
    for (i, slot) in self.fds.iter_mut().enumerate() {
      if slot.is_none() {
        *slot = Some(entry);
        return i as i32;
      }
    }
    let fd = self.fds.len() as i32;
    self.fds.push(Some(entry));
    fd
  }

  fn resolve_preopen_path(
    &self,
    dirfd: i32,
    path: &str,
    permissions: &PermissionsContainer,
    access_kind: OpenAccessKind,
  ) -> Result<std::path::PathBuf, i32> {
    let real_root = match self.get_fd(dirfd) {
      Some(FdEntry::PreopenDir { real_path, .. }) => real_path.as_str(),
      Some(FdEntry::Dir { path: dir_path, .. }) => {
        let resolved = dir_path.join(path);
        permissions
          .check_open(
            Cow::Owned(resolved.clone()),
            access_kind,
            Some("node:wasi"),
          )
          .map_err(|_| ERRNO_ACCES)?;
        return Ok(resolved);
      }
      _ => return Err(ERRNO_BADF),
    };

    let resolved = std::path::Path::new(real_root).join(path);
    let canonical_base =
      std::fs::canonicalize(real_root).map_err(|_| ERRNO_NOENT)?;

    let canonical_resolved = if resolved.exists() {
      std::fs::canonicalize(&resolved).map_err(|_| ERRNO_NOENT)?
    } else {
      let parent = resolved.parent().ok_or(ERRNO_NOENT)?;
      let parent_canonical =
        std::fs::canonicalize(parent).map_err(|_| ERRNO_NOENT)?;
      let filename = resolved.file_name().ok_or(ERRNO_INVAL)?;
      parent_canonical.join(filename)
    };

    if !canonical_resolved.starts_with(&canonical_base) {
      return Err(ERRNO_PERM);
    }

    // Check Deno permissions on the resolved path
    permissions
      .check_open(
        Cow::Owned(canonical_resolved.clone()),
        access_kind,
        Some("node:wasi"),
      )
      .map_err(|_| ERRNO_ACCES)?;

    Ok(canonical_resolved)
  }
}

fn io_err_to_errno(e: &std::io::Error) -> i32 {
  use std::io::ErrorKind;
  match e.kind() {
    ErrorKind::NotFound => ERRNO_NOENT,
    ErrorKind::PermissionDenied => ERRNO_ACCES,
    ErrorKind::AlreadyExists => ERRNO_EXIST,
    ErrorKind::InvalidInput => ERRNO_INVAL,
    ErrorKind::IsADirectory => ERRNO_ISDIR,
    ErrorKind::NotADirectory => ERRNO_NOTDIR,
    ErrorKind::DirectoryNotEmpty => ERRNO_NOTEMPTY,
    _ => ERRNO_IO,
  }
}

pub struct WasiContext {
  inner: RefCell<WasiInner>,
  permissions: PermissionsContainer,
}

// SAFETY: WasiContext contains no pointers to trace
unsafe impl GarbageCollected for WasiContext {
  fn trace(&self, _visitor: &mut deno_core::v8::cppgc::Visitor) {}

  fn get_name(&self) -> &'static std::ffi::CStr {
    c"WasiContext"
  }
}

fn get_memory_slice(
  memory: &[u8],
  offset: i32,
  len: i32,
) -> Option<&[u8]> {
  let start = offset as usize;
  let end = start.checked_add(len as usize)?;
  memory.get(start..end)
}

fn get_memory_slice_mut(
  memory: &mut [u8],
  offset: i32,
  len: i32,
) -> Option<&mut [u8]> {
  let start = offset as usize;
  let end = start.checked_add(len as usize)?;
  memory.get_mut(start..end)
}

fn read_i32(memory: &[u8], offset: i32) -> Option<i32> {
  let bytes = get_memory_slice(memory, offset, 4)?;
  Some(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn write_i32(memory: &mut [u8], offset: i32, value: i32) -> Option<()> {
  let slice = get_memory_slice_mut(memory, offset, 4)?;
  slice.copy_from_slice(&value.to_le_bytes());
  Some(())
}

fn write_u32(memory: &mut [u8], offset: i32, value: u32) -> Option<()> {
  write_i32(memory, offset, value as i32)
}

fn read_u32(memory: &[u8], offset: i32) -> Option<u32> {
  read_i32(memory, offset).map(|v| v as u32)
}

fn write_u64(memory: &mut [u8], offset: i32, value: u64) -> Option<()> {
  let slice = get_memory_slice_mut(memory, offset, 8)?;
  slice.copy_from_slice(&value.to_le_bytes());
  Some(())
}

fn write_u16(memory: &mut [u8], offset: i32, value: u16) -> Option<()> {
  let slice = get_memory_slice_mut(memory, offset, 2)?;
  slice.copy_from_slice(&value.to_le_bytes());
  Some(())
}

fn write_u8(memory: &mut [u8], offset: i32, value: u8) -> Option<()> {
  let slice = get_memory_slice_mut(memory, offset, 1)?;
  slice[0] = value;
  Some(())
}

fn read_string(memory: &[u8], offset: i32, len: i32) -> Option<String> {
  let bytes = get_memory_slice(memory, offset, len)?;
  String::from_utf8(bytes.to_vec()).ok()
}

fn parse_iovs(
  memory: &[u8],
  iovs_ptr: i32,
  iovs_len: i32,
) -> Option<Vec<(u32, u32)>> {
  let mut iovs = Vec::with_capacity(iovs_len as usize);
  for i in 0..iovs_len {
    let base = iovs_ptr + i * 8;
    let addr = read_u32(memory, base)?;
    let len = read_u32(memory, base + 4)?;
    iovs.push((addr, len));
  }
  Some(iovs)
}

fn gather_iov_bufs(
  memory: &[u8],
  iovs_ptr: i32,
  iovs_len: i32,
) -> Option<Vec<Vec<u8>>> {
  let mut bufs = Vec::with_capacity(iovs_len as usize);
  for i in 0..iovs_len {
    let base = iovs_ptr + i * 8;
    let addr = read_u32(memory, base)?;
    let len = read_u32(memory, base + 4)?;
    if len == 0 {
      bufs.push(Vec::new());
    } else {
      bufs.push(get_memory_slice(memory, addr as i32, len as i32)?.to_vec());
    }
  }
  Some(bufs)
}

fn monotonic_nanos() -> u64 {
  static START: std::sync::OnceLock<std::time::Instant> =
    std::sync::OnceLock::new();
  let start = START.get_or_init(std::time::Instant::now);
  start.elapsed().as_nanos() as u64
}

#[derive(Debug, thiserror::Error, deno_error::JsError)]
pub enum WasiError {
  #[class(type)]
  #[error("{0}")]
  Type(String),
  #[class(inherit)]
  #[error(transparent)]
  Permission(#[from] deno_permissions::PermissionCheckError),
}

#[op2]
impl WasiContext {
  #[constructor]
  #[cppgc]
  fn new(
    state: &mut OpState,
    #[serde] args: Vec<String>,
    #[serde] env_pairs: Vec<(String, String)>,
    #[serde] preopens: Vec<(String, String)>,
    #[smi] _stdin_fd: i32,
    #[smi] _stdout_fd: i32,
    #[smi] _stderr_fd: i32,
    _return_on_exit: bool,
  ) -> Result<WasiContext, WasiError> {
    let permissions = state.borrow_mut::<PermissionsContainer>().clone();

    // Check read and write permissions for each preopened directory
    for (_virtual_path, real_path) in &preopens {
      permissions.check_open(
        Cow::Borrowed(Path::new(real_path)),
        OpenAccessKind::ReadWrite,
        Some("node:wasi"),
      )?;
    }

    let mut fds: Vec<Option<FdEntry>> = vec![
      Some(FdEntry::Stdin),
      Some(FdEntry::Stdout),
      Some(FdEntry::Stderr),
    ];

    for (virtual_path, real_path) in preopens {
      fds.push(Some(FdEntry::PreopenDir {
        virtual_path,
        real_path,
      }));
    }

    Ok(WasiContext {
      inner: RefCell::new(WasiInner {
        args,
        env: env_pairs,
        fds,
      }),
      permissions,
    })
  }

  #[fast]
  fn args_get(
    &self,
    #[smi] argv_ptr: i32,
    #[smi] argv_buf_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let mut buf_offset = argv_buf_ptr;
    for (i, arg) in inner.args.iter().enumerate() {
      let ptr_offset = argv_ptr + (i as i32) * 4;
      if write_u32(memory, ptr_offset, buf_offset as u32).is_none() {
        return ERRNO_FAULT;
      }
      let arg_bytes = arg.as_bytes();
      let Some(dest) =
        get_memory_slice_mut(memory, buf_offset, arg_bytes.len() as i32 + 1)
      else {
        return ERRNO_FAULT;
      };
      dest[..arg_bytes.len()].copy_from_slice(arg_bytes);
      dest[arg_bytes.len()] = 0;
      buf_offset += arg_bytes.len() as i32 + 1;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn args_sizes_get(
    &self,
    #[smi] argc_ptr: i32,
    #[smi] argv_buf_size_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let argc = inner.args.len() as u32;
    let buf_size: u32 = inner.args.iter().map(|a| a.len() as u32 + 1).sum();
    if write_u32(memory, argc_ptr, argc).is_none()
      || write_u32(memory, argv_buf_size_ptr, buf_size).is_none()
    {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn environ_get(
    &self,
    #[smi] environ_ptr: i32,
    #[smi] environ_buf_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let mut buf_offset = environ_buf_ptr;
    for (i, (key, value)) in inner.env.iter().enumerate() {
      let ptr_offset = environ_ptr + (i as i32) * 4;
      if write_u32(memory, ptr_offset, buf_offset as u32).is_none() {
        return ERRNO_FAULT;
      }
      let entry = format!("{key}={value}");
      let entry_bytes = entry.as_bytes();
      let Some(dest) =
        get_memory_slice_mut(memory, buf_offset, entry_bytes.len() as i32 + 1)
      else {
        return ERRNO_FAULT;
      };
      dest[..entry_bytes.len()].copy_from_slice(entry_bytes);
      dest[entry_bytes.len()] = 0;
      buf_offset += entry_bytes.len() as i32 + 1;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn environ_sizes_get(
    &self,
    #[smi] environc_ptr: i32,
    #[smi] environ_buf_size_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let count = inner.env.len() as u32;
    // key=value\0  ->  key.len() + 1 + value.len() + 1
    let buf_size: u32 = inner
      .env
      .iter()
      .map(|(k, v)| k.len() as u32 + 1 + v.len() as u32 + 1)
      .sum();
    if write_u32(memory, environc_ptr, count).is_none()
      || write_u32(memory, environ_buf_size_ptr, buf_size).is_none()
    {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn clock_res_get(
    &self,
    #[smi] clock_id: i32,
    #[smi] resolution_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let res: u64 = match clock_id {
      CLOCK_REALTIME | CLOCK_PROCESS_CPUTIME | CLOCK_THREAD_CPUTIME => 1_000,
      CLOCK_MONOTONIC => 1,
      _ => return ERRNO_INVAL,
    };
    if write_u64(memory, resolution_ptr, res).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn clock_time_get(
    &self,
    #[smi] clock_id: i32,
    #[smi] _precision: i32,
    #[smi] time_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let time_ns: u64 = match clock_id {
      CLOCK_REALTIME => {
        match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        {
          Ok(d) => d.as_nanos() as u64,
          Err(_) => return ERRNO_IO,
        }
      }
      CLOCK_MONOTONIC | CLOCK_PROCESS_CPUTIME | CLOCK_THREAD_CPUTIME => {
        monotonic_nanos()
      }
      _ => return ERRNO_INVAL,
    };
    if write_u64(memory, time_ptr, time_ns).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn random_get(
    &self,
    #[smi] buf_ptr: i32,
    #[smi] buf_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(dest) = get_memory_slice_mut(memory, buf_ptr, buf_len) else {
      return ERRNO_FAULT;
    };
    rand::thread_rng().fill_bytes(dest);
    ERRNO_SUCCESS
  }

  // Returns the exit code; JS side decides whether to throw or exit.
  #[fast]
  fn proc_exit(&self, #[smi] code: i32) -> i32 {
    code
  }

  #[fast]
  fn proc_raise(&self, #[smi] _sig: i32) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn fd_write(
    &self,
    #[smi] fd: i32,
    #[smi] iovs_ptr: i32,
    #[smi] iovs_len: i32,
    #[smi] nwritten_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(bufs) = gather_iov_bufs(memory, iovs_ptr, iovs_len) else {
      return ERRNO_FAULT;
    };

    let mut inner = self.inner.borrow_mut();
    let mut total_written: u32 = 0;

    match inner.get_fd_mut(fd) {
      Some(FdEntry::Stdout) => {
        for buf in &bufs {
          if !buf.is_empty() {
            let _ = std::io::stdout().write_all(buf);
            total_written += buf.len() as u32;
          }
        }
        let _ = std::io::stdout().flush();
      }
      Some(FdEntry::Stderr) => {
        for buf in &bufs {
          if !buf.is_empty() {
            let _ = std::io::stderr().write_all(buf);
            total_written += buf.len() as u32;
          }
        }
        let _ = std::io::stderr().flush();
      }
      Some(FdEntry::File { file, .. }) => {
        for buf in &bufs {
          if !buf.is_empty() {
            match file.write(buf) {
              Ok(n) => total_written += n as u32,
              Err(e) => return io_err_to_errno(&e),
            }
          }
        }
      }
      _ => return ERRNO_BADF,
    }

    if write_u32(memory, nwritten_ptr, total_written).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_read(
    &self,
    #[smi] fd: i32,
    #[smi] iovs_ptr: i32,
    #[smi] iovs_len: i32,
    #[smi] nread_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(iovs) = parse_iovs(memory, iovs_ptr, iovs_len) else {
      return ERRNO_FAULT;
    };

    let mut inner = self.inner.borrow_mut();
    let mut total_read: u32 = 0;

    match inner.get_fd_mut(fd) {
      Some(FdEntry::Stdin) => {
        for (buf_addr, buf_len) in &iovs {
          if *buf_len == 0 {
            continue;
          }
          let Some(dest) =
            get_memory_slice_mut(memory, *buf_addr as i32, *buf_len as i32)
          else {
            return ERRNO_FAULT;
          };
          match std::io::stdin().read(dest) {
            Ok(n) => {
              total_read += n as u32;
              if (n as u32) < *buf_len {
                break;
              }
            }
            Err(e) => return io_err_to_errno(&e),
          }
        }
      }
      Some(FdEntry::File { file, .. }) => {
        for (buf_addr, buf_len) in &iovs {
          if *buf_len == 0 {
            continue;
          }
          let mut temp = vec![0u8; *buf_len as usize];
          match file.read(&mut temp) {
            Ok(n) => {
              let Some(dest) =
                get_memory_slice_mut(memory, *buf_addr as i32, n as i32)
              else {
                return ERRNO_FAULT;
              };
              dest.copy_from_slice(&temp[..n]);
              total_read += n as u32;
              if (n as u32) < *buf_len {
                break;
              }
            }
            Err(e) => return io_err_to_errno(&e),
          }
        }
      }
      _ => return ERRNO_BADF,
    }

    if write_u32(memory, nread_ptr, total_read).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_seek(
    &self,
    #[smi] fd: i32,
    #[number] offset: i64,
    #[smi] whence: i32,
    #[smi] newoffset_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let seek_from = match whence {
      WHENCE_SET => SeekFrom::Start(offset as u64),
      WHENCE_CUR => SeekFrom::Current(offset),
      WHENCE_END => SeekFrom::End(offset),
      _ => return ERRNO_INVAL,
    };

    let mut inner = self.inner.borrow_mut();
    match inner.get_fd_mut(fd) {
      Some(FdEntry::File { file, .. }) => match file.seek(seek_from) {
        Ok(pos) => {
          if write_u64(memory, newoffset_ptr, pos).is_none() {
            return ERRNO_FAULT;
          }
          ERRNO_SUCCESS
        }
        Err(e) => io_err_to_errno(&e),
      },
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_close(&self, #[smi] fd: i32) -> i32 {
    let mut inner = self.inner.borrow_mut();
    if fd < 0 || fd as usize >= inner.fds.len() {
      return ERRNO_BADF;
    }
    if inner.fds[fd as usize].is_none() {
      return ERRNO_BADF;
    }
    // Drop the entry (closes file handles via Drop)
    inner.fds[fd as usize] = None;
    ERRNO_SUCCESS
  }

  // fdstat: filetype(u8) + pad(1) + flags(u16) + pad(4)
  //       + rights_base(u64) + rights_inheriting(u64) = 24 bytes
  #[fast]
  fn fd_fdstat_get(
    &self,
    #[smi] fd: i32,
    #[smi] buf: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let Some(dest) = get_memory_slice_mut(memory, buf, 24) else {
      return ERRNO_FAULT;
    };
    dest.fill(0);

    match inner.get_fd(fd) {
      Some(FdEntry::Stdin) => {
        write_u8(memory, buf, FILETYPE_CHARACTER_DEVICE);
        write_u64(memory, buf + 8, RIGHTS_FD_READ);
      }
      Some(FdEntry::Stdout | FdEntry::Stderr) => {
        write_u8(memory, buf, FILETYPE_CHARACTER_DEVICE);
        write_u64(memory, buf + 8, RIGHTS_FD_WRITE);
      }
      Some(FdEntry::PreopenDir { .. }) => {
        write_u8(memory, buf, FILETYPE_DIRECTORY);
        write_u64(memory, buf + 8, RIGHTS_DIR);
        write_u64(memory, buf + 16, RIGHTS_FILE | RIGHTS_DIR);
      }
      Some(FdEntry::Dir { rights, .. }) => {
        write_u8(memory, buf, FILETYPE_DIRECTORY);
        write_u64(memory, buf + 8, *rights);
        write_u64(memory, buf + 16, RIGHTS_FILE | RIGHTS_DIR);
      }
      Some(FdEntry::File {
        rights, fdflags, ..
      }) => {
        write_u8(memory, buf, FILETYPE_REGULAR_FILE);
        write_u16(memory, buf + 2, *fdflags);
        write_u64(memory, buf + 8, *rights);
      }
      None => return ERRNO_BADF,
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_fdstat_set_flags(&self, #[smi] _fd: i32, #[smi] _flags: i32) -> i32 {
    ERRNO_SUCCESS
  }

  // prestat: type(u32) + name_len(u32) = 8 bytes
  #[fast]
  fn fd_prestat_get(
    &self,
    #[smi] fd: i32,
    #[smi] prestat_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      Some(FdEntry::PreopenDir { virtual_path, .. }) => {
        if write_u32(memory, prestat_ptr, 0).is_none()
          || write_u32(memory, prestat_ptr + 4, virtual_path.len() as u32)
            .is_none()
        {
          return ERRNO_FAULT;
        }
        ERRNO_SUCCESS
      }
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_prestat_dir_name(
    &self,
    #[smi] fd: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      Some(FdEntry::PreopenDir { virtual_path, .. }) => {
        let vpath = virtual_path.as_bytes();
        let n = std::cmp::min(vpath.len(), path_len as usize);
        let Some(dest) = get_memory_slice_mut(memory, path_ptr, n as i32)
        else {
          return ERRNO_FAULT;
        };
        dest.copy_from_slice(&vpath[..n]);
        ERRNO_SUCCESS
      }
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_tell(
    &self,
    #[smi] fd: i32,
    #[smi] offset_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let mut inner = self.inner.borrow_mut();
    match inner.get_fd_mut(fd) {
      Some(FdEntry::File { file, .. }) => match file.stream_position() {
        Ok(pos) => {
          if write_u64(memory, offset_ptr, pos).is_none() {
            return ERRNO_FAULT;
          }
          ERRNO_SUCCESS
        }
        Err(e) => io_err_to_errno(&e),
      },
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_sync(&self, #[smi] fd: i32) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      Some(FdEntry::File { file, .. }) => match file.sync_all() {
        Ok(()) => ERRNO_SUCCESS,
        Err(e) => io_err_to_errno(&e),
      },
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_datasync(&self, #[smi] fd: i32) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      Some(FdEntry::File { file, .. }) => match file.sync_data() {
        Ok(()) => ERRNO_SUCCESS,
        Err(e) => io_err_to_errno(&e),
      },
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_advise(
    &self,
    #[smi] _fd: i32,
    #[number] _offset: i64,
    #[number] _len: i64,
    #[smi] _advice: i32,
  ) -> i32 {
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_allocate(
    &self,
    #[smi] fd: i32,
    #[number] offset: i64,
    #[number] len: i64,
  ) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      Some(FdEntry::File { file, .. }) => {
        let required = (offset as u64).saturating_add(len as u64);
        match file.metadata() {
          Ok(meta) if meta.len() >= required => ERRNO_SUCCESS,
          Ok(_) => match file.set_len(required) {
            Ok(()) => ERRNO_SUCCESS,
            Err(e) => io_err_to_errno(&e),
          },
          Err(e) => io_err_to_errno(&e),
        }
      }
      _ => ERRNO_BADF,
    }
  }

  // filestat: dev(u64) + ino(u64) + filetype(u8) + pad(7)
  //        + nlink(u64) + size(u64) + atim(u64) + mtim(u64) + ctim(u64)
  //        = 64 bytes
  #[fast]
  fn fd_filestat_get(
    &self,
    #[smi] fd: i32,
    #[smi] buf: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let Some(dest) = get_memory_slice_mut(memory, buf, 64) else {
      return ERRNO_FAULT;
    };
    dest.fill(0);

    match inner.get_fd(fd) {
      Some(FdEntry::File { file, .. }) => match file.metadata() {
        Ok(meta) => {
          write_filestat(memory, buf, &meta);
          ERRNO_SUCCESS
        }
        Err(e) => io_err_to_errno(&e),
      },
      Some(FdEntry::PreopenDir { real_path, .. }) => {
        match std::fs::metadata(real_path) {
          Ok(meta) => {
            write_filestat(memory, buf, &meta);
            ERRNO_SUCCESS
          }
          Err(e) => io_err_to_errno(&e),
        }
      }
      Some(FdEntry::Dir { path, .. }) => match std::fs::metadata(path) {
        Ok(meta) => {
          write_filestat(memory, buf, &meta);
          ERRNO_SUCCESS
        }
        Err(e) => io_err_to_errno(&e),
      },
      Some(FdEntry::Stdin | FdEntry::Stdout | FdEntry::Stderr) => {
        write_u8(memory, buf + 16, FILETYPE_CHARACTER_DEVICE);
        ERRNO_SUCCESS
      }
      None => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_filestat_set_size(&self, #[smi] fd: i32, #[number] size: i64) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      Some(FdEntry::File { file, .. }) => match file.set_len(size as u64) {
        Ok(()) => ERRNO_SUCCESS,
        Err(e) => io_err_to_errno(&e),
      },
      _ => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_filestat_set_times(
    &self,
    #[smi] _fd: i32,
    #[number] _atim: i64,
    #[number] _mtim: i64,
    #[smi] _fst_flags: i32,
  ) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn fd_renumber(&self, #[smi] from: i32, #[smi] to: i32) -> i32 {
    let mut inner = self.inner.borrow_mut();
    if from < 0
      || from as usize >= inner.fds.len()
      || inner.fds[from as usize].is_none()
    {
      return ERRNO_BADF;
    }
    while inner.fds.len() <= to as usize {
      inner.fds.push(None);
    }
    inner.fds[to as usize] = inner.fds[from as usize].take();
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_readdir(
    &self,
    #[smi] fd: i32,
    #[smi] buf_ptr: i32,
    #[smi] buf_len: i32,
    #[number] cookie: i64,
    #[smi] bufused_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    let dir_path = match inner.get_fd(fd) {
      Some(FdEntry::PreopenDir { real_path, .. }) => {
        std::path::PathBuf::from(real_path)
      }
      Some(FdEntry::Dir { path, .. }) => path.clone(),
      _ => return ERRNO_BADF,
    };

    let entries = match std::fs::read_dir(&dir_path) {
      Ok(entries) => entries,
      Err(e) => return io_err_to_errno(&e),
    };

    let mut buf_offset = 0i32;
    let mut entry_index: i64 = 0;
    // dirent: d_next(u64) + d_ino(u64) + d_namlen(u32) + d_type(u8)
    //       + pad(3) = 24 bytes, followed by name bytes
    const DIRENT_SIZE: i32 = 24;

    for entry_result in entries {
      let entry = match entry_result {
        Ok(e) => e,
        Err(e) => return io_err_to_errno(&e),
      };

      entry_index += 1;
      if entry_index <= cookie {
        continue;
      }

      if buf_offset + DIRENT_SIZE > buf_len {
        break;
      }

      let name = entry.file_name();
      let name_bytes = name.as_encoded_bytes();
      let abs_offset = buf_ptr + buf_offset;

      write_u64(memory, abs_offset, entry_index as u64);
      write_u64(memory, abs_offset + 8, 0);
      write_u32(memory, abs_offset + 16, name_bytes.len() as u32);
      let file_type = match entry.file_type() {
        Ok(ft) if ft.is_dir() => FILETYPE_DIRECTORY,
        Ok(ft) if ft.is_file() => FILETYPE_REGULAR_FILE,
        Ok(ft) if ft.is_symlink() => FILETYPE_SYMBOLIC_LINK,
        _ => FILETYPE_UNKNOWN,
      };
      write_u8(memory, abs_offset + 20, file_type);
      buf_offset += DIRENT_SIZE;

      let name_write_len =
        std::cmp::min(name_bytes.len() as i32, buf_len - buf_offset);
      if name_write_len > 0
        && let Some(dest) =
          get_memory_slice_mut(memory, buf_ptr + buf_offset, name_write_len)
      {
        dest.copy_from_slice(&name_bytes[..name_write_len as usize]);
      }
      buf_offset += name_bytes.len() as i32;
    }

    if write_u32(
      memory,
      bufused_ptr,
      std::cmp::min(buf_offset, buf_len) as u32,
    )
    .is_none()
    {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn path_open(
    &self,
    #[smi] dirfd: i32,
    #[smi] _dirflags: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[smi] oflags: i32,
    #[number] fs_rights_base: i64,
    #[number] _fs_rights_inheriting: i64,
    #[smi] fdflags: i32,
    #[smi] fd_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };

    let oflags = oflags as u16;
    let fdflags_u16 = fdflags as u16;
    let rights = fs_rights_base as u64;
    let wants_write = rights & RIGHTS_FD_WRITE != 0;
    let creates = oflags & OFLAGS_CREAT != 0
      || oflags & OFLAGS_EXCL != 0
      || oflags & OFLAGS_TRUNC != 0;

    let access_kind = if wants_write || creates {
      OpenAccessKind::ReadWrite
    } else {
      OpenAccessKind::Read
    };

    let mut inner = self.inner.borrow_mut();
    let resolved = match inner.resolve_preopen_path(
      dirfd,
      &path_str,
      &self.permissions,
      access_kind,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };

    if oflags & OFLAGS_DIRECTORY != 0 || resolved.is_dir() {
      if oflags & OFLAGS_DIRECTORY != 0 && !resolved.is_dir() {
        return ERRNO_NOTDIR;
      }
      let new_fd = inner.alloc_fd(FdEntry::Dir {
        path: resolved,
        rights: RIGHTS_DIR,
      });
      if write_i32(memory, fd_ptr, new_fd).is_none() {
        return ERRNO_FAULT;
      }
      return ERRNO_SUCCESS;
    }

    let mut opts = std::fs::OpenOptions::new();
    opts.read(true);

    if oflags & OFLAGS_CREAT != 0 {
      opts.create(true).write(true);
    }
    if oflags & OFLAGS_EXCL != 0 {
      opts.create_new(true).write(true);
    }
    if oflags & OFLAGS_TRUNC != 0 {
      opts.truncate(true).write(true);
    }
    if fdflags_u16 & FDFLAGS_APPEND != 0 {
      opts.append(true);
    }
    if wants_write {
      opts.write(true);
    }

    match opts.open(&resolved) {
      Ok(file) => {
        let new_fd = inner.alloc_fd(FdEntry::File {
          file,
          rights: RIGHTS_FILE,
          fdflags: fdflags_u16,
        });
        if write_i32(memory, fd_ptr, new_fd).is_none() {
          return ERRNO_FAULT;
        }
        ERRNO_SUCCESS
      }
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_create_directory(
    &self,
    #[smi] dirfd: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Write,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::create_dir(&resolved) {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_remove_directory(
    &self,
    #[smi] dirfd: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Write,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::remove_dir(&resolved) {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_unlink_file(
    &self,
    #[smi] dirfd: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Write,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::remove_file(&resolved) {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_rename(
    &self,
    #[smi] old_dirfd: i32,
    #[smi] old_path_ptr: i32,
    #[smi] old_path_len: i32,
    #[smi] new_dirfd: i32,
    #[smi] new_path_ptr: i32,
    #[smi] new_path_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(old_path) = read_string(memory, old_path_ptr, old_path_len) else {
      return ERRNO_FAULT;
    };
    let Some(new_path) = read_string(memory, new_path_ptr, new_path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let old_resolved = match inner.resolve_preopen_path(
      old_dirfd,
      &old_path,
      &self.permissions,
      OpenAccessKind::ReadWrite,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    let new_resolved = match inner.resolve_preopen_path(
      new_dirfd,
      &new_path,
      &self.permissions,
      OpenAccessKind::ReadWrite,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::rename(&old_resolved, &new_resolved) {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_filestat_get(
    &self,
    #[smi] dirfd: i32,
    #[smi] _flags: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[smi] filestat_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Read,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    let Some(dest) = get_memory_slice_mut(memory, filestat_ptr, 64) else {
      return ERRNO_FAULT;
    };
    dest.fill(0);
    match std::fs::metadata(&resolved) {
      Ok(meta) => {
        write_filestat(memory, filestat_ptr, &meta);
        ERRNO_SUCCESS
      }
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_readlink(
    &self,
    #[smi] dirfd: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[smi] buf_ptr: i32,
    #[smi] buf_len: i32,
    #[smi] bufused_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Read,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::read_link(&resolved) {
      Ok(target) => {
        let target_bytes = target.as_os_str().as_encoded_bytes();
        let n = std::cmp::min(target_bytes.len(), buf_len as usize);
        let Some(dest) = get_memory_slice_mut(memory, buf_ptr, n as i32) else {
          return ERRNO_FAULT;
        };
        dest.copy_from_slice(&target_bytes[..n]);
        if write_u32(memory, bufused_ptr, n as u32).is_none() {
          return ERRNO_FAULT;
        }
        ERRNO_SUCCESS
      }
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_symlink(
    &self,
    #[smi] old_path_ptr: i32,
    #[smi] old_path_len: i32,
    #[smi] dirfd: i32,
    #[smi] new_path_ptr: i32,
    #[smi] new_path_len: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(old_path) = read_string(memory, old_path_ptr, old_path_len) else {
      return ERRNO_FAULT;
    };
    let Some(new_path) = read_string(memory, new_path_ptr, new_path_len) else {
      return ERRNO_FAULT;
    };
    let inner = self.inner.borrow();
    let new_resolved = match inner.resolve_preopen_path(
      dirfd,
      &new_path,
      &self.permissions,
      OpenAccessKind::Write,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    #[cfg(unix)]
    {
      match std::os::unix::fs::symlink(&old_path, &new_resolved) {
        Ok(()) => ERRNO_SUCCESS,
        Err(e) => io_err_to_errno(&e),
      }
    }
    #[cfg(not(unix))]
    {
      match std::os::windows::fs::symlink_file(&old_path, &new_resolved) {
        Ok(()) => ERRNO_SUCCESS,
        Err(e) => io_err_to_errno(&e),
      }
    }
  }

  #[fast]
  fn path_filestat_set_times(
    &self,
    #[smi] _dirfd: i32,
    #[smi] _flags: i32,
    #[smi] _path_ptr: i32,
    #[smi] _path_len: i32,
    #[number] _atim: i64,
    #[number] _mtim: i64,
    #[smi] _fst_flags: i32,
    #[buffer] _memory: &mut [u8],
  ) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn poll_oneoff(
    &self,
    #[smi] _in_ptr: i32,
    #[smi] _out_ptr: i32,
    #[smi] _nsubscriptions: i32,
    #[smi] _nevents_ptr: i32,
    #[buffer] _memory: &mut [u8],
  ) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn sched_yield(&self) -> i32 {
    std::thread::yield_now();
    ERRNO_SUCCESS
  }

  #[fast]
  fn sock_recv(
    &self,
    #[smi] _fd: i32,
    #[smi] _ri_data_ptr: i32,
    #[smi] _ri_data_len: i32,
    #[smi] _ri_flags: i32,
    #[smi] _ro_datalen_ptr: i32,
    #[smi] _ro_flags_ptr: i32,
    #[buffer] _memory: &mut [u8],
  ) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn sock_send(
    &self,
    #[smi] _fd: i32,
    #[smi] _si_data_ptr: i32,
    #[smi] _si_data_len: i32,
    #[smi] _si_flags: i32,
    #[smi] _so_datalen_ptr: i32,
    #[buffer] _memory: &mut [u8],
  ) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn sock_shutdown(&self, #[smi] _fd: i32, #[smi] _how: i32) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn sock_accept(
    &self,
    #[smi] _fd: i32,
    #[smi] _flags: i32,
    #[smi] _fd_ptr: i32,
    #[buffer] _memory: &mut [u8],
  ) -> i32 {
    ERRNO_NOSYS
  }

  #[fast]
  fn fd_pread(
    &self,
    #[smi] fd: i32,
    #[smi] iovs_ptr: i32,
    #[smi] iovs_len: i32,
    #[number] offset: i64,
    #[smi] nread_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(iovs) = parse_iovs(memory, iovs_ptr, iovs_len) else {
      return ERRNO_FAULT;
    };

    let mut inner = self.inner.borrow_mut();
    let mut total_read: u32 = 0;

    match inner.get_fd_mut(fd) {
      Some(FdEntry::File { file, .. }) => {
        let cur = match file.stream_position() {
          Ok(pos) => pos,
          Err(e) => return io_err_to_errno(&e),
        };
        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)) {
          return io_err_to_errno(&e);
        }
        for (buf_addr, buf_len) in &iovs {
          if *buf_len == 0 {
            continue;
          }
          let mut temp = vec![0u8; *buf_len as usize];
          match file.read(&mut temp) {
            Ok(n) => {
              let Some(dest) =
                get_memory_slice_mut(memory, *buf_addr as i32, n as i32)
              else {
                let _ = file.seek(SeekFrom::Start(cur));
                return ERRNO_FAULT;
              };
              dest.copy_from_slice(&temp[..n]);
              total_read += n as u32;
              if (n as u32) < *buf_len {
                break;
              }
            }
            Err(e) => {
              let _ = file.seek(SeekFrom::Start(cur));
              return io_err_to_errno(&e);
            }
          }
        }
        let _ = file.seek(SeekFrom::Start(cur));
      }
      _ => return ERRNO_BADF,
    }

    if write_u32(memory, nread_ptr, total_read).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_pwrite(
    &self,
    #[smi] fd: i32,
    #[smi] iovs_ptr: i32,
    #[smi] iovs_len: i32,
    #[number] offset: i64,
    #[smi] nwritten_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(bufs) = gather_iov_bufs(memory, iovs_ptr, iovs_len) else {
      return ERRNO_FAULT;
    };

    let mut inner = self.inner.borrow_mut();
    let mut total_written: u32 = 0;

    match inner.get_fd_mut(fd) {
      Some(FdEntry::File { file, .. }) => {
        let cur = match file.stream_position() {
          Ok(pos) => pos,
          Err(e) => return io_err_to_errno(&e),
        };
        if let Err(e) = file.seek(SeekFrom::Start(offset as u64)) {
          return io_err_to_errno(&e);
        }
        for buf in &bufs {
          if !buf.is_empty() {
            match file.write(buf) {
              Ok(n) => total_written += n as u32,
              Err(e) => {
                let _ = file.seek(SeekFrom::Start(cur));
                return io_err_to_errno(&e);
              }
            }
          }
        }
        let _ = file.seek(SeekFrom::Start(cur));
      }
      _ => return ERRNO_BADF,
    }

    if write_u32(memory, nwritten_ptr, total_written).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
  }

  #[fast]
  fn fd_fdstat_set_rights(
    &self,
    #[smi] _fd: i32,
    #[number] _fs_rights_base: i64,
    #[number] _fs_rights_inheriting: i64,
  ) -> i32 {
    ERRNO_SUCCESS
  }
}

fn write_filestat(memory: &mut [u8], offset: i32, meta: &std::fs::Metadata) {
  use std::time::UNIX_EPOCH;

  let filetype = if meta.is_dir() {
    FILETYPE_DIRECTORY
  } else if meta.is_file() {
    FILETYPE_REGULAR_FILE
  } else if meta.file_type().is_symlink() {
    FILETYPE_SYMBOLIC_LINK
  } else {
    FILETYPE_UNKNOWN
  };
  write_u8(memory, offset + 16, filetype);
  write_u64(memory, offset + 24, 1); // nlink
  write_u64(memory, offset + 32, meta.len());

  if let Ok(Ok(d)) = meta.accessed().map(|t| t.duration_since(UNIX_EPOCH)) {
    write_u64(memory, offset + 40, d.as_nanos() as u64);
  }
  if let Ok(Ok(d)) = meta.modified().map(|t| t.duration_since(UNIX_EPOCH)) {
    write_u64(memory, offset + 48, d.as_nanos() as u64);
  }
  if let Ok(Ok(d)) = meta.created().map(|t| t.duration_since(UNIX_EPOCH)) {
    write_u64(memory, offset + 56, d.as_nanos() as u64);
  }
}
