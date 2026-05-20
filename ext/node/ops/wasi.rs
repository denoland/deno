// Copyright 2018-2026 the Deno authors. MIT license.

// WASI requires direct filesystem access for host operations.
// The FileSystem trait doesn't provide the low-level primitives needed here.
#![allow(
  clippy::disallowed_methods,
  reason = "WASI needs direct host filesystem access instead of the FileSystem trait"
)]

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
const ERRNO_LOOP: i32 = 32;
const ERRNO_NOENT: i32 = 44;
const ERRNO_NOSYS: i32 = 52;
const ERRNO_NOTDIR: i32 = 54;
const ERRNO_NOTEMPTY: i32 = 55;
const ERRNO_NOTCAPABLE: i32 = 76;
#[allow(dead_code, reason = "kept for parity with full WASI errno set")]
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
const RIGHTS_PATH_LINK_SOURCE: u64 = 1 << 11;
const RIGHTS_PATH_LINK_TARGET: u64 = 1 << 12;
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
  | RIGHTS_PATH_LINK_SOURCE
  | RIGHTS_PATH_LINK_TARGET
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

// Rights whose presence indicates the caller intends to mutate the file or
// its metadata (write data, extend it, change its times, fsync, etc.). When
// any of these is requested at path_open, we treat the open as needing
// Deno write permission so a read-only --allow-read can't be turned into a
// write-capable handle.
const RIGHTS_MUTATING: u64 = RIGHTS_FD_WRITE
  | RIGHTS_FD_DATASYNC
  | RIGHTS_FD_SYNC
  | RIGHTS_FD_ALLOCATE
  | RIGHTS_FD_FILESTAT_SET_SIZE
  | RIGHTS_FD_FILESTAT_SET_TIMES
  | RIGHTS_PATH_CREATE_DIRECTORY
  | RIGHTS_PATH_FILESTAT_SET_TIMES
  | RIGHTS_PATH_LINK_TARGET
  | RIGHTS_PATH_SYMLINK
  | RIGHTS_PATH_REMOVE_DIRECTORY
  | RIGHTS_PATH_UNLINK_FILE
  | RIGHTS_PATH_RENAME_SOURCE
  | RIGHTS_PATH_RENAME_TARGET;

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

const LOOKUPFLAGS_SYMLINK_FOLLOW: u32 = 1;

// fstflags bits used by *_filestat_set_times
const FSTFLAGS_ATIM: u16 = 1 << 0;
const FSTFLAGS_ATIM_NOW: u16 = 1 << 1;
const FSTFLAGS_MTIM: u16 = 1 << 2;
const FSTFLAGS_MTIM_NOW: u16 = 1 << 3;

// poll_oneoff
const EVENTTYPE_CLOCK: u8 = 0;
const EVENTTYPE_FD_READ: u8 = 1;
const EVENTTYPE_FD_WRITE: u8 = 2;
const SUBCLOCKFLAGS_ABSTIME: u16 = 1;
const EVENT_FD_READWRITE_HANGUP: u16 = 1;

enum FdEntry {
  // Inherited stdio: read/write to the process's std streams.
  Stdin,
  Stdout,
  Stderr,
  // Override stdio backed by a host file (e.g. when the WASI constructor was
  // given `stdin/stdout/stderr` host fds). We dup the host fd at WASI
  // construction time so the host-side fd remains usable after WASI exits
  // and closes its dup.
  HostFile {
    file: std::fs::File,
    is_stdin: bool,
  },
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
    preopen_root_path: std::path::PathBuf,
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

  /// Returns the WASI rights granted to `fd` at path_open time, if the fd
  /// is a regular file or directory we tracked. Stdio and HostFile entries
  /// return None because rights are implied by the entry type rather than
  /// stored explicitly.
  ///
  /// Note: callers pair this with [`has_right`], which treats `None` as
  /// "all rights granted". For inherited stdio that's fine. For HostFile,
  /// the per-op rights gate is intentionally skipped because the Deno
  /// permission for the underlying path was already paid when node:fs
  /// opened the user fd (see [`make_stdio_entry`]'s `FdTable` check);
  /// the OS-level open mode then limits what actually succeeds.
  fn fd_rights(&self, fd: i32) -> Option<u64> {
    match self.get_fd(fd) {
      Some(FdEntry::File { rights, .. })
      | Some(FdEntry::Dir { rights, .. }) => Some(*rights),
      _ => None,
    }
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
    self.resolve_preopen_path_ex(
      dirfd,
      path,
      permissions,
      access_kind,
      /* follow_symlinks */ true,
    )
  }

