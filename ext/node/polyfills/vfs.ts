// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials require-await

// node:vfs - experimental Virtual File System.
//
// Ported from the merged upstream implementation in nodejs/node#63115
// (lib/vfs.js and lib/internal/vfs/*). The polyfill exposes the same public
// surface: VirtualFileSystem, VirtualProvider, MemoryProvider, RealFSProvider,
// VirtualFileHandle, MemoryFileHandle, VirtualDir, plus the polling-based
// watcher classes. The module-loader / require() hooks from upstream are
// intentionally not wired into Deno's loaders - the VFS instance is the
// namespace through which callers interact with the virtual files.

(function () {
const { core, primordials } = __bootstrap;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const pathMod = core.loadExtScript("ext:deno_node/path/mod.ts");
const fsUtils = core.loadExtScript("ext:deno_node/internal/fs/utils.mjs");
const errorsMod = core.loadExtScript("ext:deno_node/internal/errors.ts");
const utilMod = core.loadExtScript("ext:deno_node/internal/util.mjs");
const validatorsMod = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
const fsConstants = core.loadExtScript(
  "ext:deno_node/internal_binding/constants.ts",
).fs;
const lazyStream = core.createLazyLoader("node:stream");
const lazyEvents = core.createLazyLoader("node:events");
const lazyTimers = core.createLazyLoader("node:timers");
const lazyNodeFs = core.createLazyLoader("node:fs");

const {
  Stats,
  BigIntStats,
  Dirent,
} = fsUtils;
const { emitExperimentalWarning } = utilMod;
const { validateBoolean } = validatorsMod;
const { AbortError } = errorsMod;
const ERR_METHOD_NOT_IMPLEMENTED = errorsMod.codes.ERR_METHOD_NOT_IMPLEMENTED;
const ERR_INVALID_STATE = errorsMod.codes.ERR_INVALID_STATE;
const ERR_DIR_CLOSED = errorsMod.codes.ERR_DIR_CLOSED;
const ERR_OUT_OF_RANGE = errorsMod.codes.ERR_OUT_OF_RANGE;

const {
  ArrayFrom,
  ArrayPrototypePush,
  BigInt,
  DateNow,
  Error,
  ErrorCaptureStackTrace,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeSet,
  MathCeil,
  MathFloor,
  MathMax,
  MathMin,
  MathRandom,
  Number,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectFreeze,
  Promise: PrimordialPromise,
  PromisePrototypeThen,
  PromiseResolve,
  SafeMap,
  SafeSet,
  StringPrototypeEndsWith,
  StringPrototypeReplaceAll,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  SymbolAsyncDispose,
  SymbolAsyncIterator,
  SymbolDispose,
  TypeError,
} = primordials;

const { posix: pathPosix } = pathMod;
const {
  basename: pathBasename,
  dirname: pathDirname,
  join: pathJoin,
  normalize: pathNormalize,
} = pathPosix;

const {
  S_IFREG,
  S_IFDIR,
  S_IFLNK,
  O_APPEND,
  O_CREAT,
  O_EXCL,
  O_RDWR,
  O_TRUNC,
  O_WRONLY,
  R_OK,
  W_OK,
  X_OK,
  COPYFILE_EXCL,
  UV_DIRENT_FILE,
  UV_DIRENT_DIR,
  UV_DIRENT_LINK,
} = fsConstants;

// =====================================================================
// Errors
// =====================================================================

function errDescription(code) {
  switch (code) {
    case "ENOENT":
      return "no such file or directory";
    case "ENOTDIR":
      return "not a directory";
    case "ENOTEMPTY":
      return "directory not empty";
    case "EISDIR":
      return "illegal operation on a directory";
    case "EBADF":
      return "bad file descriptor";
    case "EEXIST":
      return "file already exists";
    case "EROFS":
      return "read-only file system";
    case "EINVAL":
      return "invalid argument";
    case "ELOOP":
      return "too many levels of symbolic links";
    case "EACCES":
      return "permission denied";
    default:
      return "unknown error";
  }
}

function makeFsError(code, syscall, path) {
  const msg = `${code}: ${errDescription(code)}, ${syscall}${
    path !== undefined ? ` '${path}'` : ""
  }`;
  const err = new Error(msg);
  err.code = code;
  err.syscall = syscall;
  if (path !== undefined) err.path = path;
  ErrorCaptureStackTrace(err, makeFsError);
  return err;
}

const createENOENT = (s, p) => makeFsError("ENOENT", s, p);
const createENOTDIR = (s, p) => makeFsError("ENOTDIR", s, p);
const createENOTEMPTY = (s, p) => makeFsError("ENOTEMPTY", s, p);
const createEISDIR = (s, p) => makeFsError("EISDIR", s, p);
const createEBADF = (s) => makeFsError("EBADF", s);
const createEEXIST = (s, p) => makeFsError("EEXIST", s, p);
const createEROFS = (s, p) => makeFsError("EROFS", s, p);
const createEINVAL = (s, p) => makeFsError("EINVAL", s, p);
const createELOOP = (s, p) => makeFsError("ELOOP", s, p);
const createEACCES = (s, p) => makeFsError("EACCES", s, p);

// =====================================================================
// Stats
// =====================================================================

// Distinctive device number for VFS files (0xVF5 = 4085).
const kVfsDev = 4085;
const kDefaultBlockSize = 4096;

let inoCounter = 1;
const kNsPerMs = 1_000_000n;

function buildStats(
  size,
  mode,
  nlink,
  blocks,
  options,
) {
  const now = DateNow();
  const uid = options?.uid ?? 0;
  const gid = options?.gid ?? 0;
  const atimeMs = options?.atimeMs ?? now;
  const mtimeMs = options?.mtimeMs ?? now;
  const ctimeMs = options?.ctimeMs ?? now;
  const birthtimeMs = options?.birthtimeMs ?? now;
  const ino = inoCounter++;

  if (options?.bigint) {
    return new BigIntStats(
      BigInt(kVfsDev),
      BigInt(mode),
      BigInt(nlink),
      BigInt(uid),
      BigInt(gid),
      0n,
      BigInt(kDefaultBlockSize),
      BigInt(ino),
      BigInt(size),
      BigInt(blocks),
      BigInt(MathFloor(atimeMs)) * kNsPerMs,
      BigInt(MathFloor(mtimeMs)) * kNsPerMs,
      BigInt(MathFloor(ctimeMs)) * kNsPerMs,
      BigInt(MathFloor(birthtimeMs)) * kNsPerMs,
    );
  }

  return new Stats(
    kVfsDev,
    mode,
    nlink,
    uid,
    gid,
    0,
    kDefaultBlockSize,
    ino,
    size,
    blocks,
    atimeMs,
    mtimeMs,
    ctimeMs,
    birthtimeMs,
  );
}

function createFileStats(size, options) {
  const mode = (options?.mode ?? 0o644) | S_IFREG;
  const nlink = options?.nlink ?? 1;
  const blocks = MathCeil(size / 512);
  return buildStats(size, mode, nlink, blocks, options);
}

function createDirectoryStats(options) {
  const mode = (options?.mode ?? 0o755) | S_IFDIR;
  return buildStats(kDefaultBlockSize, mode, 1, 8, options);
}

function createSymlinkStats(size, options) {
  const mode = (options?.mode ?? 0o777) | S_IFLNK;
  const blocks = MathCeil(size / 512);
  return buildStats(size, mode, 1, blocks, options);
}

function createZeroStats(options) {
  if (options?.bigint) {
    return new BigIntStats(
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
      0n,
    );
  }
  return new Stats(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
}

// =====================================================================
// File descriptor table
// =====================================================================

// VFS FDs use bit 30 set to avoid conflicts with real OS fds.
const VFS_FD_MASK = 0x40000000;
let nextFd = 0;

const openFDs = new SafeMap();

class VirtualFD {
  #fd;
  #entry;
  constructor(fd, entry) {
    this.#fd = fd;
    this.#entry = entry;
  }
  get fd() {
    return this.#fd;
  }
  get entry() {
    return this.#entry;
  }
}

function openVirtualFd(entry) {
  const fd = VFS_FD_MASK | nextFd++;
  MapPrototypeSet(openFDs, fd, new VirtualFD(fd, entry));
  return fd;
}

function getVirtualFd(fd) {
  return MapPrototypeGet(openFDs, fd);
}

function closeVirtualFd(fd) {
  return MapPrototypeDelete(openFDs, fd);
}

// =====================================================================
// VirtualFileHandle
// =====================================================================

class VirtualFileHandle {
  #path;
  #flags;
  #mode;
  #position;
  #closed;

  constructor(path, flags, mode) {
    this.#path = path;
    this.#flags = flags;
    this.#mode = mode ?? 0o644;
    this.#position = 0;
    this.#closed = false;
  }

  get path() {
    return this.#path;
  }
  get flags() {
    return this.#flags;
  }
  get mode() {
    return this.#mode;
  }
  get position() {
    return this.#position;
  }
  set position(pos) {
    this.#position = pos;
  }
  get closed() {
    return this.#closed;
  }

  _checkClosed(syscall) {
    if (this.#closed) throw createEBADF(syscall);
  }

  _markClosed() {
    this.#closed = true;
  }

  async read(_buffer, _offset, _length, _position) {
    this._checkClosed("read");
    throw new ERR_METHOD_NOT_IMPLEMENTED("read");
  }
  readSync(_buffer, _offset, _length, _position) {
    this._checkClosed("read");
    throw new ERR_METHOD_NOT_IMPLEMENTED("readSync");
  }
  async write(_buffer, _offset, _length, _position) {
    this._checkClosed("write");
    throw new ERR_METHOD_NOT_IMPLEMENTED("write");
  }
  writeSync(_buffer, _offset, _length, _position) {
    this._checkClosed("write");
    throw new ERR_METHOD_NOT_IMPLEMENTED("writeSync");
  }
  async readFile(_options) {
    this._checkClosed("read");
    throw new ERR_METHOD_NOT_IMPLEMENTED("readFile");
  }
  readFileSync(_options) {
    this._checkClosed("read");
    throw new ERR_METHOD_NOT_IMPLEMENTED("readFileSync");
  }
  async writeFile(_data, _options) {
    this._checkClosed("write");
    throw new ERR_METHOD_NOT_IMPLEMENTED("writeFile");
  }
  writeFileSync(_data, _options) {
    this._checkClosed("write");
    throw new ERR_METHOD_NOT_IMPLEMENTED("writeFileSync");
  }
  async stat(_options) {
    this._checkClosed("fstat");
    throw new ERR_METHOD_NOT_IMPLEMENTED("stat");
  }
  statSync(_options) {
    this._checkClosed("fstat");
    throw new ERR_METHOD_NOT_IMPLEMENTED("statSync");
  }
  async truncate(_len) {
    this._checkClosed("ftruncate");
    throw new ERR_METHOD_NOT_IMPLEMENTED("truncate");
  }
  truncateSync(_len) {
    this._checkClosed("ftruncate");
    throw new ERR_METHOD_NOT_IMPLEMENTED("truncateSync");
  }
  async chmod(_mode) {}
  async chown(_uid, _gid) {}
  async utimes(_atime, _mtime) {}
  async datasync() {}
  async sync() {}

  async readv(buffers, position) {
    this._checkClosed("readv");
    let totalRead = 0;
    for (let i = 0; i < buffers.length; i++) {
      const buf = buffers[i];
      const pos = position != null ? position + totalRead : null;
      const { bytesRead } = await this.read(buf, 0, buf.byteLength, pos);
      totalRead += bytesRead;
      if (bytesRead < buf.byteLength) break;
    }
    return { bytesRead: totalRead, buffers };
  }

  async writev(buffers, position) {
    this._checkClosed("writev");
    let totalWritten = 0;
    for (let i = 0; i < buffers.length; i++) {
      const buf = buffers[i];
      const pos = position != null ? position + totalWritten : null;
      const { bytesWritten } = await this.write(buf, 0, buf.byteLength, pos);
      totalWritten += bytesWritten;
      if (bytesWritten < buf.byteLength) break;
    }
    return { bytesWritten: totalWritten, buffers };
  }

  async appendFile(data, options) {
    this._checkClosed("appendFile");
    const buf = typeof data === "string"
      ? Buffer.from(data, options?.encoding)
      : data;
    await this.write(buf, 0, buf.length, null);
  }

  readableWebStream() {
    throw new ERR_METHOD_NOT_IMPLEMENTED("readableWebStream");
  }
  readLines() {
    throw new ERR_METHOD_NOT_IMPLEMENTED("readLines");
  }
  createReadStream() {
    throw new ERR_METHOD_NOT_IMPLEMENTED("createReadStream");
  }
  createWriteStream() {
    throw new ERR_METHOD_NOT_IMPLEMENTED("createWriteStream");
  }

  async close() {
    this._markClosed();
  }
  closeSync() {
    this._markClosed();
  }
}

VirtualFileHandle.prototype[SymbolAsyncDispose] =
  VirtualFileHandle.prototype.close;
VirtualFileHandle.prototype[SymbolDispose] =
  VirtualFileHandle.prototype.closeSync;

class MemoryFileHandle extends VirtualFileHandle {
  #content;
  #size;
  #entry;
  #getStats;

  constructor(path, flags, mode, content, entry, getStats) {
    super(path, flags, mode);
    this.#content = content;
    this.#size = content.length;
    this.#entry = entry;
    this.#getStats = getStats;

    if (
      flags === "w" || flags === "w+" || flags === "wx" || flags === "wx+"
    ) {
      this.#content = Buffer.alloc(0);
      this.#size = 0;
      if (entry) entry.content = this.#content;
    } else if (
      flags === "a" || flags === "a+" || flags === "ax" || flags === "ax+"
    ) {
      this.position = this.#size;
    }
  }

  #ensureOpen(syscall) {
    if (this.closed) throw createEBADF(syscall);
  }
  #checkWritable() {
    if (this.flags === "r") throw createEBADF("write");
  }
  #checkReadable() {
    const f = this.flags;
    if (f === "w" || f === "a" || f === "wx" || f === "ax") {
      throw createEBADF("read");
    }
  }
  #isAppend() {
    const f = this.flags;
    return f === "a" || f === "a+" || f === "ax" || f === "ax+";
  }

  get content() {
    return this.#content.subarray(0, this.#size);
  }

  readSync(buffer, offset, length, position) {
    this.#ensureOpen("read");
    this.#checkReadable();
    const content = this.content;
    const readPos = position !== null && position !== undefined
      ? Number(position)
      : this.position;
    const available = content.length - readPos;
    if (available <= 0) return 0;
    const bytesToRead = MathMin(length, available);
    content.copy(buffer, offset, readPos, readPos + bytesToRead);
    if (position === null || position === undefined) {
      this.position = readPos + bytesToRead;
    }
    return bytesToRead;
  }

  async read(buffer, offset, length, position) {
    const bytesRead = this.readSync(buffer, offset, length, position);
    return { bytesRead, buffer };
  }

  writeSync(buffer, offset, length, position) {
    this.#ensureOpen("write");
    this.#checkWritable();
    const writePos = this.#isAppend()
      ? this.#size
      : (position !== null && position !== undefined
        ? Number(position)
        : this.position);
    const data = buffer.subarray(offset, offset + length);
    const neededSize = writePos + length;
    if (neededSize > this.#content.length) {
      const newCapacity = MathMax(neededSize, this.#content.length * 2);
      const newContent = Buffer.alloc(newCapacity);
      this.#content.copy(newContent, 0, 0, this.#size);
      this.#content = newContent;
    }
    data.copy(this.#content, writePos);
    if (neededSize > this.#size) this.#size = neededSize;
    if (this.#entry) {
      const now = DateNow();
      this.#entry.content = this.#content.subarray(0, this.#size);
      this.#entry.mtime = now;
      this.#entry.ctime = now;
    }
    if (position === null || position === undefined) {
      this.position = writePos + length;
    }
    return length;
  }

  async write(buffer, offset, length, position) {
    const bytesWritten = this.writeSync(buffer, offset, length, position);
    return { bytesWritten, buffer };
  }

  readFileSync(options) {
    this.#ensureOpen("read");
    this.#checkReadable();
    const content = this.content;
    const encoding = typeof options === "string" ? options : options?.encoding;
    if (encoding) return content.toString(encoding);
    return Buffer.from(content);
  }

  async readFile(options) {
    return this.readFileSync(options);
  }

  writeFileSync(data, options) {
    this.#ensureOpen("write");
    this.#checkWritable();
    const buf = typeof data === "string"
      ? Buffer.from(data, options?.encoding)
      : data;
    if (this.#isAppend()) {
      const neededSize = this.#size + buf.length;
      if (neededSize > this.#content.length) {
        const newCapacity = MathMax(neededSize, this.#content.length * 2);
        const newContent = Buffer.alloc(newCapacity);
        this.#content.copy(newContent, 0, 0, this.#size);
        this.#content = newContent;
      }
      buf.copy(this.#content, this.#size);
      this.#size = neededSize;
    } else {
      this.#content = Buffer.from(buf);
      this.#size = buf.length;
    }
    if (this.#entry) {
      const now = DateNow();
      this.#entry.content = this.#content.subarray(0, this.#size);
      this.#entry.mtime = now;
      this.#entry.ctime = now;
    }
    this.position = this.#size;
  }

  async writeFile(data, options) {
    this.writeFileSync(data, options);
  }

  statSync(_options) {
    this.#ensureOpen("fstat");
    if (this.#getStats) return this.#getStats(this.#size);
    throw new ERR_INVALID_STATE("stats not available");
  }

  async stat(options) {
    return this.statSync(options);
  }

  truncateSync(len = 0) {
    this.#ensureOpen("ftruncate");
    this.#checkWritable();
    if (len < 0) len = 0;
    if (len < this.#size) {
      this.#content.fill(0, len, this.#size);
      this.#size = len;
    } else if (len > this.#size) {
      if (len > this.#content.length) {
        const newContent = Buffer.alloc(len);
        this.#content.copy(newContent, 0, 0, this.#size);
        this.#content = newContent;
      } else {
        this.#content.fill(0, this.#size, len);
      }
      this.#size = len;
    }
    if (this.#entry) {
      const now = DateNow();
      this.#entry.content = this.#content.subarray(0, this.#size);
      this.#entry.mtime = now;
      this.#entry.ctime = now;
    }
  }

  async truncate(len) {
    this.truncateSync(len);
  }
}

// =====================================================================
// VirtualProvider
// =====================================================================

class VirtualProvider {
  get readonly() {
    return false;
  }
  get supportsSymlinks() {
    return false;
  }
  get supportsWatch() {
    return false;
  }

  async open(_path, _flags, _mode) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("open");
  }
  openSync(_path, _flags, _mode) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("openSync");
  }
  async stat(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("stat");
  }
  statSync(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("statSync");
  }
  async lstat(path, options) {
    return this.stat(path, options);
  }
  lstatSync(path, options) {
    return this.statSync(path, options);
  }
  async readdir(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("readdir");
  }
  readdirSync(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("readdirSync");
  }
  async mkdir(path, _options) {
    if (this.readonly) throw createEROFS("mkdir", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("mkdir");
  }
  mkdirSync(path, _options) {
    if (this.readonly) throw createEROFS("mkdir", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("mkdirSync");
  }
  async rmdir(path) {
    if (this.readonly) throw createEROFS("rmdir", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("rmdir");
  }
  rmdirSync(path) {
    if (this.readonly) throw createEROFS("rmdir", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("rmdirSync");
  }
  async unlink(path) {
    if (this.readonly) throw createEROFS("unlink", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("unlink");
  }
  unlinkSync(path) {
    if (this.readonly) throw createEROFS("unlink", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("unlinkSync");
  }
  async rename(oldPath, _newPath) {
    if (this.readonly) throw createEROFS("rename", oldPath);
    throw new ERR_METHOD_NOT_IMPLEMENTED("rename");
  }
  renameSync(oldPath, _newPath) {
    if (this.readonly) throw createEROFS("rename", oldPath);
    throw new ERR_METHOD_NOT_IMPLEMENTED("renameSync");
  }

  async readFile(path, options) {
    const flag =
      (typeof options === "object" && options !== null && options.flag) ||
      "r";
    const handle = await this.open(path, flag);
    try {
      return await handle.readFile(options);
    } finally {
      await handle.close();
    }
  }
  readFileSync(path, options) {
    const flag =
      (typeof options === "object" && options !== null && options.flag) ||
      "r";
    const handle = this.openSync(path, flag);
    try {
      return handle.readFileSync(options);
    } finally {
      handle.closeSync();
    }
  }
  async writeFile(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const flag = options?.flag ?? "w";
    const handle = await this.open(path, flag, options?.mode);
    try {
      await handle.writeFile(data, options);
    } finally {
      await handle.close();
    }
  }
  writeFileSync(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const flag = options?.flag ?? "w";
    const handle = this.openSync(path, flag, options?.mode);
    try {
      handle.writeFileSync(data, options);
    } finally {
      handle.closeSync();
    }
  }
  async appendFile(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const flag = options?.flag ?? "a";
    const handle = await this.open(path, flag, options?.mode);
    try {
      await handle.writeFile(data, options);
    } finally {
      await handle.close();
    }
  }
  appendFileSync(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const flag = options?.flag ?? "a";
    const handle = this.openSync(path, flag, options?.mode);
    try {
      handle.writeFileSync(data, options);
    } finally {
      handle.closeSync();
    }
  }

  async exists(path) {
    try {
      await this.stat(path);
      return true;
    } catch {
      return false;
    }
  }
  existsSync(path) {
    try {
      this.statSync(path);
      return true;
    } catch {
      return false;
    }
  }

  async copyFile(src, dest, mode) {
    if (this.readonly) throw createEROFS("copyfile", dest);
    if ((mode & COPYFILE_EXCL) !== 0) {
      if (await this.exists(dest)) throw createEEXIST("copyfile", dest);
    }
    const content = await this.readFile(src);
    await this.writeFile(dest, content);
  }
  copyFileSync(src, dest, mode) {
    if (this.readonly) throw createEROFS("copyfile", dest);
    if ((mode & COPYFILE_EXCL) !== 0) {
      if (this.existsSync(dest)) throw createEEXIST("copyfile", dest);
    }
    const content = this.readFileSync(src);
    this.writeFileSync(dest, content);
  }

  async realpath(path, _options) {
    await this.stat(path);
    return path;
  }
  realpathSync(path, _options) {
    this.statSync(path);
    return path;
  }

  async access(path, mode) {
    const stats = await this.stat(path);
    this._checkAccessMode(path, stats, mode);
  }
  accessSync(path, mode) {
    const stats = this.statSync(path);
    this._checkAccessMode(path, stats, mode);
  }

  _checkAccessMode(path, stats, mode) {
    if (mode == null || mode === 0) return;
    const fileMode = stats.mode & 0o777;
    if ((mode & R_OK) !== 0 && (fileMode & 0o400) === 0) {
      throw createEACCES("access", path);
    }
    if ((mode & W_OK) !== 0 && (fileMode & 0o200) === 0) {
      throw createEACCES("access", path);
    }
    if ((mode & X_OK) !== 0 && (fileMode & 0o100) === 0) {
      throw createEACCES("access", path);
    }
  }

  async link(_existingPath, newPath) {
    if (this.readonly) throw createEROFS("link", newPath);
    throw new ERR_METHOD_NOT_IMPLEMENTED("link");
  }
  linkSync(_existingPath, newPath) {
    if (this.readonly) throw createEROFS("link", newPath);
    throw new ERR_METHOD_NOT_IMPLEMENTED("linkSync");
  }

  async readlink(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("readlink");
  }
  readlinkSync(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("readlinkSync");
  }
  async symlink(_target, path, _type) {
    if (this.readonly) throw createEROFS("symlink", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("symlink");
  }
  symlinkSync(_target, path, _type) {
    if (this.readonly) throw createEROFS("symlink", path);
    throw new ERR_METHOD_NOT_IMPLEMENTED("symlinkSync");
  }

  watch(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("watch");
  }
  watchAsync(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("watchAsync");
  }
  watchFile(_path, _options) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("watchFile");
  }
  unwatchFile(_path, _listener) {
    throw new ERR_METHOD_NOT_IMPLEMENTED("unwatchFile");
  }
}

// =====================================================================
// MemoryProvider
// =====================================================================

const TYPE_FILE = 0;
const TYPE_DIR = 1;
const TYPE_SYMLINK = 2;
const kMaxSymlinkDepth = 40;

class MemoryEntry {
  constructor(type, options) {
    this.type = type;
    this.mode = options?.mode ??
      (type === TYPE_DIR ? 0o755 : type === TYPE_SYMLINK ? 0o777 : 0o644);
    this.content = null;
    this.target = null;
    this.children = null;
    this.nlink = 1;
    this.uid = 0;
    this.gid = 0;
    const now = DateNow();
    this.atime = now;
    this.mtime = now;
    this.ctime = now;
    this.birthtime = now;
  }
  isFile() {
    return this.type === TYPE_FILE;
  }
  isDirectory() {
    return this.type === TYPE_DIR;
  }
  isSymbolicLink() {
    return this.type === TYPE_SYMLINK;
  }
}

function normalizeFlags(flags) {
  if (typeof flags === "string") return flags;
  if (typeof flags !== "number") return "r";
  const rdwr = (flags & O_RDWR) !== 0;
  const append = (flags & O_APPEND) !== 0;
  const excl = (flags & O_EXCL) !== 0;
  const write = (flags & O_WRONLY) !== 0 ||
    (flags & O_CREAT) !== 0 ||
    (flags & O_TRUNC) !== 0;
  if (append) return "a" + (excl ? "x" : "") + (rdwr ? "+" : "");
  if (write) return "w" + (excl ? "x" : "") + (rdwr ? "+" : "");
  if (rdwr) return "r+";
  return "r";
}

function toMs(time) {
  if (typeof time === "number") return time * 1000;
  if (typeof time === "string") return DateNow();
  if (typeof time === "object" && time !== null) return +time;
  return time;
}

class MemoryProvider extends VirtualProvider {
  #root;
  #readonly;
  #statWatchers;

  constructor() {
    super();
    this.#root = new MemoryEntry(TYPE_DIR);
    this.#root.children = new SafeMap();
    this.#readonly = false;
    this.#statWatchers = new SafeMap();
  }

  get readonly() {
    return this.#readonly;
  }
  get supportsSymlinks() {
    return true;
  }
  get supportsWatch() {
    return true;
  }

  setReadOnly() {
    this.#readonly = true;
  }

  #normalizePath(path) {
    let normalized = StringPrototypeReplaceAll(path, "\\", "/");
    if (normalized[0] !== "/") normalized = "/" + normalized;
    return pathNormalize(normalized);
  }

  #splitPath(path) {
    if (path === "/") return [];
    return StringPrototypeSlice(path, 1).split("/");
  }

  #resolveSymlinkTarget(symlinkPath, target) {
    if (StringPrototypeStartsWith(target, "/")) {
      return this.#normalizePath(target);
    }
    const parent = pathDirname(symlinkPath);
    return this.#normalizePath(pathJoin(parent, target));
  }

  #lookupEntry(path, followSymlinks = true, depth = 0) {
    const normalized = this.#normalizePath(path);
    if (normalized === "/") {
      return { entry: this.#root, resolvedPath: "/" };
    }
    const segments = this.#splitPath(normalized);
    let current = this.#root;
    let currentPath = "/";
    for (let i = 0; i < segments.length; i++) {
      const segment = segments[i];
      if (current.isSymbolicLink()) {
        if (depth >= kMaxSymlinkDepth) {
          return { entry: null, resolvedPath: null, eloop: true };
        }
        const targetPath = this.#resolveSymlinkTarget(
          currentPath,
          current.target,
        );
        const r = this.#lookupEntry(targetPath, true, depth + 1);
        if (r.eloop) return r;
        if (!r.entry) return { entry: null, resolvedPath: null };
        current = r.entry;
        currentPath = r.resolvedPath;
      }
      if (!current.isDirectory()) {
        return { entry: null, resolvedPath: null };
      }
      const child = MapPrototypeGet(current.children, segment);
      if (!child) return { entry: null, resolvedPath: null };
      currentPath = pathJoin(currentPath, segment);
      current = child;
    }
    if (current.isSymbolicLink() && followSymlinks) {
      if (depth >= kMaxSymlinkDepth) {
        return { entry: null, resolvedPath: null, eloop: true };
      }
      const targetPath = this.#resolveSymlinkTarget(
        currentPath,
        current.target,
      );
      return this.#lookupEntry(targetPath, true, depth + 1);
    }
    return { entry: current, resolvedPath: currentPath };
  }

  #getEntry(path, syscall, followSymlinks = true) {
    const r = this.#lookupEntry(path, followSymlinks);
    if (r.eloop) throw createELOOP(syscall, path);
    if (!r.entry) throw createENOENT(syscall, path);
    return r.entry;
  }

  #ensureParent(path, create, syscall) {
    if (path === "/") return this.#root;
    const parentPath = pathDirname(path);
    const segments = this.#splitPath(parentPath);
    let current = this.#root;
    for (let i = 0; i < segments.length; i++) {
      const segment = segments[i];
      const currentPath = pathJoin("/", ...segments.slice(0, i));
      if (current.isSymbolicLink()) {
        const targetPath = this.#resolveSymlinkTarget(
          currentPath,
          current.target,
        );
        const r = this.#lookupEntry(targetPath, true, 0);
        if (!r.entry) throw createENOENT(syscall, path);
        current = r.entry;
      }
      if (!current.isDirectory()) throw createENOTDIR(syscall, path);
      let entry = MapPrototypeGet(current.children, segment);
      if (!entry) {
        if (create) {
          entry = new MemoryEntry(TYPE_DIR);
          entry.children = new SafeMap();
          MapPrototypeSet(current.children, segment, entry);
        } else {
          throw createENOENT(syscall, path);
        }
      }
      current = entry;
    }
    if (current.isSymbolicLink()) {
      const targetPath = this.#resolveSymlinkTarget(parentPath, current.target);
      const r = this.#lookupEntry(targetPath, true, 0);
      if (!r.entry) throw createENOENT(syscall, path);
      current = r.entry;
    }
    if (!current.isDirectory()) throw createENOTDIR(syscall, path);
    return current;
  }

  #createStats(entry, size, bigint) {
    const opts = {
      mode: entry.mode,
      nlink: entry.nlink,
      uid: entry.uid,
      gid: entry.gid,
      atimeMs: entry.atime,
      mtimeMs: entry.mtime,
      ctimeMs: entry.ctime,
      birthtimeMs: entry.birthtime,
      bigint,
    };
    if (entry.isFile()) {
      return createFileStats(size ?? entry.content.length, opts);
    }
    if (entry.isDirectory()) return createDirectoryStats(opts);
    if (entry.isSymbolicLink()) {
      return createSymlinkStats(entry.target.length, opts);
    }
    throw new ERR_INVALID_STATE("Unknown entry type");
  }

  openSync(path, flags, mode) {
    const normalized = this.#normalizePath(path);
    flags = normalizeFlags(flags);
    const isCreate = flags === "w" || flags === "w+" ||
      flags === "a" || flags === "a+" ||
      flags === "wx" || flags === "wx+" ||
      flags === "ax" || flags === "ax+";
    const isExclusive = flags === "wx" || flags === "wx+" ||
      flags === "ax" || flags === "ax+";
    const isWritable = flags !== "r";
    if (this.readonly && isWritable) throw createEROFS("open", path);

    let entry;
    try {
      entry = this.#getEntry(normalized, "open");
      if (isExclusive) throw createEEXIST("open", path);
    } catch (err) {
      if (err.code !== "ENOENT" || !isCreate) throw err;
      const parent = this.#ensureParent(normalized, false, "open");
      const name = pathBasename(normalized);
      entry = new MemoryEntry(TYPE_FILE, { mode });
      entry.content = Buffer.alloc(0);
      MapPrototypeSet(parent.children, name, entry);
      const now = DateNow();
      parent.mtime = now;
      parent.ctime = now;
    }
    if (entry.isDirectory()) throw createEISDIR("open", path);
    if (entry.isSymbolicLink()) throw createEINVAL("open", path);

    const getStats = (size) => this.#createStats(entry, size);
    return new MemoryFileHandle(
      normalized,
      flags,
      mode ?? entry.mode,
      entry.content,
      entry,
      getStats,
    );
  }
  async open(path, flags, mode) {
    return this.openSync(path, flags, mode);
  }

  statSync(path, options) {
    const entry = this.#getEntry(path, "stat", true);
    return this.#createStats(entry, undefined, options?.bigint);
  }
  async stat(path, options) {
    return this.statSync(path, options);
  }
  lstatSync(path, options) {
    const entry = this.#getEntry(path, "lstat", false);
    return this.#createStats(entry, undefined, options?.bigint);
  }
  async lstat(path, options) {
    return this.lstatSync(path, options);
  }

  readdirSync(path, options) {
    const entry = this.#getEntry(path, "scandir", true);
    if (!entry.isDirectory()) throw createENOTDIR("scandir", path);
    const normalized = this.#normalizePath(path);
    const withFileTypes = options?.withFileTypes === true;
    const recursive = options?.recursive === true;

    if (recursive) {
      return this.#readdirRecursive(entry, normalized, withFileTypes);
    }

    if (withFileTypes) {
      const dirents = [];
      for (const { 0: name, 1: child } of entry.children) {
        let type;
        if (child.isSymbolicLink()) type = UV_DIRENT_LINK;
        else if (child.isDirectory()) type = UV_DIRENT_DIR;
        else type = UV_DIRENT_FILE;
        ArrayPrototypePush(dirents, new Dirent(name, type, normalized));
      }
      return dirents;
    }
    return ArrayFrom(entry.children.keys());
  }

  #readdirRecursive(dirEntry, dirPath, withFileTypes) {
    const results = [];
    const walk = (entry, currentPath, relativePath) => {
      for (const { 0: name, 1: child } of entry.children) {
        const childRel = relativePath ? relativePath + "/" + name : name;
        if (withFileTypes) {
          let type;
          if (child.isSymbolicLink()) type = UV_DIRENT_LINK;
          else if (child.isDirectory()) type = UV_DIRENT_DIR;
          else type = UV_DIRENT_FILE;
          ArrayPrototypePush(results, new Dirent(childRel, type, dirPath));
        } else {
          ArrayPrototypePush(results, childRel);
        }
        let resolved = child;
        if (child.isSymbolicLink()) {
          const targetPath = this.#resolveSymlinkTarget(
            pathJoin(currentPath, name),
            child.target,
          );
          const r = this.#lookupEntry(targetPath, true, 0);
          if (r.entry) resolved = r.entry;
        }
        if (resolved.isDirectory()) {
          walk(resolved, pathJoin(currentPath, name), childRel);
        }
      }
    };
    walk(dirEntry, dirPath, "");
    return results;
  }

  async readdir(path, options) {
    return this.readdirSync(path, options);
  }

  mkdirSync(path, options) {
    if (this.readonly) throw createEROFS("mkdir", path);
    const normalized = this.#normalizePath(path);
    const recursive = options?.recursive === true;
    const existing = this.#lookupEntry(normalized, true);
    if (existing.entry) {
      if (existing.entry.isDirectory() && recursive) return undefined;
      throw createEEXIST("mkdir", path);
    }
    if (recursive) {
      const segments = this.#splitPath(normalized);
      let current = this.#root;
      let currentPath = "/";
      let firstCreated;
      for (const segment of segments) {
        currentPath = pathJoin(currentPath, segment);
        let entry = MapPrototypeGet(current.children, segment);
        if (!entry) {
          entry = new MemoryEntry(TYPE_DIR, { mode: options?.mode });
          entry.children = new SafeMap();
          MapPrototypeSet(current.children, segment, entry);
          if (firstCreated === undefined) firstCreated = currentPath;
        } else if (!entry.isDirectory()) {
          throw createENOTDIR("mkdir", path);
        }
        current = entry;
      }
      return firstCreated;
    }
    const parent = this.#ensureParent(normalized, false, "mkdir");
    const name = pathBasename(normalized);
    const entry = new MemoryEntry(TYPE_DIR, { mode: options?.mode });
    entry.children = new SafeMap();
    MapPrototypeSet(parent.children, name, entry);
    const now = DateNow();
    parent.mtime = now;
    parent.ctime = now;
    return undefined;
  }
  async mkdir(path, options) {
    return this.mkdirSync(path, options);
  }

  rmdirSync(path) {
    if (this.readonly) throw createEROFS("rmdir", path);
    const normalized = this.#normalizePath(path);
    const entry = this.#getEntry(normalized, "rmdir", false);
    if (!entry.isDirectory()) throw createENOTDIR("rmdir", path);
    if (entry.children.size > 0) throw createENOTEMPTY("rmdir", path);
    const parent = this.#ensureParent(normalized, false, "rmdir");
    const name = pathBasename(normalized);
    MapPrototypeDelete(parent.children, name);
    const now = DateNow();
    parent.mtime = now;
    parent.ctime = now;
  }
  async rmdir(path) {
    this.rmdirSync(path);
  }

  unlinkSync(path) {
    if (this.readonly) throw createEROFS("unlink", path);
    const normalized = this.#normalizePath(path);
    const entry = this.#getEntry(normalized, "unlink", false);
    if (entry.isDirectory()) throw createEISDIR("unlink", path);
    const parent = this.#ensureParent(normalized, false, "unlink");
    const name = pathBasename(normalized);
    MapPrototypeDelete(parent.children, name);
    entry.nlink--;
    const now = DateNow();
    parent.mtime = now;
    parent.ctime = now;
  }
  async unlink(path) {
    this.unlinkSync(path);
  }

  renameSync(oldPath, newPath) {
    if (this.readonly) throw createEROFS("rename", oldPath);
    const oldNorm = this.#normalizePath(oldPath);
    const newNorm = this.#normalizePath(newPath);
    const entry = this.#getEntry(oldNorm, "rename", false);
    const newParent = this.#ensureParent(newNorm, false, "rename");
    const newName = pathBasename(newNorm);
    const existingDest = MapPrototypeGet(newParent.children, newName);
    if (existingDest) {
      if (existingDest.isDirectory() && !entry.isDirectory()) {
        throw createEISDIR("rename", newPath);
      }
      if (!existingDest.isDirectory() && entry.isDirectory()) {
        throw createENOTDIR("rename", newPath);
      }
    }
    const oldParent = this.#ensureParent(oldNorm, false, "rename");
    const oldName = pathBasename(oldNorm);
    MapPrototypeDelete(oldParent.children, oldName);
    MapPrototypeSet(newParent.children, newName, entry);
    const now = DateNow();
    oldParent.mtime = now;
    oldParent.ctime = now;
    if (newParent !== oldParent) {
      newParent.mtime = now;
      newParent.ctime = now;
    }
  }
  async rename(oldPath, newPath) {
    this.renameSync(oldPath, newPath);
  }

  linkSync(existingPath, newPath) {
    if (this.readonly) throw createEROFS("link", newPath);
    const oldNorm = this.#normalizePath(existingPath);
    const newNorm = this.#normalizePath(newPath);
    const entry = this.#getEntry(oldNorm, "link", true);
    if (!entry.isFile()) throw createEINVAL("link", existingPath);
    const existing = this.#lookupEntry(newNorm, false);
    if (existing.entry) throw createEEXIST("link", newPath);
    const parent = this.#ensureParent(newNorm, false, "link");
    const name = pathBasename(newNorm);
    MapPrototypeSet(parent.children, name, entry);
    entry.nlink++;
    const now = DateNow();
    parent.mtime = now;
    parent.ctime = now;
  }
  async link(existingPath, newPath) {
    this.linkSync(existingPath, newPath);
  }

  readlinkSync(path, _options) {
    const normalized = this.#normalizePath(path);
    const entry = this.#getEntry(normalized, "readlink", false);
    if (!entry.isSymbolicLink()) throw createEINVAL("readlink", path);
    return entry.target;
  }
  async readlink(path, options) {
    return this.readlinkSync(path, options);
  }

  symlinkSync(target, path, _type) {
    if (this.readonly) throw createEROFS("symlink", path);
    const normalized = this.#normalizePath(path);
    const existing = this.#lookupEntry(normalized, false);
    if (existing.entry) throw createEEXIST("symlink", path);
    const parent = this.#ensureParent(normalized, false, "symlink");
    const name = pathBasename(normalized);
    const entry = new MemoryEntry(TYPE_SYMLINK);
    entry.target = target;
    MapPrototypeSet(parent.children, name, entry);
    const now = DateNow();
    parent.mtime = now;
    parent.ctime = now;
  }
  async symlink(target, path, type) {
    this.symlinkSync(target, path, type);
  }

  realpathSync(path, _options) {
    const r = this.#lookupEntry(path, true, 0);
    if (r.eloop) throw createELOOP("realpath", path);
    if (!r.entry) throw createENOENT("realpath", path);
    return r.resolvedPath;
  }
  async realpath(path, options) {
    return this.realpathSync(path, options);
  }

  chmodSync(path, mode) {
    const entry = this.#getEntry(path, "chmod", true);
    entry.mode = (entry.mode & ~0o7777) | (mode & 0o7777);
    entry.ctime = DateNow();
  }
  chownSync(path, uid, gid) {
    const entry = this.#getEntry(path, "chown", true);
    if (uid >= 0) entry.uid = uid;
    if (gid >= 0) entry.gid = gid;
    entry.ctime = DateNow();
  }
  utimesSync(path, atime, mtime) {
    const entry = this.#getEntry(path, "utime", true);
    entry.atime = toMs(atime);
    entry.mtime = toMs(mtime);
    entry.ctime = DateNow();
  }
  lutimesSync(path, atime, mtime) {
    const entry = this.#getEntry(path, "utime", false);
    entry.atime = toMs(atime);
    entry.mtime = toMs(mtime);
    entry.ctime = DateNow();
  }

  watch(path, options) {
    return new VFSWatcher(this, this.#normalizePath(path), options);
  }
  watchAsync(path, options) {
    return new VFSWatchAsyncIterable(this, this.#normalizePath(path), options);
  }
  watchFile(path, options, listener) {
    const normalized = this.#normalizePath(path);
    let watcher = MapPrototypeGet(this.#statWatchers, normalized);
    if (!watcher) {
      watcher = new VFSStatWatcher(this, normalized, options);
      MapPrototypeSet(this.#statWatchers, normalized, watcher);
    }
    if (listener) watcher.addListener("change", listener);
    return watcher;
  }
  unwatchFile(path, listener) {
    const normalized = this.#normalizePath(path);
    const watcher = MapPrototypeGet(this.#statWatchers, normalized);
    if (!watcher) return;
    if (listener) watcher.removeListener("change", listener);
    else watcher.removeAllListeners("change");
    if (watcher.hasNoListeners()) {
      watcher.stop();
      MapPrototypeDelete(this.#statWatchers, normalized);
    }
  }
}

// =====================================================================
// RealFSProvider
// =====================================================================

class RealFileHandle extends VirtualFileHandle {
  #fd;
  #realPath;

  constructor(path, flags, mode, fd, realPath) {
    super(path, flags, mode);
    this.#fd = fd;
    this.#realPath = realPath;
  }

  #ensureOpen(syscall) {
    if (this.closed) throw createEBADF(syscall);
  }

  readSync(buffer, offset, length, position) {
    this.#ensureOpen("read");
    return lazyNodeFs().readSync(this.#fd, buffer, offset, length, position);
  }
  async read(buffer, offset, length, position) {
    this.#ensureOpen("read");
    const nodeFs = lazyNodeFs();
    return new PrimordialPromise((resolve, reject) => {
      nodeFs.read(
        this.#fd,
        buffer,
        offset,
        length,
        position,
        (err, bytesRead) => {
          if (err) reject(err);
          else resolve({ bytesRead, buffer });
        },
      );
    });
  }
  writeSync(buffer, offset, length, position) {
    this.#ensureOpen("write");
    return lazyNodeFs().writeSync(this.#fd, buffer, offset, length, position);
  }
  async write(buffer, offset, length, position) {
    this.#ensureOpen("write");
    const nodeFs = lazyNodeFs();
    return new PrimordialPromise((resolve, reject) => {
      nodeFs.write(
        this.#fd,
        buffer,
        offset,
        length,
        position,
        (err, bytesWritten) => {
          if (err) reject(err);
          else resolve({ bytesWritten, buffer });
        },
      );
    });
  }
  readFileSync(options) {
    this.#ensureOpen("read");
    return lazyNodeFs().readFileSync(this.#realPath, options);
  }
  async readFile(options) {
    this.#ensureOpen("read");
    return lazyNodeFs().promises.readFile(this.#realPath, options);
  }
  writeFileSync(data, options) {
    this.#ensureOpen("write");
    lazyNodeFs().writeFileSync(this.#realPath, data, options);
  }
  async writeFile(data, options) {
    this.#ensureOpen("write");
    return lazyNodeFs().promises.writeFile(this.#realPath, data, options);
  }
  statSync(options) {
    this.#ensureOpen("fstat");
    return lazyNodeFs().fstatSync(this.#fd, options);
  }
  async stat(options) {
    this.#ensureOpen("fstat");
    const nodeFs = lazyNodeFs();
    return new PrimordialPromise((resolve, reject) => {
      nodeFs.fstat(this.#fd, options, (err, stats) => {
        if (err) reject(err);
        else resolve(stats);
      });
    });
  }
  truncateSync(len = 0) {
    this.#ensureOpen("ftruncate");
    lazyNodeFs().ftruncateSync(this.#fd, len);
  }
  async truncate(len = 0) {
    this.#ensureOpen("ftruncate");
    const nodeFs = lazyNodeFs();
    return new PrimordialPromise((resolve, reject) => {
      nodeFs.ftruncate(this.#fd, len, (err) => {
        if (err) reject(err);
        else resolve();
      });
    });
  }
  closeSync() {
    if (!this.closed) {
      lazyNodeFs().closeSync(this.#fd);
      super.closeSync();
    }
  }
  async close() {
    if (!this.closed) {
      const nodeFs = lazyNodeFs();
      const fd = this.#fd;
      await new PrimordialPromise((resolve, reject) => {
        nodeFs.close(fd, (err) => {
          if (err) reject(err);
          else resolve();
        });
      });
      super.closeSync();
    }
  }
}

class RealFSProvider extends VirtualProvider {
  #rootPath;

  constructor(rootPath) {
    super();
    if (typeof rootPath !== "string" || rootPath.length === 0) {
      throw new TypeError(
        'The "rootPath" argument must be of type string. ' +
          `Received ${rootPath === null ? "null" : typeof rootPath}`,
      );
    }
    // Use platform path module on disk; normalize/resolve via pathMod.default
    this.#rootPath = pathMod.resolve(rootPath);
    ObjectDefineProperty(this, "readonly", {
      value: false,
      writable: false,
      enumerable: false,
      configurable: true,
    });
    ObjectDefineProperty(this, "supportsSymlinks", {
      value: true,
      writable: false,
      enumerable: false,
      configurable: true,
    });
  }

  get rootPath() {
    return this.#rootPath;
  }
  get supportsWatch() {
    return true;
  }

  #resolvePath(vfsPath, followSymlinks = true) {
    let normalized = vfsPath;
    if (StringPrototypeStartsWith(normalized, "/")) {
      normalized = StringPrototypeSlice(normalized, 1);
    }
    const realPath = pathMod.resolve(this.#rootPath, normalized);
    const sep = pathMod.sep;
    const rootWithSep = StringPrototypeEndsWith(this.#rootPath, sep)
      ? this.#rootPath
      : this.#rootPath + sep;
    if (
      realPath !== this.#rootPath &&
      !StringPrototypeStartsWith(realPath, rootWithSep)
    ) {
      throw createENOENT("open", vfsPath);
    }
    if (followSymlinks) {
      try {
        const resolved = lazyNodeFs().realpathSync(realPath);
        if (
          resolved !== this.#rootPath &&
          !StringPrototypeStartsWith(resolved, rootWithSep)
        ) {
          throw createENOENT("open", vfsPath);
        }
        return resolved;
      } catch (err) {
        if (err?.code !== "ENOENT") throw err;
        this.#verifyAncestorInRoot(realPath, rootWithSep, vfsPath);
        return realPath;
      }
    }
    this.#verifyAncestorInRoot(realPath, rootWithSep, vfsPath);
    return realPath;
  }

  #verifyAncestorInRoot(realPath, rootWithSep, vfsPath) {
    const nodeFs = lazyNodeFs();
    let current = pathMod.dirname(realPath);
    while (current.length >= this.#rootPath.length) {
      try {
        const resolved = nodeFs.realpathSync(current);
        if (
          resolved !== this.#rootPath &&
          !StringPrototypeStartsWith(resolved, rootWithSep)
        ) {
          throw createENOENT("open", vfsPath);
        }
        return;
      } catch (err) {
        if (err?.code !== "ENOENT") throw err;
        current = pathMod.dirname(current);
      }
    }
  }

  openSync(vfsPath, flags, mode) {
    const realPath = this.#resolvePath(vfsPath);
    const fd = lazyNodeFs().openSync(realPath, flags, mode);
    return new RealFileHandle(vfsPath, flags, mode ?? 0o644, fd, realPath);
  }
  async open(vfsPath, flags, mode) {
    const realPath = this.#resolvePath(vfsPath);
    const nodeFs = lazyNodeFs();
    return new PrimordialPromise((resolve, reject) => {
      nodeFs.open(realPath, flags, mode, (err, fd) => {
        if (err) reject(err);
        else {
          resolve(
            new RealFileHandle(vfsPath, flags, mode ?? 0o644, fd, realPath),
          );
        }
      });
    });
  }
  statSync(vfsPath, options) {
    return lazyNodeFs().statSync(this.#resolvePath(vfsPath), options);
  }
  async stat(vfsPath, options) {
    return lazyNodeFs().promises.stat(this.#resolvePath(vfsPath), options);
  }
  lstatSync(vfsPath, options) {
    return lazyNodeFs().lstatSync(this.#resolvePath(vfsPath, false), options);
  }
  async lstat(vfsPath, options) {
    return lazyNodeFs().promises.lstat(
      this.#resolvePath(vfsPath, false),
      options,
    );
  }
  readdirSync(vfsPath, options) {
    return lazyNodeFs().readdirSync(this.#resolvePath(vfsPath), options);
  }
  async readdir(vfsPath, options) {
    return lazyNodeFs().promises.readdir(this.#resolvePath(vfsPath), options);
  }
  mkdirSync(vfsPath, options) {
    return lazyNodeFs().mkdirSync(this.#resolvePath(vfsPath), options);
  }
  async mkdir(vfsPath, options) {
    return lazyNodeFs().promises.mkdir(this.#resolvePath(vfsPath), options);
  }
  rmdirSync(vfsPath) {
    lazyNodeFs().rmdirSync(this.#resolvePath(vfsPath));
  }
  async rmdir(vfsPath) {
    return lazyNodeFs().promises.rmdir(this.#resolvePath(vfsPath));
  }
  unlinkSync(vfsPath) {
    lazyNodeFs().unlinkSync(this.#resolvePath(vfsPath));
  }
  async unlink(vfsPath) {
    return lazyNodeFs().promises.unlink(this.#resolvePath(vfsPath));
  }
  renameSync(oldVfsPath, newVfsPath) {
    lazyNodeFs().renameSync(
      this.#resolvePath(oldVfsPath),
      this.#resolvePath(newVfsPath),
    );
  }
  async rename(oldVfsPath, newVfsPath) {
    return lazyNodeFs().promises.rename(
      this.#resolvePath(oldVfsPath),
      this.#resolvePath(newVfsPath),
    );
  }
  readlinkSync(vfsPath, options) {
    const realPath = this.#resolvePath(vfsPath, false);
    const target = lazyNodeFs().readlinkSync(realPath, options);
    return this.#translateLinkTarget(target);
  }
  async readlink(vfsPath, options) {
    const realPath = this.#resolvePath(vfsPath, false);
    const target = await lazyNodeFs().promises.readlink(realPath, options);
    return this.#translateLinkTarget(target);
  }
  #translateLinkTarget(target) {
    if (typeof target !== "string") return target;
    if (!pathMod.isAbsolute(target)) return target;
    const sep = pathMod.sep;
    const rootWithSep = this.#rootPath + sep;
    if (target === this.#rootPath) return "/";
    if (StringPrototypeStartsWith(target, rootWithSep)) {
      return "/" +
        StringPrototypeReplaceAll(
          StringPrototypeSlice(target, rootWithSep.length),
          "\\",
          "/",
        );
    }
    return target;
  }
  symlinkSync(target, vfsPath, type) {
    if (pathMod.isAbsolute(target)) {
      throw createEACCES("symlink", vfsPath);
    }
    const realPath = this.#resolvePath(vfsPath);
    const sep = pathMod.sep;
    const resolvedTarget = pathMod.resolve(pathMod.dirname(realPath), target);
    const rootWithSep = StringPrototypeEndsWith(this.#rootPath, sep)
      ? this.#rootPath
      : this.#rootPath + sep;
    if (
      resolvedTarget !== this.#rootPath &&
      !StringPrototypeStartsWith(resolvedTarget, rootWithSep)
    ) {
      throw createEACCES("symlink", vfsPath);
    }
    lazyNodeFs().symlinkSync(target, realPath, type);
  }
  async symlink(target, vfsPath, type) {
    if (pathMod.isAbsolute(target)) {
      throw createEACCES("symlink", vfsPath);
    }
    const realPath = this.#resolvePath(vfsPath);
    const sep = pathMod.sep;
    const resolvedTarget = pathMod.resolve(pathMod.dirname(realPath), target);
    const rootWithSep = StringPrototypeEndsWith(this.#rootPath, sep)
      ? this.#rootPath
      : this.#rootPath + sep;
    if (
      resolvedTarget !== this.#rootPath &&
      !StringPrototypeStartsWith(resolvedTarget, rootWithSep)
    ) {
      throw createEACCES("symlink", vfsPath);
    }
    return lazyNodeFs().promises.symlink(target, realPath, type);
  }
  #toVfsPath(resolved, vfsPath, syscall) {
    const rel = pathMod.relative(this.#rootPath, resolved);
    const sep = pathMod.sep;
    if (rel === "") return "/";
    if (
      rel === ".." ||
      StringPrototypeStartsWith(rel, ".." + sep) ||
      pathMod.isAbsolute(rel)
    ) {
      throw createEACCES(syscall, vfsPath);
    }
    return "/" + StringPrototypeReplaceAll(rel, "\\", "/");
  }
  realpathSync(vfsPath, options) {
    const realPath = this.#resolvePath(vfsPath);
    const resolved = lazyNodeFs().realpathSync(realPath, options);
    return this.#toVfsPath(resolved, vfsPath, "realpath");
  }
  async realpath(vfsPath, options) {
    const realPath = this.#resolvePath(vfsPath);
    const resolved = await lazyNodeFs().promises.realpath(realPath, options);
    return this.#toVfsPath(resolved, vfsPath, "realpath");
  }
  accessSync(vfsPath, mode) {
    lazyNodeFs().accessSync(this.#resolvePath(vfsPath), mode);
  }
  async access(vfsPath, mode) {
    return lazyNodeFs().promises.access(this.#resolvePath(vfsPath), mode);
  }
  copyFileSync(srcVfs, destVfs, mode) {
    lazyNodeFs().copyFileSync(
      this.#resolvePath(srcVfs),
      this.#resolvePath(destVfs),
      mode,
    );
  }
  async copyFile(srcVfs, destVfs, mode) {
    return lazyNodeFs().promises.copyFile(
      this.#resolvePath(srcVfs),
      this.#resolvePath(destVfs),
      mode,
    );
  }
  watch(vfsPath, options) {
    return lazyNodeFs().watch(this.#resolvePath(vfsPath), options);
  }
  watchAsync(vfsPath, options) {
    return lazyNodeFs().promises.watch(this.#resolvePath(vfsPath), options);
  }
  watchFile(vfsPath, options) {
    return lazyNodeFs().watchFile(
      this.#resolvePath(vfsPath),
      options,
      () => {},
    );
  }
  unwatchFile(vfsPath, listener) {
    lazyNodeFs().unwatchFile(this.#resolvePath(vfsPath), listener);
  }
}

// =====================================================================
// VirtualDir
// =====================================================================

class VirtualDir {
  #path;
  #entries;
  #index;
  #closed;

  constructor(dirPath, entries) {
    this.#path = dirPath;
    this.#entries = entries;
    this.#index = 0;
    this.#closed = false;
  }
  get path() {
    return this.#path;
  }
  readSync() {
    if (this.#closed) throw new ERR_DIR_CLOSED();
    if (this.#index >= this.#entries.length) return null;
    return this.#entries[this.#index++];
  }
  async read(callback) {
    if (typeof callback === "function") {
      try {
        const r = this.readSync();
        queueMicrotask(() => callback(null, r));
      } catch (err) {
        queueMicrotask(() => callback(err));
      }
      return;
    }
    return this.readSync();
  }
  closeSync() {
    if (this.#closed) throw new ERR_DIR_CLOSED();
    this.#closed = true;
  }
  async close(callback) {
    if (typeof callback === "function") {
      this.closeSync();
      queueMicrotask(() => callback(null));
      return;
    }
    this.closeSync();
  }
  async *entries() {
    if (this.#closed) throw new ERR_DIR_CLOSED();
    try {
      let entry;
      while ((entry = this.readSync()) !== null) yield entry;
    } finally {
      if (!this.#closed) this.closeSync();
    }
  }
}
VirtualDir.prototype[SymbolAsyncIterator] = VirtualDir.prototype.entries;
VirtualDir.prototype[SymbolAsyncDispose] = VirtualDir.prototype.close;

// =====================================================================
// Watchers (VFSWatcher, VFSStatWatcher, VFSWatchAsyncIterable)
// =====================================================================

let VFSWatcher;
let VFSStatWatcher;
let VFSWatchAsyncIterable;

function ensureWatcherClasses() {
  if (VFSWatcher !== undefined) return;
  const EventEmitter = lazyEvents().EventEmitter;
  const { setInterval, clearInterval } = lazyTimers();

  VFSWatcher = class VFSWatcher extends EventEmitter {
    #vfs;
    #path;
    #interval;
    #timer = null;
    #lastStats;
    #closed = false;
    #persistent;
    #recursive;
    #encoding;
    #trackedFiles;
    #signal;
    #abortHandler = null;

    constructor(provider, path, options = {}) {
      super();
      this.#vfs = provider;
      this.#path = path;
      this.#interval = options.interval ?? 100;
      this.#persistent = options.persistent !== false;
      this.#recursive = options.recursive === true;
      this.#encoding = options.encoding;
      this.#trackedFiles = new SafeMap();
      this.#signal = options.signal;

      if (this.#signal) {
        if (this.#signal.aborted) {
          this.close();
          return;
        }
        this.#abortHandler = () => this.close();
        this.#signal.addEventListener("abort", this.#abortHandler, {
          once: true,
        });
      }

      this.#lastStats = this.#getStats();
      if (this.#lastStats?.isDirectory()) {
        if (this.#recursive) this.#buildList(this.#path, "");
        else this.#buildChildren(this.#path);
      }
      this.#startPolling();
    }

    #encodeFilename(filename) {
      if (this.#encoding === "buffer") return Buffer.from(filename);
      return filename;
    }
    #getStats() {
      try {
        return this.#vfs.statSync(this.#path);
      } catch {
        return null;
      }
    }
    #getStatsFor(filePath) {
      try {
        return this.#vfs.statSync(filePath);
      } catch {
        return null;
      }
    }
    #buildList(dirPath, relativePath) {
      try {
        const entries = this.#vfs.readdirSync(dirPath, { withFileTypes: true });
        for (const entry of entries) {
          const fullPath = pathJoin(dirPath, entry.name);
          const relPath = relativePath
            ? pathJoin(relativePath, entry.name)
            : entry.name;
          if (entry.isDirectory()) this.#buildList(fullPath, relPath);
          else {
            const stats = this.#getStatsFor(fullPath);
            MapPrototypeSet(this.#trackedFiles, fullPath, {
              stats,
              relativePath: relPath,
            });
          }
        }
      } catch { /* ignore */ }
    }
    #buildChildren(dirPath) {
      try {
        const entries = this.#vfs.readdirSync(dirPath);
        for (const name of entries) {
          const fullPath = pathJoin(dirPath, name);
          const stats = this.#getStatsFor(fullPath);
          MapPrototypeSet(this.#trackedFiles, fullPath, {
            stats,
            relativePath: name,
          });
        }
      } catch { /* ignore */ }
    }
    #startPolling() {
      if (this.#closed) return;
      this.#timer = setInterval(() => this.#poll(), this.#interval);
      if (!this.#persistent && this.#timer.unref) this.#timer.unref();
    }
    #poll() {
      if (this.#closed) return;
      if (this.#lastStats?.isDirectory()) {
        this.#pollDirectory();
        return;
      }
      const newStats = this.#getStats();
      if (this.#statsChanged(this.#lastStats, newStats)) {
        const eventType = this.#determineEventType(this.#lastStats, newStats);
        const filename = this.#encodeFilename(pathBasename(this.#path));
        this.emit("change", eventType, filename);
      }
      this.#lastStats = newStats;
    }
    #pollDirectory() {
      if (this.#recursive) this.#rescanRecursive(this.#path, "");
      else this.#rescanChildren(this.#path);
      for (const { 0: filePath, 1: info } of this.#trackedFiles) {
        const newStats = this.#getStatsFor(filePath);
        if (newStats === null && info.stats !== null) {
          this.emit(
            "change",
            "rename",
            this.#encodeFilename(info.relativePath),
          );
          MapPrototypeDelete(this.#trackedFiles, filePath);
        } else if (this.#statsChanged(info.stats, newStats)) {
          const eventType = this.#determineEventType(info.stats, newStats);
          this.emit(
            "change",
            eventType,
            this.#encodeFilename(info.relativePath),
          );
          info.stats = newStats;
        }
      }
    }
    #rescanChildren(dirPath) {
      try {
        const entries = this.#vfs.readdirSync(dirPath);
        for (const name of entries) {
          const fullPath = pathJoin(dirPath, name);
          if (!this.#trackedFiles.has(fullPath)) {
            const stats = this.#getStatsFor(fullPath);
            MapPrototypeSet(this.#trackedFiles, fullPath, {
              stats,
              relativePath: name,
            });
            this.emit("change", "rename", this.#encodeFilename(name));
          }
        }
      } catch { /* ignore */ }
    }
    #rescanRecursive(dirPath, relativePath) {
      try {
        const entries = this.#vfs.readdirSync(dirPath, {
          withFileTypes: true,
        });
        for (const entry of entries) {
          const fullPath = pathJoin(dirPath, entry.name);
          const relPath = relativePath
            ? pathJoin(relativePath, entry.name)
            : entry.name;
          if (entry.isDirectory()) this.#rescanRecursive(fullPath, relPath);
          else if (!this.#trackedFiles.has(fullPath)) {
            const stats = this.#getStatsFor(fullPath);
            MapPrototypeSet(this.#trackedFiles, fullPath, {
              stats,
              relativePath: relPath,
            });
            this.emit("change", "rename", this.#encodeFilename(relPath));
          }
        }
      } catch { /* ignore */ }
    }
    #statsChanged(oldStats, newStats) {
      if ((oldStats === null) !== (newStats === null)) return true;
      if (oldStats === null && newStats === null) return false;
      if (oldStats.mtimeMs !== newStats.mtimeMs) return true;
      if (oldStats.size !== newStats.size) return true;
      return false;
    }
    #determineEventType(oldStats, newStats) {
      if ((oldStats === null) !== (newStats === null)) return "rename";
      return "change";
    }
    close() {
      if (this.#closed) return;
      this.#closed = true;
      if (this.#timer) {
        clearInterval(this.#timer);
        this.#timer = null;
      }
      this.#trackedFiles.clear();
      if (this.#signal && this.#abortHandler) {
        this.#signal.removeEventListener("abort", this.#abortHandler);
      }
      this.emit("close");
    }
    unref() {
      this.#timer?.unref?.();
      return this;
    }
    ref() {
      this.#timer?.ref?.();
      return this;
    }
  };

  VFSStatWatcher = class VFSStatWatcher extends EventEmitter {
    #vfs;
    #path;
    #interval;
    #persistent;
    #bigint;
    #closed = false;
    #timer = null;
    #lastStats;
    #listeners;

    constructor(provider, path, options = {}) {
      super();
      this.#vfs = provider;
      this.#path = path;
      this.#interval = options.interval ?? 5007;
      this.#persistent = options.persistent !== false;
      this.#bigint = options.bigint === true;
      this.#listeners = new SafeSet();
      this.#lastStats = this.#getStats();
      this.#startPolling();
    }
    #getStats() {
      try {
        return this.#vfs.statSync(this.#path, { bigint: this.#bigint });
      } catch {
        return createZeroStats({ bigint: this.#bigint });
      }
    }
    #startPolling() {
      if (this.#closed) return;
      this.#timer = setInterval(() => this.#poll(), this.#interval);
      if (!this.#persistent && this.#timer.unref) this.#timer.unref();
    }
    #poll() {
      if (this.#closed) return;
      const newStats = this.#getStats();
      if (this.#statsChanged(this.#lastStats, newStats)) {
        const prev = this.#lastStats;
        this.#lastStats = newStats;
        this.emit("change", newStats, prev);
      }
    }
    #statsChanged(oldStats, newStats) {
      if (oldStats.mtimeMs !== newStats.mtimeMs) return true;
      if (oldStats.ctimeMs !== newStats.ctimeMs) return true;
      if (oldStats.size !== newStats.size) return true;
      return false;
    }
    addListener(event, listener) {
      if (event === "change") this.#listeners.add(listener);
      super.addListener(event, listener);
      return this;
    }
    removeListener(event, listener) {
      if (event === "change") this.#listeners.delete(listener);
      super.removeListener(event, listener);
      return this;
    }
    removeAllListeners(eventName) {
      if (eventName === "change") this.#listeners.clear();
      super.removeAllListeners(eventName);
      return this;
    }
    hasNoListeners() {
      return this.#listeners.size === 0;
    }
    stop() {
      if (this.#closed) return;
      this.#closed = true;
      if (this.#timer) {
        clearInterval(this.#timer);
        this.#timer = null;
      }
      this.emit("stop");
    }
    unref() {
      this.#timer?.unref?.();
      return this;
    }
    ref() {
      this.#timer?.ref?.();
      return this;
    }
  };

  const kMaxPendingEvents = 1024;

  VFSWatchAsyncIterable = class VFSWatchAsyncIterable {
    #watcher;
    #closed = false;
    #pendingEvents = [];
    #pendingResolvers = [];

    constructor(provider, path, options = {}) {
      const signal = options.signal;
      const watcherOptions = ObjectAssign({}, options);
      delete watcherOptions.signal;
      this.#watcher = new VFSWatcher(provider, path, watcherOptions);

      this.#watcher.on("change", (eventType, filename) => {
        const event = { eventType, filename };
        if (this.#pendingResolvers.length > 0) {
          const { resolve } = this.#pendingResolvers.shift();
          resolve({ value: event, done: false });
        } else if (this.#pendingEvents.length < kMaxPendingEvents) {
          ArrayPrototypePush(this.#pendingEvents, event);
        }
      });

      this.#watcher.on("close", () => {
        this.#closed = true;
        while (this.#pendingResolvers.length > 0) {
          const { resolve } = this.#pendingResolvers.shift();
          resolve({ value: undefined, done: true });
        }
      });

      if (signal) {
        const onAbort = () => {
          this.#closed = true;
          const err = new AbortError(undefined, { cause: signal.reason });
          while (this.#pendingResolvers.length > 0) {
            const { reject } = this.#pendingResolvers.shift();
            reject(err);
          }
          this.#watcher.close();
        };
        if (signal.aborted) onAbort();
        else signal.addEventListener("abort", onAbort, { once: true });
      }
    }

    [SymbolAsyncIterator]() {
      return this;
    }
    next() {
      if (this.#closed) {
        return PromiseResolve({ value: undefined, done: true });
      }
      if (this.#pendingEvents.length > 0) {
        const event = this.#pendingEvents.shift();
        return PromiseResolve({ value: event, done: false });
      }
      return new PrimordialPromise((resolve, reject) => {
        ArrayPrototypePush(this.#pendingResolvers, { resolve, reject });
      });
    }
    return() {
      this.#watcher.close();
      return PromiseResolve({ value: undefined, done: true });
    }
    throw(_error) {
      this.#watcher.close();
      return PromiseResolve({ value: undefined, done: true });
    }
  };
}

// =====================================================================
// Streams (VirtualReadStream, VirtualWriteStream)
// =====================================================================

let VirtualReadStreamCtor = null;
let VirtualWriteStreamCtor = null;

function getStreamCtors() {
  if (VirtualReadStreamCtor !== null) {
    return {
      VirtualReadStream: VirtualReadStreamCtor,
      VirtualWriteStream: VirtualWriteStreamCtor,
    };
  }
  const { Readable, Writable } = lazyStream();

  class VirtualReadStream extends Readable {
    #vfs;
    #path;
    #fd = null;
    #end;
    #pos;
    #content = null;
    #autoClose;
    bytesRead = 0;
    pending = true;

    constructor(vfs, filePath, options = {}) {
      const {
        start,
        end,
        highWaterMark = 64 * 1024,
        encoding,
        fd,
        // deno-lint-ignore no-unused-vars
        autoClose,
        ...streamOptions
      } = options;
      if (start !== undefined && (!Number.isInteger(start) || start < 0)) {
        throw new ERR_OUT_OF_RANGE("start", ">= 0", start);
      }
      if (
        end !== undefined && end !== Infinity &&
        (!Number.isInteger(end) || end < 0)
      ) {
        throw new ERR_OUT_OF_RANGE("end", ">= 0", end);
      }
      if (
        start !== undefined && end !== undefined && end !== Infinity &&
        start > end
      ) {
        throw new ERR_OUT_OF_RANGE("start", `<= "end" (here: ${end})`, start);
      }

      super({ ...streamOptions, highWaterMark, encoding });
      this.#vfs = vfs;
      this.#path = filePath;
      this.#end = end === undefined ? Infinity : end;
      this.#pos = start === undefined ? 0 : start;
      this.#autoClose = options.autoClose !== false;

      if (fd !== null && fd !== undefined) {
        this.#fd = fd;
        queueMicrotask(() => {
          this.emit("open", this.#fd);
          this.emit("ready");
        });
      } else {
        queueMicrotask(() => this.#openFile());
      }
    }

    get path() {
      return this.#path;
    }

    #openFile() {
      try {
        this.#fd = this.#vfs.openSync(this.#path);
        this.emit("open", this.#fd);
        this.emit("ready");
      } catch (err) {
        this.destroy(err);
      }
    }

    _read(size) {
      if (this.destroyed || this.#fd === null) return;
      if (this.#content === null) {
        try {
          const vfd = getVirtualFd(this.#fd);
          if (!vfd) {
            this.destroy(createEBADF("read"));
            return;
          }
          this.#content = vfd.entry.readFileSync();
          this.pending = false;
        } catch (err) {
          this.destroy(err);
          return;
        }
      }
      const endPos = this.#end === Infinity
        ? this.#content.length
        : this.#end + 1;
      const remaining = MathMin(endPos, this.#content.length) - this.#pos;
      if (remaining <= 0) {
        this.push(null);
        return;
      }
      const bytesToRead = MathMin(size, remaining);
      const chunk = this.#content.subarray(this.#pos, this.#pos + bytesToRead);
      this.#pos += bytesToRead;
      this.bytesRead += bytesToRead;
      this.push(chunk);
      if (this.#pos >= endPos || this.#pos >= this.#content.length) {
        this.push(null);
      }
    }

    #close() {
      if (this.#fd !== null) {
        try {
          this.#vfs.closeSync(this.#fd);
        } catch { /* ignore */ }
        this.#fd = null;
      }
    }
    _destroy(err, callback) {
      if (this.#autoClose) this.#close();
      callback(err);
    }
  }

  class VirtualWriteStream extends Writable {
    #vfs;
    #path;
    #fd = null;
    #autoClose;
    #start;
    bytesWritten = 0;
    pending = true;

    constructor(vfs, filePath, options = {}) {
      const { highWaterMark = 64 * 1024, ...streamOptions } = options;
      if (
        options.start !== undefined &&
        (!Number.isInteger(options.start) || options.start < 0)
      ) {
        throw new ERR_OUT_OF_RANGE("start", ">= 0", options.start);
      }
      super({ ...streamOptions, highWaterMark });
      this.#vfs = vfs;
      this.#path = filePath;
      this.#autoClose = options.autoClose !== false;
      this.#start = options.start;
      const fd = options.fd;
      if (fd !== null && fd !== undefined) {
        this.#fd = fd;
        if (this.#start !== undefined) this.#setPosition(this.#start);
        queueMicrotask(() => {
          this.emit("open", this.#fd);
          this.emit("ready");
        });
      } else {
        const flags = options.flags || "w";
        try {
          this.#fd = this.#vfs.openSync(this.#path, flags);
          if (this.#start !== undefined) this.#setPosition(this.#start);
        } catch (err) {
          queueMicrotask(() => this.destroy(err));
          return;
        }
        queueMicrotask(() => {
          this.emit("open", this.#fd);
          this.emit("ready");
        });
      }
    }

    #setPosition(pos) {
      const vfd = getVirtualFd(this.#fd);
      if (vfd) vfd.entry.position = pos;
    }

    get path() {
      return this.#path;
    }

    _write(chunk, encoding, callback) {
      if (this.destroyed || this.#fd === null) {
        callback(createEBADF("write"));
        return;
      }
      try {
        const buf = typeof chunk === "string"
          ? Buffer.from(chunk, encoding)
          : chunk;
        this.#vfs.writeSync(this.#fd, buf, 0, buf.length, null);
        this.bytesWritten += buf.length;
        this.pending = false;
        callback();
      } catch (err) {
        callback(err);
      }
    }
    _final(callback) {
      callback();
    }
    #close() {
      if (this.#fd !== null) {
        try {
          this.#vfs.closeSync(this.#fd);
        } catch { /* ignore */ }
        this.#fd = null;
      }
    }
    _destroy(err, callback) {
      if (this.#autoClose) this.#close();
      callback(err);
    }
  }

  VirtualReadStreamCtor = VirtualReadStream;
  VirtualWriteStreamCtor = VirtualWriteStream;
  return { VirtualReadStream, VirtualWriteStream };
}

// =====================================================================
// VirtualFileSystem
// =====================================================================

const kEmptyObject = ObjectFreeze({ __proto__: null });

class VirtualFileSystem {
  #provider;
  #promises;

  constructor(providerOrOptions, options = kEmptyObject) {
    let provider = null;
    if (providerOrOptions !== undefined && providerOrOptions !== null) {
      if (typeof providerOrOptions.openSync === "function") {
        provider = providerOrOptions;
      } else if (typeof providerOrOptions === "object") {
        options = providerOrOptions;
        provider = null;
      }
    }
    if (options.emitExperimentalWarning !== undefined) {
      validateBoolean(
        options.emitExperimentalWarning,
        "options.emitExperimentalWarning",
      );
    }
    if (options.emitExperimentalWarning !== false) {
      try {
        emitExperimentalWarning("VirtualFileSystem");
      } catch {
        // Some embedders (e.g. snapshots) may not have process warnings wired up
        // yet; the warning is non-essential.
      }
    }
    this.#provider = provider ?? new MemoryProvider();
    this.#promises = null;
  }

  get provider() {
    return this.#provider;
  }
  get readonly() {
    return this.#provider.readonly;
  }

  #toProviderPath(p) {
    return pathNormalize(p);
  }

  // ===== Sync =====
  existsSync(p) {
    try {
      return this.#provider.existsSync(this.#toProviderPath(p));
    } catch {
      return false;
    }
  }
  statSync(p, options) {
    return this.#provider.statSync(this.#toProviderPath(p), options);
  }
  lstatSync(p, options) {
    return this.#provider.lstatSync(this.#toProviderPath(p), options);
  }
  readFileSync(p, options) {
    return this.#provider.readFileSync(this.#toProviderPath(p), options);
  }
  writeFileSync(p, data, options) {
    this.#provider.writeFileSync(this.#toProviderPath(p), data, options);
  }
  appendFileSync(p, data, options) {
    this.#provider.appendFileSync(this.#toProviderPath(p), data, options);
  }
  readdirSync(dirPath, options) {
    const result = this.#provider.readdirSync(
      this.#toProviderPath(dirPath),
      options,
    );
    if (options?.withFileTypes === true) {
      const recursive = options?.recursive === true;
      for (let i = 0; i < result.length; i++) {
        const dirent = result[i];
        if (recursive) {
          const slashIdx = dirent.name.lastIndexOf("/");
          if (slashIdx !== -1) {
            const subdir = dirent.name.slice(0, slashIdx);
            dirent.parentPath = pathJoin(dirPath, subdir);
            dirent.name = dirent.name.slice(slashIdx + 1);
          } else {
            dirent.parentPath = dirPath;
          }
        } else {
          dirent.parentPath = dirPath;
        }
      }
    }
    return result;
  }
  mkdirSync(p, options) {
    return this.#provider.mkdirSync(this.#toProviderPath(p), options);
  }
  rmdirSync(p) {
    this.#provider.rmdirSync(this.#toProviderPath(p));
  }
  unlinkSync(p) {
    this.#provider.unlinkSync(this.#toProviderPath(p));
  }
  renameSync(oldPath, newPath) {
    this.#provider.renameSync(
      this.#toProviderPath(oldPath),
      this.#toProviderPath(newPath),
    );
  }
  copyFileSync(src, dest, mode) {
    this.#provider.copyFileSync(
      this.#toProviderPath(src),
      this.#toProviderPath(dest),
      mode,
    );
  }
  realpathSync(p, options) {
    return this.#provider.realpathSync(this.#toProviderPath(p), options);
  }
  readlinkSync(p, options) {
    return this.#provider.readlinkSync(this.#toProviderPath(p), options);
  }
  symlinkSync(target, p, type) {
    this.#provider.symlinkSync(target, this.#toProviderPath(p), type);
  }
  accessSync(p, mode) {
    this.#provider.accessSync(this.#toProviderPath(p), mode);
  }
  rmSync(p, options) {
    const recursive = options?.recursive === true;
    const force = options?.force === true;
    let stats;
    try {
      stats = this.lstatSync(p);
    } catch (err) {
      if (force && err?.code === "ENOENT") return;
      throw err;
    }
    if (stats.isSymbolicLink()) {
      this.unlinkSync(p);
      return;
    }
    if (stats.isDirectory()) {
      if (!recursive) throw createEISDIR("rm", p);
      const entries = this.readdirSync(p);
      for (let i = 0; i < entries.length; i++) {
        this.rmSync(pathJoin(p, entries[i]), options);
      }
      this.rmdirSync(p);
    } else {
      this.unlinkSync(p);
    }
  }
  truncateSync(p, len = 0) {
    if (len < 0) len = 0;
    const handle = this.#provider.openSync(this.#toProviderPath(p), "r+");
    try {
      handle.truncateSync(len);
    } finally {
      handle.closeSync();
    }
  }
  ftruncateSync(fd, len = 0) {
    const vfd = getVirtualFd(fd);
    if (!vfd) throw createEBADF("ftruncate");
    vfd.entry.truncateSync(len);
  }
  linkSync(existingPath, newPath) {
    this.#provider.linkSync(
      this.#toProviderPath(existingPath),
      this.#toProviderPath(newPath),
    );
  }
  chmodSync(p, mode) {
    this.#provider.chmodSync(this.#toProviderPath(p), mode);
  }
  chownSync(p, uid, gid) {
    this.#provider.chownSync(this.#toProviderPath(p), uid, gid);
  }
  utimesSync(p, atime, mtime) {
    this.#provider.utimesSync(this.#toProviderPath(p), atime, mtime);
  }
  lutimesSync(p, atime, mtime) {
    this.#provider.lutimesSync(this.#toProviderPath(p), atime, mtime);
  }
  mkdtempSync(prefix) {
    const providerPrefix = this.#toProviderPath(prefix);
    const chars =
      "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let suffix = "";
    for (let i = 0; i < 6; i++) {
      suffix += chars[(MathRandom() * chars.length) | 0];
    }
    const dirPath = providerPrefix + suffix;
    this.#provider.mkdirSync(dirPath);
    return dirPath;
  }
  opendirSync(dirPath, options) {
    const entries = this.readdirSync(dirPath, {
      withFileTypes: true,
      recursive: options?.recursive,
    });
    return new VirtualDir(dirPath, entries);
  }
  openAsBlob(p, options) {
    const content = this.#provider.readFileSync(this.#toProviderPath(p));
    const type = options?.type || "";
    return new Blob([content], { type });
  }

  // ===== File descriptor =====
  openSync(p, flags = "r", mode) {
    const handle = this.#provider.openSync(
      this.#toProviderPath(p),
      flags,
      mode,
    );
    return openVirtualFd(handle);
  }
  closeSync(fd) {
    const vfd = getVirtualFd(fd);
    if (!vfd) throw createEBADF("close");
    vfd.entry.closeSync();
    closeVirtualFd(fd);
  }
  readSync(fd, buffer, offset, length, position) {
    const vfd = getVirtualFd(fd);
    if (!vfd) throw createEBADF("read");
    return vfd.entry.readSync(buffer, offset, length, position);
  }
  writeSync(fd, buffer, offset, length, position) {
    const vfd = getVirtualFd(fd);
    if (!vfd) throw createEBADF("write");
    return vfd.entry.writeSync(buffer, offset, length, position);
  }
  fstatSync(fd, options) {
    const vfd = getVirtualFd(fd);
    if (!vfd) throw createEBADF("fstat");
    return vfd.entry.statSync(options);
  }

  // ===== Callback API =====
  readFile(p, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.readFile(this.#toProviderPath(p), options),
      (data) => callback(null, data),
      (err) => callback(err),
    );
  }
  writeFile(p, data, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.writeFile(this.#toProviderPath(p), data, options),
      () => callback(null),
      (err) => callback(err),
    );
  }
  stat(p, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.stat(this.#toProviderPath(p), options),
      (stats) => callback(null, stats),
      (err) => callback(err),
    );
  }
  lstat(p, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.lstat(this.#toProviderPath(p), options),
      (stats) => callback(null, stats),
      (err) => callback(err),
    );
  }
  readdir(dirPath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.readdir(this.#toProviderPath(dirPath), options),
      (entries) => callback(null, entries),
      (err) => callback(err),
    );
  }
  realpath(p, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.realpath(this.#toProviderPath(p), options),
      (rp) => callback(null, rp),
      (err) => callback(err),
    );
  }
  readlink(p, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.readlink(this.#toProviderPath(p), options),
      (target) => callback(null, target),
      (err) => callback(err),
    );
  }
  access(p, mode, callback) {
    if (typeof mode === "function") {
      callback = mode;
      mode = undefined;
    }
    PromisePrototypeThen(
      this.#provider.access(this.#toProviderPath(p), mode),
      () => callback(null),
      (err) => callback(err),
    );
  }
  open(p, flags, mode, callback) {
    if (typeof flags === "function") {
      callback = flags;
      flags = "r";
      mode = undefined;
    } else if (typeof mode === "function") {
      callback = mode;
      mode = undefined;
    }
    PromisePrototypeThen(
      this.#provider.open(this.#toProviderPath(p), flags, mode),
      (handle) => callback(null, openVirtualFd(handle)),
      (err) => callback(err),
    );
  }
  close(fd, callback) {
    const vfd = getVirtualFd(fd);
    if (!vfd) {
      queueMicrotask(() => callback(createEBADF("close")));
      return;
    }
    PromisePrototypeThen(
      vfd.entry.close(),
      () => {
        closeVirtualFd(fd);
        callback(null);
      },
      (err) => callback(err),
    );
  }
  read(fd, buffer, offset, length, position, callback) {
    const vfd = getVirtualFd(fd);
    if (!vfd) {
      queueMicrotask(() => callback(createEBADF("read")));
      return;
    }
    PromisePrototypeThen(
      vfd.entry.read(buffer, offset, length, position),
      ({ bytesRead }) => callback(null, bytesRead, buffer),
      (err) => callback(err),
    );
  }
  write(fd, buffer, offset, length, position, callback) {
    const vfd = getVirtualFd(fd);
    if (!vfd) {
      queueMicrotask(() => callback(createEBADF("write")));
      return;
    }
    PromisePrototypeThen(
      vfd.entry.write(buffer, offset, length, position),
      ({ bytesWritten }) => callback(null, bytesWritten, buffer),
      (err) => callback(err),
    );
  }
  rm(p, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    try {
      this.rmSync(p, options);
      queueMicrotask(() => callback(null));
    } catch (err) {
      queueMicrotask(() => callback(err));
    }
  }
  fstat(fd, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    const vfd = getVirtualFd(fd);
    if (!vfd) {
      queueMicrotask(() => callback(createEBADF("fstat")));
      return;
    }
    PromisePrototypeThen(
      vfd.entry.stat(options),
      (stats) => callback(null, stats),
      (err) => callback(err),
    );
  }
  truncate(p, len, callback) {
    if (typeof len === "function") {
      callback = len;
      len = 0;
    }
    try {
      this.truncateSync(p, len);
      queueMicrotask(() => callback(null));
    } catch (err) {
      queueMicrotask(() => callback(err));
    }
  }
  ftruncate(fd, len, callback) {
    if (typeof len === "function") {
      callback = len;
      len = 0;
    }
    try {
      this.ftruncateSync(fd, len);
      queueMicrotask(() => callback(null));
    } catch (err) {
      queueMicrotask(() => callback(err));
    }
  }
  link(existingPath, newPath, callback) {
    try {
      this.linkSync(existingPath, newPath);
      queueMicrotask(() => callback(null));
    } catch (err) {
      queueMicrotask(() => callback(err));
    }
  }
  mkdtemp(prefix, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    try {
      const dirPath = this.mkdtempSync(prefix);
      queueMicrotask(() => callback(null, dirPath));
    } catch (err) {
      queueMicrotask(() => callback(err));
    }
  }
  opendir(dirPath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    try {
      const dir = this.opendirSync(dirPath, options);
      queueMicrotask(() => callback(null, dir));
    } catch (err) {
      queueMicrotask(() => callback(err));
    }
  }

  // ===== Streams =====
  createReadStream(p, options) {
    const { VirtualReadStream } = getStreamCtors();
    return new VirtualReadStream(this, p, options);
  }
  createWriteStream(p, options) {
    const { VirtualWriteStream } = getStreamCtors();
    return new VirtualWriteStream(this, p, options);
  }

  // ===== Watch =====
  watch(p, options, listener) {
    if (typeof options === "function") {
      listener = options;
      options = {};
    }
    ensureWatcherClasses();
    const watcher = this.#provider.watch(this.#toProviderPath(p), options);
    if (listener) watcher.on("change", listener);
    return watcher;
  }
  watchFile(p, options, listener) {
    if (typeof options === "function") {
      listener = options;
      options = {};
    }
    ensureWatcherClasses();
    return this.#provider.watchFile(
      this.#toProviderPath(p),
      options,
      listener,
    );
  }
  unwatchFile(p, listener) {
    this.#provider.unwatchFile(this.#toProviderPath(p), listener);
  }

  // ===== Promises =====
  get promises() {
    if (this.#promises === null) {
      this.#promises = this.#createPromisesAPI();
    }
    return this.#promises;
  }

  #createPromisesAPI() {
    const provider = this.#provider;
    const toProviderPath = (p) => this.#toProviderPath(p);
    const mkdtempImpl = (prefix) => this.mkdtempSync(prefix);

    return ObjectFreeze({
      async readFile(p, options) {
        return provider.readFile(toProviderPath(p), options);
      },
      async writeFile(p, data, options) {
        return provider.writeFile(toProviderPath(p), data, options);
      },
      async appendFile(p, data, options) {
        return provider.appendFile(toProviderPath(p), data, options);
      },
      async stat(p, options) {
        return provider.stat(toProviderPath(p), options);
      },
      async lstat(p, options) {
        return provider.lstat(toProviderPath(p), options);
      },
      async readdir(p, options) {
        return provider.readdir(toProviderPath(p), options);
      },
      async mkdir(p, options) {
        return provider.mkdir(toProviderPath(p), options);
      },
      async rmdir(p) {
        return provider.rmdir(toProviderPath(p));
      },
      async unlink(p) {
        return provider.unlink(toProviderPath(p));
      },
      async rename(oldPath, newPath) {
        return provider.rename(
          toProviderPath(oldPath),
          toProviderPath(newPath),
        );
      },
      async copyFile(src, dest, mode) {
        return provider.copyFile(
          toProviderPath(src),
          toProviderPath(dest),
          mode,
        );
      },
      async realpath(p, options) {
        return provider.realpath(toProviderPath(p), options);
      },
      async readlink(p, options) {
        return provider.readlink(toProviderPath(p), options);
      },
      async symlink(target, p, type) {
        return provider.symlink(target, toProviderPath(p), type);
      },
      async access(p, mode) {
        return provider.access(toProviderPath(p), mode);
      },
      async rm(p, options) {
        const recursive = options?.recursive === true;
        const force = options?.force === true;
        let stats;
        try {
          stats = await provider.lstat(toProviderPath(p));
        } catch (err) {
          if (force && err?.code === "ENOENT") return;
          throw err;
        }
        if (stats.isSymbolicLink()) {
          await provider.unlink(toProviderPath(p));
          return;
        }
        if (stats.isDirectory()) {
          if (!recursive) throw createEISDIR("rm", p);
          const entries = await provider.readdir(toProviderPath(p));
          for (let i = 0; i < entries.length; i++) {
            await this.rm(pathJoin(p, entries[i]), options);
          }
          await provider.rmdir(toProviderPath(p));
        } else {
          await provider.unlink(toProviderPath(p));
        }
      },
      async truncate(p, len = 0) {
        const handle = await provider.open(toProviderPath(p), "r+");
        try {
          await handle.truncate(len);
        } finally {
          await handle.close();
        }
      },
      async link(existingPath, newPath) {
        return provider.link(
          toProviderPath(existingPath),
          toProviderPath(newPath),
        );
      },
      async mkdtemp(prefix) {
        return mkdtempImpl(prefix);
      },
      async chmod(p, mode) {
        provider.chmodSync(toProviderPath(p), mode);
      },
      async lchmod(p, mode) {
        provider.chmodSync(toProviderPath(p), mode);
      },
      async chown(p, uid, gid) {
        provider.chownSync(toProviderPath(p), uid, gid);
      },
      async lchown(p, uid, gid) {
        provider.chownSync(toProviderPath(p), uid, gid);
      },
      async utimes(p, atime, mtime) {
        provider.utimesSync(toProviderPath(p), atime, mtime);
      },
      async lutimes(p, atime, mtime) {
        provider.lutimesSync(toProviderPath(p), atime, mtime);
      },
      async open(p, flags, mode) {
        const handle = provider.openSync(toProviderPath(p), flags, mode);
        return openVirtualFd(handle);
      },
      watch(p, options) {
        ensureWatcherClasses();
        return provider.watchAsync(toProviderPath(p), options);
      },
    });
  }
}

// =====================================================================
// Public API
// =====================================================================

function create(provider, options) {
  if (
    provider != null &&
    !(provider instanceof VirtualProvider) &&
    typeof provider === "object"
  ) {
    options = provider;
    provider = undefined;
  }
  return new VirtualFileSystem(provider, options);
}

const exportsObj = {
  create,
  VirtualFileSystem,
  VirtualProvider,
  MemoryProvider,
  RealFSProvider,
  VirtualFileHandle,
  MemoryFileHandle,
  VirtualDir,
  get VFSWatcher() {
    ensureWatcherClasses();
    return VFSWatcher;
  },
  get VFSStatWatcher() {
    ensureWatcherClasses();
    return VFSStatWatcher;
  },
  get VFSWatchAsyncIterable() {
    ensureWatcherClasses();
    return VFSWatchAsyncIterable;
  },
};

return {
  ...exportsObj,
  default: exportsObj,
};
})();