  /// Resolves a path under a preopen, optionally following symlinks for the
  /// final component. The result is canonicalized when the path exists.
  ///
  /// Returns ERRNO_NOTCAPABLE if the resolved path escapes the preopen
  /// root (matches Node/uvwasi behavior for sandbox violations such as
  /// `../outside.txt` or symlinks that point outside the preopen).
  /// Returns ERRNO_LOOP when canonicalization hits a symlink loop.
  fn resolve_preopen_path_ex(
    &self,
    dirfd: i32,
    path: &str,
    permissions: &PermissionsContainer,
    access_kind: OpenAccessKind,
    follow_symlinks: bool,
  ) -> Result<std::path::PathBuf, i32> {
    let (real_root, dir_base) = match self.get_fd(dirfd) {
      Some(FdEntry::PreopenDir { real_path, .. }) => {
        (real_path.as_str().to_string(), None)
      }
      Some(FdEntry::Dir {
        path: dir_path,
        preopen_root_path,
        ..
      }) => (
        preopen_root_path.to_string_lossy().into_owned(),
        Some(dir_path.clone()),
      ),
      _ => return Err(ERRNO_BADF),
    };

    let base =
      dir_base.unwrap_or_else(|| std::path::PathBuf::from(real_root.clone()));
    let resolved = base.join(path);
    let canonical_base = canonicalize_io(&real_root)?;

    let canonical_resolved =
      canonicalize_lookup(&resolved, follow_symlinks, &canonical_base)?;

    if !canonical_resolved.starts_with(&canonical_base) {
      return Err(ERRNO_NOTCAPABLE);
    }

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

/// Canonicalize a path, mapping io errors to WASI errno.
fn canonicalize_io<P: AsRef<Path>>(p: P) -> Result<std::path::PathBuf, i32> {
  std::fs::canonicalize(p).map_err(|e| io_err_to_errno(&e))
}

/// Best-effort canonicalization used by path lookup.
///
/// When `follow_symlinks` is false and the final path component is a symlink,
/// the link itself is the target (used by path_open with
/// LOOKUPFLAGS_SYMLINK_FOLLOW=0). The link must still resolve to a path
/// rooted in the preopen, otherwise ERRNO_NOTCAPABLE is returned by the
/// caller after a starts_with check.
fn canonicalize_lookup(
  resolved: &std::path::Path,
  follow_symlinks: bool,
  canonical_base: &std::path::Path,
) -> Result<std::path::PathBuf, i32> {
  // When the path exists and we can canonicalize, that's the most accurate
  // answer. Canonicalizing follows symlinks, which is the default WASI
  // lookup mode (LOOKUPFLAGS_SYMLINK_FOLLOW).
  let try_canonical = if follow_symlinks {
    Some(std::fs::canonicalize(resolved))
  } else {
    // Canonicalize the parent and append the final component literally.
    if let (Some(parent), Some(name)) =
      (resolved.parent(), resolved.file_name())
    {
      match std::fs::canonicalize(parent) {
        Ok(p) => Some(Ok(p.join(name))),
        Err(e) => Some(Err(e)),
      }
    } else {
      Some(std::fs::canonicalize(resolved))
    }
  };

  match try_canonical {
    Some(Ok(p)) => Ok(p),
    Some(Err(e)) => {
      // Detect ELOOP via raw_os_error so we don't depend on the unstable
      // ErrorKind::FilesystemLoop variant.
      if is_loop_error(&e) {
        return Err(ERRNO_LOOP);
      }
      // When the path doesn't exist, fall back to canonicalizing the parent
      // and joining the file_name — used by path_open with O_CREAT for
      // files that don't yet exist.
      let parent = resolved.parent().ok_or(ERRNO_NOENT)?;
      let parent_canonical = std::fs::canonicalize(parent).map_err(|pe| {
        if is_loop_error(&pe) {
          ERRNO_LOOP
        } else {
          io_err_to_errno(&pe)
        }
      })?;
      let filename = resolved.file_name().ok_or(ERRNO_INVAL)?;
      let joined = parent_canonical.join(filename);
      // Even for a non-existent target we sanity-check the resolved parent
      // is rooted in the preopen so we never permit writes outside.
      if !joined.starts_with(canonical_base) {
        return Err(ERRNO_NOTCAPABLE);
      }
      Ok(joined)
    }
    None => Err(ERRNO_NOENT),
  }
}

fn is_loop_error(e: &std::io::Error) -> bool {
  let Some(code) = e.raw_os_error() else {
    return false;
  };
  #[cfg(unix)]
  {
    code == libc::ELOOP
  }
  #[cfg(windows)]
  {
    // ERROR_CANT_RESOLVE_FILENAME (1921) is what Windows returns when a
    // reparse-point chain is too deep.
    code == 1921
  }
  #[cfg(not(any(unix, windows)))]
  {
    let _ = code;
    false
  }
}

/// Whether the fd's stored rights bitset includes every bit in `needed`.
/// Stdio / HostFile entries don't carry an explicit rights field — they
/// return `None` from `fd_rights` and we let those fall through to the
/// per-op entry-type checks. For File/Dir entries the rights were granted
/// at path_open time and intersected with the caller's request.
fn has_right(rights: Option<u64>, needed: u64) -> bool {
  match rights {
    Some(r) => r & needed == needed,
    None => true,
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

fn get_memory_slice(memory: &[u8], offset: i32, len: i32) -> Option<&[u8]> {
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
    #[smi] stdin_fd: i32,
    #[smi] stdout_fd: i32,
    #[smi] stderr_fd: i32,
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

    // Build the three stdio slots, honoring user-supplied host fds. A user
    // fd of 0/1/2 means "use the process std stream"; any other fd is the
    // OS fd (Unix) or CRT fd (Windows) of a file the user opened via
    // node:fs and wants WASI to read/write through.
    let stdin = make_stdio_entry(state, stdin_fd, /*is_stdin*/ true)
      .unwrap_or(FdEntry::Stdin);
    let stdout = make_stdio_entry(state, stdout_fd, /*is_stdin*/ false)
      .unwrap_or(FdEntry::Stdout);
    let stderr = make_stdio_entry(state, stderr_fd, /*is_stdin*/ false)
      .unwrap_or(FdEntry::Stderr);

    let mut fds: Vec<Option<FdEntry>> =
      vec![Some(stdin), Some(stdout), Some(stderr)];

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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_WRITE) {
      return ERRNO_NOTCAPABLE;
    }
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
      Some(FdEntry::HostFile { file, is_stdin }) if !*is_stdin => {
        for buf in &bufs {
          if !buf.is_empty() {
            match file.write(buf) {
              Ok(n) => total_written += n as u32,
              Err(e) => return io_err_to_errno(&e),
            }
          }
        }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_READ) {
      return ERRNO_NOTCAPABLE;
    }
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
      Some(FdEntry::HostFile { file, is_stdin }) if *is_stdin => {
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
      Some(FdEntry::HostFile { is_stdin, .. }) => {
        // Backing file inherits the rights of the stdio slot it replaces.
        write_u8(memory, buf, FILETYPE_REGULAR_FILE);
        if *is_stdin {
          write_u64(memory, buf + 8, RIGHTS_FD_READ | RIGHTS_FD_SEEK);
        } else {
          write_u64(memory, buf + 8, RIGHTS_FD_WRITE | RIGHTS_FD_SEEK);
        }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_SYNC) {
      return ERRNO_NOTCAPABLE;
    }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_DATASYNC) {
      return ERRNO_NOTCAPABLE;
    }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_ALLOCATE) {
      return ERRNO_NOTCAPABLE;
    }
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
      Some(FdEntry::HostFile { file, .. }) => match file.metadata() {
        Ok(meta) => {
          write_filestat(memory, buf, &meta);
          ERRNO_SUCCESS
        }
        Err(e) => io_err_to_errno(&e),
      },
      None => ERRNO_BADF,
    }
  }

  #[fast]
  fn fd_filestat_set_size(&self, #[smi] fd: i32, #[number] size: i64) -> i32 {
    let inner = self.inner.borrow();
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_FILESTAT_SET_SIZE) {
      return ERRNO_NOTCAPABLE;
    }
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
    #[smi] fd: i32,
    #[number] atim: i64,
    #[number] mtim: i64,
    #[smi] fst_flags: i32,
  ) -> i32 {
    let inner = self.inner.borrow();
    // futimens on a read-only handle still succeeds at the OS level when
    // the caller owns the file, so this rights check is the only guard
    // against using read-only access to mutate atime/mtime.
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_FILESTAT_SET_TIMES) {
      return ERRNO_NOTCAPABLE;
    }
    let file_ref = match inner.get_fd(fd) {
      Some(FdEntry::File { file, .. }) => file,
      _ => return ERRNO_BADF,
    };
    let meta = match file_ref.metadata() {
      Ok(m) => m,
      Err(e) => return io_err_to_errno(&e),
    };
    let (atime, mtime) = match resolve_filestat_times_from_meta(
      &meta,
      atim as u64,
      mtim as u64,
      fst_flags as u16,
    ) {
      Ok(v) => v,
      Err(e) => return e,
    };
    match filetime::set_file_handle_times(file_ref, Some(atime), Some(mtime)) {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_READDIR) {
      return ERRNO_NOTCAPABLE;
    }
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
    #[smi] dirflags: i32,
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
    // Any right that would let the caller mutate file state or metadata
    // requires Deno write permission. RIGHTS_FD_WRITE alone is not enough
    // because RIGHTS_FD_FILESTAT_SET_TIMES (utimens via fd) succeeds at the
    // OS level even on read-only handles when the caller owns the file.
    let needs_write_perm = rights & RIGHTS_MUTATING != 0;
    // Distinct from the permission gate: only actually open the OS file in
    // write mode when the caller asked to write data or wants O_CREAT etc.
    // Asking for SET_TIMES alone shouldn't force the underlying open to
    // demand OS write access.
    let wants_os_write = rights & RIGHTS_FD_WRITE != 0;
    let creates = oflags & OFLAGS_CREAT != 0
      || oflags & OFLAGS_EXCL != 0
      || oflags & OFLAGS_TRUNC != 0;
    let follow_symlinks = (dirflags as u32) & LOOKUPFLAGS_SYMLINK_FOLLOW != 0;

    let access_kind = if needs_write_perm || creates {
      OpenAccessKind::ReadWrite
    } else {
      OpenAccessKind::Read
    };

    let mut inner = self.inner.borrow_mut();
    let resolved = match inner.resolve_preopen_path_ex(
      dirfd,
      &path_str,
      &self.permissions,
      access_kind,
      follow_symlinks,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };

    if oflags & OFLAGS_DIRECTORY != 0 || resolved.is_dir() {
      if oflags & OFLAGS_DIRECTORY != 0 && !resolved.is_dir() {
        return ERRNO_NOTDIR;
      }
      let preopen_root_path = match inner.get_fd(dirfd) {
        Some(FdEntry::PreopenDir { real_path, .. }) => {
          match std::fs::canonicalize(real_path) {
            Ok(path) => path,
            Err(e) => return io_err_to_errno(&e),
          }
        }
        Some(FdEntry::Dir {
          preopen_root_path, ..
        }) => preopen_root_path.clone(),
        _ => return ERRNO_BADF,
      };
      // Grant only the requested rights (intersected with the maximum a
      // directory fd can hold). The earlier permissions.check_open already
      // gated Read vs ReadWrite based on the requested rights, so we mustn't
      // hand out rights beyond what the caller asked for.
      let granted = rights & RIGHTS_DIR;
      let new_fd = inner.alloc_fd(FdEntry::Dir {
        path: resolved,
        preopen_root_path,
        rights: granted,
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
    if wants_os_write {
      opts.write(true);
    }

    match opts.open(&resolved) {
      Ok(file) => {
        // Hand out only the requested rights, intersected with what a file
        // fd can hold. Each mutating fd_* op re-checks the corresponding
        // right below so the caller can't escalate by, say, opening with
        // RIGHTS_FD_READ and then calling fd_filestat_set_times.
        let granted = rights & RIGHTS_FILE;
        let new_fd = inner.alloc_fd(FdEntry::File {
          file,
          rights: granted,
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
    #[smi] flags: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[smi] filestat_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let follow_symlinks = (flags as u32) & LOOKUPFLAGS_SYMLINK_FOLLOW != 0;
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path_ex(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Read,
      follow_symlinks,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    let Some(dest) = get_memory_slice_mut(memory, filestat_ptr, 64) else {
      return ERRNO_FAULT;
    };
    dest.fill(0);
    let meta_result = if follow_symlinks {
      std::fs::metadata(&resolved)
    } else {
      std::fs::symlink_metadata(&resolved)
    };
    match meta_result {
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
    // readlink must NOT follow the final symlink: we want to read the link
    // target as stored on disk. Following would resolve to the underlying
    // file and std::fs::read_link would reject it as InvalidInput.
    let resolved = match inner.resolve_preopen_path_ex(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Read,
      /* follow_symlinks */ false,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::read_link(&resolved) {
      Ok(target) => {
        let target_bytes = target.as_os_str().as_encoded_bytes();
        // Best-effort reverse of the normalization in path_symlink: on
        // Windows we rewrote forward slashes to backslashes so the OS
        // could resolve relative symlinks, so here we flip them back to
        // give WASI callers a POSIX-style target. This is biased toward
        // the test-wasi-symlinks round-trip case — a symlink created
        // outside WASI with literal backslashes in its target will have
        // its bytes rewritten to forward slashes too, which is a known
        // limitation we accept to keep the WASI surface POSIX-shaped.
        #[cfg(windows)]
        let owned = target_bytes
          .iter()
          .map(|b| if *b == b'\\' { b'/' } else { *b })
          .collect::<Vec<u8>>();
        #[cfg(windows)]
        let target_bytes: &[u8] = &owned;
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
      // Windows stores the link target verbatim in the reparse point. WASI
      // callers pass POSIX-style targets with forward slashes (the C
      // create_symlink test calls `symlink("./input-in-subdir.txt", …)`),
      // and Windows can't resolve those — opens through the link fail.
      // Normalize to backslashes here so subsequent path_open succeeds.
      // If the existing target is a directory, fall back to symlink_dir so
      // resolution doesn't break on directory targets.
      let target = old_path.replace('/', "\\");
      let target_path = std::path::PathBuf::from(&target);
      let abs_target = if target_path.is_absolute() {
        target_path.clone()
      } else if let Some(parent) = new_resolved.parent() {
        parent.join(&target_path)
      } else {
        target_path.clone()
      };
      let is_dir = std::fs::metadata(&abs_target)
        .map(|m| m.is_dir())
        .unwrap_or(false);
      let result = if is_dir {
        std::os::windows::fs::symlink_dir(&target, &new_resolved)
      } else {
        std::os::windows::fs::symlink_file(&target, &new_resolved)
      };
      match result {
        Ok(()) => ERRNO_SUCCESS,
        Err(e) => io_err_to_errno(&e),
      }
    }
  }

  #[fast]
  fn path_filestat_set_times(
    &self,
    #[smi] dirfd: i32,
    #[smi] flags: i32,
    #[smi] path_ptr: i32,
    #[smi] path_len: i32,
    #[number] atim: i64,
    #[number] mtim: i64,
    #[smi] fst_flags: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    let Some(path_str) = read_string(memory, path_ptr, path_len) else {
      return ERRNO_FAULT;
    };
    let follow_symlinks = (flags as u32) & LOOKUPFLAGS_SYMLINK_FOLLOW != 0;
    let inner = self.inner.borrow();
    let resolved = match inner.resolve_preopen_path_ex(
      dirfd,
      &path_str,
      &self.permissions,
      OpenAccessKind::Write,
      follow_symlinks,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    // Mirror path_filestat_get: honor LOOKUPFLAGS_SYMLINK_FOLLOW for both the
    // metadata lookup we use to fill in absent atim/mtim and for the
    // utimensat-equivalent call. Otherwise a caller passing flags=0 would
    // silently traverse symlinks and modify the target's times.
    let meta = if follow_symlinks {
      std::fs::metadata(&resolved)
    } else {
      std::fs::symlink_metadata(&resolved)
    };
    let meta = match meta {
      Ok(m) => m,
      Err(e) => return io_err_to_errno(&e),
    };
    let (atime, mtime) = match resolve_filestat_times_from_meta(
      &meta,
      atim as u64,
      mtim as u64,
      fst_flags as u16,
    ) {
      Ok(v) => v,
      Err(e) => return e,
    };
    let result = if follow_symlinks {
      filetime::set_file_times(&resolved, atime, mtime)
    } else {
      filetime::set_symlink_file_times(&resolved, atime, mtime)
    };
    match result {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
  }

  #[fast]
  fn path_link(
    &self,
    #[smi] old_dirfd: i32,
    #[smi] _old_flags: i32,
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
    // The Deno permission check inside `resolve_preopen_path` is what
    // actually gates the OS-level hardlink; the per-fd rights check here
    // is purely for parity with the other path_* ops that consult the
    // dirfd's stored rights (`path_create_directory`, `path_unlink_file`,
    // etc.). Skipping it would be safe but inconsistent.
    if !has_right(inner.fd_rights(old_dirfd), RIGHTS_PATH_LINK_SOURCE)
      || !has_right(inner.fd_rights(new_dirfd), RIGHTS_PATH_LINK_TARGET)
    {
      return ERRNO_NOTCAPABLE;
    }
    let old_resolved = match inner.resolve_preopen_path(
      old_dirfd,
      &old_path,
      &self.permissions,
      OpenAccessKind::Read,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    let new_resolved = match inner.resolve_preopen_path(
      new_dirfd,
      &new_path,
      &self.permissions,
      OpenAccessKind::Write,
    ) {
      Ok(p) => p,
      Err(e) => return e,
    };
    match std::fs::hard_link(&old_resolved, &new_resolved) {
      Ok(()) => ERRNO_SUCCESS,
      Err(e) => io_err_to_errno(&e),
    }
  }

  // poll_oneoff blocks the current thread until at least one of the
  // requested subscriptions fires. We support:
  //   * SUBSCRIPTION_CLOCK on monotonic/realtime: both relative deadlines
  //     (used by wasi-libc's sleep()) and absolute (SUBCLOCKFLAGS_ABSTIME).
  //   * SUBSCRIPTION_FD_READ on regular files (returns immediately with
  //     remaining bytes) and stdin (returns immediately with hangup set,
  //     matching the behavior wasi-libc maps to POLLHUP|POLLIN — this is
  //     what test-wasi-poll.js expects when stdin is not a TTY).
  //   * SUBSCRIPTION_FD_WRITE on stdout/stderr/regular files (always ready).
  // Behavior for unrecognized subscription types is to emit an EINVAL event.
  #[fast]
  fn poll_oneoff(
    &self,
    #[smi] in_ptr: i32,
    #[smi] out_ptr: i32,
    #[smi] nsubscriptions: i32,
    #[smi] nevents_ptr: i32,
    #[buffer] memory: &mut [u8],
  ) -> i32 {
    if nsubscriptions <= 0 {
      return ERRNO_INVAL;
    }

    let subs = match read_subscriptions(memory, in_ptr, nsubscriptions) {
      Ok(s) => s,
      Err(e) => return e,
    };

    let inner = self.inner.borrow();
    let mut events: Vec<PollEvent> = Vec::new();
    // Earliest deadline across all CLOCK subscriptions. None = no deadline.
    let mut deadline: Option<std::time::Instant> = None;

    for sub in &subs {
      match sub.tag {
        EVENTTYPE_CLOCK => {
          let now = std::time::Instant::now();
          let due = if sub.clock_flags & SUBCLOCKFLAGS_ABSTIME != 0 {
            // Absolute deadline against the clock id. For monotonic this is
            // nanoseconds since process start; for realtime we approximate
            // by computing how much wall time remains.
            let now_ns = match sub.clock_id {
              CLOCK_REALTIME => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
              _ => monotonic_nanos(),
            };
            if sub.clock_timeout > now_ns {
              now + std::time::Duration::from_nanos(sub.clock_timeout - now_ns)
            } else {
              now
            }
          } else {
            now + std::time::Duration::from_nanos(sub.clock_timeout)
          };
          deadline = Some(match deadline {
            Some(d) => d.min(due),
            None => due,
          });
        }
        EVENTTYPE_FD_READ => {
          let (errno, hangup, nbytes) = match inner.get_fd(sub.fd) {
            Some(FdEntry::Stdin) => {
              // wasi-libc maps EVENT_FD_READWRITE_HANGUP into the POLLHUP
              // bit of revents. test-wasi-poll.js asserts POLLHUP|POLLIN
              // when stdin is not a tty.
              (ERRNO_SUCCESS, EVENT_FD_READWRITE_HANGUP, 0u64)
            }
            Some(FdEntry::File { file, .. }) => {
              // metadata().len() doesn't need &mut, but stream_position does.
              // Use a non-mutating estimate (file length) when we can't seek.
              let nb = file.metadata().ok().map(|m| m.len()).unwrap_or(0);
              (ERRNO_SUCCESS, 0, nb)
            }
            Some(_) => (ERRNO_INVAL, 0, 0),
            None => (ERRNO_BADF, 0, 0),
          };
          events.push(PollEvent {
            userdata: sub.userdata,
            error: errno as u16,
            ty: EVENTTYPE_FD_READ,
            nbytes,
            flags: hangup,
          });
        }
        EVENTTYPE_FD_WRITE => {
          let (errno, nbytes) = match inner.get_fd(sub.fd) {
            Some(FdEntry::Stdout | FdEntry::Stderr) => (ERRNO_SUCCESS, 0u64),
            Some(FdEntry::File { .. }) => (ERRNO_SUCCESS, 0u64),
            Some(_) => (ERRNO_INVAL, 0),
            None => (ERRNO_BADF, 0),
          };
          events.push(PollEvent {
            userdata: sub.userdata,
            error: errno as u16,
            ty: EVENTTYPE_FD_WRITE,
            nbytes,
            flags: 0,
          });
        }
        _ => {
          events.push(PollEvent {
            userdata: sub.userdata,
            error: ERRNO_INVAL as u16,
            ty: sub.tag,
            nbytes: 0,
            flags: 0,
          });
        }
      }
    }

    // Drop the borrow before we sleep — sleeping while holding the RefCell
    // would deadlock any reentrant op (though poll_oneoff doesn't reenter
    // today, this future-proofs the implementation).
    drop(inner);

    // If no FD subscription fired and a clock deadline was set, wait it out.
    // The clock event itself only fires after the wait, signaled below.
    //
    // NOTE: this intentionally blocks the V8 isolate thread for the full
    // clock subscription. WASI's poll_oneoff is a synchronous host call, so
    // we can't yield back to the event loop here without breaking the
    // contract. Callers passing long clock timeouts will hang JS until the
    // deadline elapses — that's the inherent cost of running a synchronous
    // WASI program on a single-threaded runtime, not a bug.
    if events.is_empty()
      && let Some(due) = deadline
    {
      let now = std::time::Instant::now();
      if due > now {
        std::thread::sleep(due - now);
      }
    }

    // If we waited and had clock subscriptions, emit a single clock event so
    // the libc poll() shim returns. We pick the userdata of the first clock
    // subscription, which is sufficient for callers like wasi-libc's sleep
    // that use a single subscription.
    if events.is_empty()
      && let Some(clock_sub) = subs.iter().find(|s| s.tag == EVENTTYPE_CLOCK)
    {
      events.push(PollEvent {
        userdata: clock_sub.userdata,
        error: 0,
        ty: EVENTTYPE_CLOCK,
        nbytes: 0,
        flags: 0,
      });
    }

    let nevents = events.len() as u32;
    for (i, ev) in events.iter().enumerate() {
      let base = out_ptr + (i as i32) * 32;
      if write_u64(memory, base, ev.userdata).is_none()
        || write_u16(memory, base + 8, ev.error).is_none()
        || write_u8(memory, base + 10, ev.ty).is_none()
        || write_u64(memory, base + 16, ev.nbytes).is_none()
        || write_u16(memory, base + 24, ev.flags).is_none()
      {
        return ERRNO_FAULT;
      }
    }
    if write_u32(memory, nevents_ptr, nevents).is_none() {
      return ERRNO_FAULT;
    }
    ERRNO_SUCCESS
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

  // Sockets are not represented in our WASI FdTable, so treat any fd as
  // either an unknown fd (ERRNO_BADF) or a regular fd of the wrong type
  // (ERRNO_NOTSOCK). Returning BADF matches Node/uvwasi behavior for
  // accept() against a non-existent fd, which is what node's
  // test-wasi-sock.js exercises.
  #[fast]
  fn sock_accept(
    &self,
    #[smi] fd: i32,
    #[smi] _flags: i32,
    #[smi] _fd_ptr: i32,
    #[buffer] _memory: &mut [u8],
  ) -> i32 {
    let inner = self.inner.borrow();
    match inner.get_fd(fd) {
      // ERRNO_NOTSOCK (57) for an fd that exists but isn't a socket.
      Some(_) => 57,
      None => ERRNO_BADF,
    }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_READ) {
      return ERRNO_NOTCAPABLE;
    }
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
    if !has_right(inner.fd_rights(fd), RIGHTS_FD_WRITE) {
      return ERRNO_NOTCAPABLE;
    }
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

/// Resolve the atime/mtime tuple for `*_filestat_set_times`, applying
/// the WASI `_NOW` and absent-bit semantics. When neither the bit for the
/// explicit time nor the `_NOW` bit is set for a given component, the
/// existing time on the file is preserved.
fn resolve_filestat_times_from_meta(
  meta: &std::fs::Metadata,
  atim_ns: u64,
  mtim_ns: u64,
  fst_flags: u16,
) -> Result<(filetime::FileTime, filetime::FileTime), i32> {
  // Both *_NOW and the explicit bit set is invalid in WASI.
  if (fst_flags & FSTFLAGS_ATIM != 0 && fst_flags & FSTFLAGS_ATIM_NOW != 0)
    || (fst_flags & FSTFLAGS_MTIM != 0 && fst_flags & FSTFLAGS_MTIM_NOW != 0)
  {
    return Err(ERRNO_INVAL);
  }

  let now = std::time::SystemTime::now();

  let atime = if fst_flags & FSTFLAGS_ATIM_NOW != 0 {
    filetime::FileTime::from_system_time(now)
  } else if fst_flags & FSTFLAGS_ATIM != 0 {
    filetime::FileTime::from_unix_time(
      (atim_ns / 1_000_000_000) as i64,
      (atim_ns % 1_000_000_000) as u32,
    )
  } else {
    filetime::FileTime::from_last_access_time(meta)
  };

  let mtime = if fst_flags & FSTFLAGS_MTIM_NOW != 0 {
    filetime::FileTime::from_system_time(now)
  } else if fst_flags & FSTFLAGS_MTIM != 0 {
    filetime::FileTime::from_unix_time(
      (mtim_ns / 1_000_000_000) as i64,
      (mtim_ns % 1_000_000_000) as u32,
    )
  } else {
    filetime::FileTime::from_last_modification_time(meta)
  };

  Ok((atime, mtime))
}

/// Build an FdEntry for one of the WASI stdio slots from a user-supplied
/// host fd. Returns None for fd 0/1/2 (use inherited process stdio) and
/// for any fd we can't dup or resolve. The dup'd file owns its own OS
/// handle so the user-side fd stays open after WASI exits.
///
/// Permissions: we only accept fds that node:fs has registered in
/// `deno_io::FdTable`, so an opener went through Deno's permission system
/// (op_node_open_sync). Naked OS fds inherited from the parent process or
/// internal Deno opens are rejected so the WASI constructor can't become
/// an escape hatch around `--allow-read` / `--allow-write`.
fn make_stdio_entry(
  state: &mut OpState,
  user_fd: i32,
  is_stdin: bool,
) -> Option<FdEntry> {
  match user_fd {
    0..=2 => None,
    fd if fd < 0 => None,
    fd => {
      if !state.borrow::<deno_io::FdTable>().contains(fd) {
        return None;
      }
      dup_user_fd_to_file(state, fd)
        .map(|file| FdEntry::HostFile { file, is_stdin })
    }
  }
}

/// Duplicate a user-supplied node:fs fd into a `std::fs::File`. The dup
/// gives us an independent OS handle that we close on WASI fd_close /
/// drop, leaving the user's original fd untouched.
fn dup_user_fd_to_file(
  state: &mut OpState,
  user_fd: i32,
) -> Option<std::fs::File> {
  // Try the host fd path first. node:fs.openSync hands the caller the
  // real OS fd (Unix) or CRT fd (Windows), so libc::dup is the most direct
  // way to capture an independent owning handle.
  #[cfg(unix)]
  {
    use std::os::fd::FromRawFd;
    // Use F_DUPFD_CLOEXEC instead of plain dup so the dup'd fd is marked
    // close-on-exec. A WASI program that spawns a child process would
    // otherwise leak this host fd into the child, which is exactly the
    // kind of escape hatch we just closed off in make_stdio_entry.
    // SAFETY: fcntl(F_DUPFD_CLOEXEC, 0) is always safe to call. It either
    // returns -1 on error or a fresh fd (>=0) owned by the caller.
    let new_fd = unsafe { libc::fcntl(user_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if new_fd >= 0 {
      // SAFETY: new_fd was just returned by fcntl(F_DUPFD_CLOEXEC), so it's
      // a fresh OS file descriptor we now own. Wrapping it in std::fs::File
      // transfers ownership to the returned File, which closes the fd on
      // Drop.
      return Some(unsafe { std::fs::File::from_raw_fd(new_fd) });
    }
    let _ = state;
    None
  }
  #[cfg(windows)]
  {
    use std::os::windows::io::FromRawHandle;
    // SAFETY: _get_osfhandle on a CRT fd either returns INVALID_HANDLE_VALUE
    // (-1) or a kernel HANDLE. We duplicate that handle so the resulting
    // OwnedHandle / File owns an independent kernel handle.
    let raw_handle = unsafe { libc::get_osfhandle(user_fd) };
    if raw_handle == -1 {
      let _ = state;
      return None;
    }
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::Foundation::DUPLICATE_SAME_ACCESS;
    use windows_sys::Win32::Foundation::DuplicateHandle;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut dup_handle = std::ptr::null_mut();
    // SAFETY: raw_handle is a valid kernel handle from the CRT.
    let ok = unsafe {
      DuplicateHandle(
        GetCurrentProcess(),
        raw_handle as _,
        GetCurrentProcess(),
        &mut dup_handle,
        0,
        0,
        DUPLICATE_SAME_ACCESS,
      )
    };
    if ok == 0 {
      let _ = state;
      return None;
    }
    let file =
      // SAFETY: dup_handle is a fresh duplicated handle owned by us.
      unsafe { std::fs::File::from_raw_handle(dup_handle as _) };
    let _ = CloseHandle;
    let _ = state;
    Some(file)
  }
}

struct PollSubscription {
  userdata: u64,
  tag: u8,
  clock_id: i32,
  clock_timeout: u64,
  #[allow(
    dead_code,
    reason = "WASI subscription field, not currently consulted"
  )]
  clock_precision: u64,
  clock_flags: u16,
  fd: i32,
}

struct PollEvent {
  userdata: u64,
  error: u16,
  ty: u8,
  nbytes: u64,
  flags: u16,
}

fn read_subscriptions(
  memory: &[u8],
  in_ptr: i32,
  nsubscriptions: i32,
) -> Result<Vec<PollSubscription>, i32> {
  let mut subs = Vec::with_capacity(nsubscriptions as usize);
  // subscription layout:
  //   userdata: u64                                   offset 0
  //   subscription_u {
  //     tag: u8 (+7 pad)                              offset 8
  //     u {                                           offset 16
  //       clock { id u32, _pad u32, timeout u64,
  //               precision u64, flags u16 (+6 pad) }
  //       fd_readwrite { file_descriptor u32 }
  //     }
  //   }
  // total size: 48 bytes
  for i in 0..nsubscriptions {
    let base = in_ptr + i * 48;
    let userdata = read_u64(memory, base).ok_or(ERRNO_FAULT)?;
    let tag = *memory.get((base + 8) as usize).ok_or(ERRNO_FAULT)?;
    let u_base = base + 16;
    let (clock_id, clock_timeout, clock_precision, clock_flags, fd) = match tag
    {
      EVENTTYPE_CLOCK => {
        let id = read_u32(memory, u_base).ok_or(ERRNO_FAULT)? as i32;
        let timeout = read_u64(memory, u_base + 8).ok_or(ERRNO_FAULT)?;
        let precision = read_u64(memory, u_base + 16).ok_or(ERRNO_FAULT)?;
        let flags = read_u16(memory, u_base + 24).ok_or(ERRNO_FAULT)?;
        (id, timeout, precision, flags, 0)
      }
      EVENTTYPE_FD_READ | EVENTTYPE_FD_WRITE => {
        let fd = read_u32(memory, u_base).ok_or(ERRNO_FAULT)? as i32;
        (0, 0, 0, 0, fd)
      }
      _ => (0, 0, 0, 0, 0),
    };
    subs.push(PollSubscription {
      userdata,
      tag,
      clock_id,
      clock_timeout,
      clock_precision,
      clock_flags,
      fd,
    });
  }
  Ok(subs)
}

fn read_u64(memory: &[u8], offset: i32) -> Option<u64> {
  let bytes = get_memory_slice(memory, offset, 8)?;
  Some(u64::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u16(memory: &[u8], offset: i32) -> Option<u16> {
  let bytes = get_memory_slice(memory, offset, 2)?;
  Some(u16::from_le_bytes([bytes[0], bytes[1]]))
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
