// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials require-await

// node:vfs - Virtual File System for Node.js.
//
// Ported from https://github.com/platformatic/vfs (the package extracted from
// nodejs/node#61478). The polyfill provides the in-memory VFS data model and
// the synchronous, callback, and promises fs-like APIs. The `mount()`
// machinery that patches Node's `require()` and core `fs` functions is not
// applied to Deno's loaders - instead the VFS instance is the namespace
// through which callers interact with the virtual files. The mount/unmount
// state is still tracked so user code that branches on `vfs.mounted` or
// listens for `vfs-mount` / `vfs-unmount` events still works.

(function () {
const { core, primordials } = __bootstrap;
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const pathMod = core.loadExtScript("ext:deno_node/path/mod.ts");
const lazyStream = core.createLazyLoader("node:stream");

const {
  ArrayFrom,
  ArrayPrototypeJoin,
  ArrayPrototypePush,
  ArrayPrototypeSlice,
  DateNow,
  Error,
  ErrorCaptureStackTrace,
  MapPrototypeDelete,
  MapPrototypeGet,
  MapPrototypeKeys,
  MapPrototypeSet,
  MathCeil,
  MathMin,
  ObjectFreeze,
  PromisePrototypeThen,
  SafeMap,
  StringPrototypeReplaceAll,
  StringPrototypeSlice,
  StringPrototypeSplit,
  StringPrototypeStartsWith,
  SymbolDispose,
  TypeError,
} = primordials;

const { posix: pathPosix, isAbsolute, resolve: resolvePath } = pathMod;

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

class ErrMethodNotImplemented extends Error {
  code;
  constructor(method) {
    super(`The ${method} method is not implemented`);
    this.code = "ERR_METHOD_NOT_IMPLEMENTED";
  }
}

class ErrInvalidState extends Error {
  code;
  constructor(message) {
    super(`Invalid state: ${message}`);
    this.code = "ERR_INVALID_STATE";
  }
}

// =====================================================================
// Stats
// =====================================================================

const S_IFMT = 0o170000;
const S_IFREG = 0o100000;
const S_IFDIR = 0o040000;
const S_IFLNK = 0o120000;
const kDefaultBlockSize = 4096;

// Read uid/gid lazily so we don't trigger Deno's --allow-sys permission
// requirement just by instantiating a VirtualStats; in a virtual fs, the
// owning user is not particularly meaningful anyway.
function safeUid() {
  try {
    // deno-lint-ignore no-explicit-any
    return (globalThis as any).process?.getuid?.() ?? 0;
  } catch {
    return 0;
  }
}
function safeGid() {
  try {
    // deno-lint-ignore no-explicit-any
    return (globalThis as any).process?.getgid?.() ?? 0;
  } catch {
    return 0;
  }
}

class VirtualStats {
  constructor(props) {
    this.dev = props.dev ?? 0;
    this.mode = props.mode;
    this.nlink = props.nlink ?? 1;
    this.uid = props.uid ?? safeUid();
    this.gid = props.gid ?? safeGid();
    this.rdev = props.rdev ?? 0;
    this.blksize = props.blksize ?? kDefaultBlockSize;
    this.ino = props.ino ?? 0;
    this.size = props.size;
    this.blocks = props.blocks ?? MathCeil(props.size / 512);

    this.atimeMs = props.atimeMs;
    this.mtimeMs = props.mtimeMs;
    this.ctimeMs = props.ctimeMs;
    this.birthtimeMs = props.birthtimeMs;

    this.atime = new Date(this.atimeMs);
    this.mtime = new Date(this.mtimeMs);
    this.ctime = new Date(this.ctimeMs);
    this.birthtime = new Date(this.birthtimeMs);
  }

  isFile() {
    return (this.mode & S_IFMT) === S_IFREG;
  }
  isDirectory() {
    return (this.mode & S_IFMT) === S_IFDIR;
  }
  isSymbolicLink() {
    return (this.mode & S_IFMT) === S_IFLNK;
  }
  isBlockDevice() {
    return false;
  }
  isCharacterDevice() {
    return false;
  }
  isFIFO() {
    return false;
  }
  isSocket() {
    return false;
  }
}

function createFileStats(size, options = {}) {
  const now = DateNow();
  return new VirtualStats({
    mode: (options.mode ?? 0o644) | S_IFREG,
    size,
    atimeMs: options.atimeMs ?? now,
    mtimeMs: options.mtimeMs ?? now,
    ctimeMs: options.ctimeMs ?? now,
    birthtimeMs: options.birthtimeMs ?? now,
  });
}

function createDirectoryStats(options = {}) {
  const now = DateNow();
  return new VirtualStats({
    mode: (options.mode ?? 0o755) | S_IFDIR,
    size: kDefaultBlockSize,
    blocks: 8,
    atimeMs: options.atimeMs ?? now,
    mtimeMs: options.mtimeMs ?? now,
    ctimeMs: options.ctimeMs ?? now,
    birthtimeMs: options.birthtimeMs ?? now,
  });
}

function createSymlinkStats(size, options = {}) {
  const now = DateNow();
  return new VirtualStats({
    mode: (options.mode ?? 0o777) | S_IFLNK,
    size,
    atimeMs: options.atimeMs ?? now,
    mtimeMs: options.mtimeMs ?? now,
    ctimeMs: options.ctimeMs ?? now,
    birthtimeMs: options.birthtimeMs ?? now,
  });
}

// =====================================================================
// Dirent
// =====================================================================

const UV_DIRENT_FILE = 1;
const UV_DIRENT_DIR = 2;
const UV_DIRENT_LINK = 3;

class VirtualDirent {
  #name;
  #type;
  #parentPath;
  constructor(name, type, parentPath) {
    this.#name = name;
    this.#type = type;
    this.#parentPath = parentPath;
  }
  get name() {
    return this.#name;
  }
  get parentPath() {
    return this.#parentPath;
  }
  get path() {
    return this.#parentPath;
  }
  isFile() {
    return this.#type === UV_DIRENT_FILE;
  }
  isDirectory() {
    return this.#type === UV_DIRENT_DIR;
  }
  isSymbolicLink() {
    return this.#type === UV_DIRENT_LINK;
  }
  isBlockDevice() {
    return false;
  }
  isCharacterDevice() {
    return false;
  }
  isFIFO() {
    return false;
  }
  isSocket() {
    return false;
  }
}

// =====================================================================
// File descriptor table
// =====================================================================

let nextFd = 10_000;
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
  const fd = nextFd++;
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

  _checkClosed() {
    if (this.#closed) {
      throw createEBADF("read");
    }
  }

  _markClosed() {
    this.#closed = true;
  }

  async read(_buffer, _offset, _length, _position) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("read");
  }

  readSync(_buffer, _offset, _length, _position) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("readSync");
  }

  async write(_buffer, _offset, _length, _position) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("write");
  }

  writeSync(_buffer, _offset, _length, _position) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("writeSync");
  }

  async readFile(_options) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("readFile");
  }

  readFileSync(_options) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("readFileSync");
  }

  async writeFile(_data, _options) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("writeFile");
  }

  writeFileSync(_data, _options) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("writeFileSync");
  }

  async stat(_options) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("stat");
  }

  statSync(_options) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("statSync");
  }

  async truncate(_len) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("truncate");
  }

  truncateSync(_len) {
    this._checkClosed();
    throw new ErrMethodNotImplemented("truncateSync");
  }

  async close() {
    this._markClosed();
  }
  closeSync() {
    this._markClosed();
  }
}

class MemoryFileHandle extends VirtualFileHandle {
  #content;
  #entry;
  #getStats;

  constructor(path, flags, mode, content, entry, getStats) {
    super(path, flags, mode);
    this.#content = content;
    this.#entry = entry;
    this.#getStats = getStats;

    if (flags === "w" || flags === "w+") {
      this.#content = Buffer.alloc(0);
      if (entry) entry.content = this.#content;
    } else if (flags === "a" || flags === "a+") {
      this.position = this.#content.length;
    }
  }

  get content() {
    if (this.#entry?.isDynamic && this.#entry.isDynamic()) {
      return this.#entry.getContentSync();
    }
    return this.#content;
  }

  async getContentAsync() {
    if (this.#entry?.getContentAsync) {
      return this.#entry.getContentAsync();
    }
    return this.#content;
  }

  readSync(buffer, offset, length, position) {
    this._checkClosed();
    const content = this.content;
    const readPos = position !== null && position !== undefined
      ? position
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
    this._checkClosed();
    const writePos = position !== null && position !== undefined
      ? position
      : this.position;
    const data = buffer.subarray(offset, offset + length);
    if (writePos + length > this.#content.length) {
      const newContent = Buffer.alloc(writePos + length);
      this.#content.copy(newContent, 0, 0, this.#content.length);
      this.#content = newContent;
    }
    data.copy(this.#content, writePos);
    if (this.#entry) {
      this.#entry.content = this.#content;
      this.#entry.mtime = DateNow();
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
    this._checkClosed();
    const content = this.content;
    const encoding = typeof options === "string" ? options : options?.encoding;
    if (encoding) return content.toString(encoding);
    return Buffer.from(content);
  }

  async readFile(options) {
    this._checkClosed();
    const content = await this.getContentAsync();
    const encoding = typeof options === "string" ? options : options?.encoding;
    if (encoding) return content.toString(encoding);
    return Buffer.from(content);
  }

  writeFileSync(data, options) {
    this._checkClosed();
    const buf = typeof data === "string"
      ? Buffer.from(data, options?.encoding)
      : data;
    if (this.flags === "a" || this.flags === "a+") {
      const newContent = Buffer.alloc(this.#content.length + buf.length);
      this.#content.copy(newContent, 0);
      buf.copy(newContent, this.#content.length);
      this.#content = newContent;
    } else {
      this.#content = Buffer.from(buf);
    }
    if (this.#entry) {
      this.#entry.content = this.#content;
      this.#entry.mtime = DateNow();
    }
    this.position = this.#content.length;
  }

  async writeFile(data, options) {
    this.writeFileSync(data, options);
  }

  statSync(_options) {
    this._checkClosed();
    if (this.#getStats) return this.#getStats(this.#content.length);
    throw new ErrInvalidState("stats not available");
  }

  async stat(options) {
    return this.statSync(options);
  }

  truncateSync(len = 0) {
    this._checkClosed();
    if (len < this.#content.length) {
      this.#content = this.#content.subarray(0, len);
    } else if (len > this.#content.length) {
      const newContent = Buffer.alloc(len);
      this.#content.copy(newContent, 0, 0, this.#content.length);
      this.#content = newContent;
    }
    if (this.#entry) {
      this.#entry.content = this.#content;
      this.#entry.mtime = DateNow();
    }
  }

  async truncate(len) {
    this.truncateSync(len);
  }
}

// =====================================================================
// VirtualProvider base class
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
    throw new ErrMethodNotImplemented("open");
  }
  openSync(_path, _flags, _mode) {
    throw new ErrMethodNotImplemented("openSync");
  }

  async stat(_path, _options) {
    throw new ErrMethodNotImplemented("stat");
  }
  statSync(_path, _options) {
    throw new ErrMethodNotImplemented("statSync");
  }

  async lstat(path, options) {
    return this.stat(path, options);
  }
  lstatSync(path, options) {
    return this.statSync(path, options);
  }

  async readdir(_path, _options) {
    throw new ErrMethodNotImplemented("readdir");
  }
  readdirSync(_path, _options) {
    throw new ErrMethodNotImplemented("readdirSync");
  }

  async mkdir(path, _options) {
    if (this.readonly) throw createEROFS("mkdir", path);
    throw new ErrMethodNotImplemented("mkdir");
  }
  mkdirSync(path, _options) {
    if (this.readonly) throw createEROFS("mkdir", path);
    throw new ErrMethodNotImplemented("mkdirSync");
  }

  async rmdir(path) {
    if (this.readonly) throw createEROFS("rmdir", path);
    throw new ErrMethodNotImplemented("rmdir");
  }
  rmdirSync(path) {
    if (this.readonly) throw createEROFS("rmdir", path);
    throw new ErrMethodNotImplemented("rmdirSync");
  }

  async unlink(path) {
    if (this.readonly) throw createEROFS("unlink", path);
    throw new ErrMethodNotImplemented("unlink");
  }
  unlinkSync(path) {
    if (this.readonly) throw createEROFS("unlink", path);
    throw new ErrMethodNotImplemented("unlinkSync");
  }

  async rename(oldPath, _newPath) {
    if (this.readonly) throw createEROFS("rename", oldPath);
    throw new ErrMethodNotImplemented("rename");
  }
  renameSync(oldPath, _newPath) {
    if (this.readonly) throw createEROFS("rename", oldPath);
    throw new ErrMethodNotImplemented("renameSync");
  }

  // === DEFAULT IMPLEMENTATIONS ===

  async readFile(path, options) {
    const handle = await this.open(path, "r");
    try {
      return await handle.readFile(options);
    } finally {
      await handle.close();
    }
  }

  readFileSync(path, options) {
    const handle = this.openSync(path, "r");
    try {
      return handle.readFileSync(options);
    } finally {
      handle.closeSync();
    }
  }

  async writeFile(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const handle = await this.open(path, "w", options?.mode);
    try {
      await handle.writeFile(data, options);
    } finally {
      await handle.close();
    }
  }

  writeFileSync(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const handle = this.openSync(path, "w", options?.mode);
    try {
      handle.writeFileSync(data, options);
    } finally {
      handle.closeSync();
    }
  }

  async appendFile(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const handle = await this.open(path, "a", options?.mode);
    try {
      await handle.writeFile(data, options);
    } finally {
      await handle.close();
    }
  }

  appendFileSync(path, data, options) {
    if (this.readonly) throw createEROFS("open", path);
    const handle = this.openSync(path, "a", options?.mode);
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

  async copyFile(src, dest, _mode) {
    if (this.readonly) throw createEROFS("copyfile", dest);
    const content = await this.readFile(src);
    await this.writeFile(dest, content);
  }

  copyFileSync(src, dest, _mode) {
    if (this.readonly) throw createEROFS("copyfile", dest);
    const content = this.readFileSync(src);
    this.writeFileSync(dest, content);
  }

  internalModuleStat(path) {
    try {
      const stats = this.statSync(path);
      if (stats.isDirectory()) return 1;
      return 0;
    } catch {
      return -2;
    }
  }

  async realpath(path, _options) {
    await this.stat(path);
    return path;
  }

  realpathSync(path, _options) {
    this.statSync(path);
    return path;
  }

  async access(path, _mode) {
    await this.stat(path);
  }
  accessSync(path, _mode) {
    this.statSync(path);
  }

  // === SYMLINKS ===

  async readlink(_path, _options) {
    throw new ErrMethodNotImplemented("readlink");
  }
  readlinkSync(_path, _options) {
    throw new ErrMethodNotImplemented("readlinkSync");
  }

  async symlink(_target, path, _type) {
    if (this.readonly) throw createEROFS("symlink", path);
    throw new ErrMethodNotImplemented("symlink");
  }
  symlinkSync(_target, path, _type) {
    if (this.readonly) throw createEROFS("symlink", path);
    throw new ErrMethodNotImplemented("symlinkSync");
  }

  // === WATCH ===
  watch(_path, _options) {
    throw new ErrMethodNotImplemented("watch");
  }
  watchAsync(_path, _options) {
    throw new ErrMethodNotImplemented("watchAsync");
  }
  watchFile(_path, _options) {
    throw new ErrMethodNotImplemented("watchFile");
  }
  unwatchFile(_path, _listener) {
    throw new ErrMethodNotImplemented("unwatchFile");
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
  constructor(type, options = {}) {
    this.type = type;
    this.mode = options.mode ?? (type === TYPE_DIR ? 0o755 : 0o644);
    this.content = null;
    this.contentProvider = null;
    this.target = null;
    this.children = null;
    this.populate = null;
    this.populated = true;
    const now = DateNow();
    this.mtime = now;
    this.ctime = now;
    this.birthtime = now;
  }

  getContentSync() {
    if (this.contentProvider !== null) {
      const result = this.contentProvider();
      if (result && typeof result.then === "function") {
        throw new ErrInvalidState(
          "cannot use sync API with async content provider",
        );
      }
      return typeof result === "string" ? Buffer.from(result) : result;
    }
    return this.content;
  }

  async getContentAsync() {
    if (this.contentProvider !== null) {
      const result = await this.contentProvider();
      return typeof result === "string" ? Buffer.from(result) : result;
    }
    return this.content;
  }

  isDynamic() {
    return this.contentProvider !== null;
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

class MemoryProvider extends VirtualProvider {
  #root;
  #readonly;

  constructor() {
    super();
    this.#root = new MemoryEntry(TYPE_DIR);
    this.#root.children = new SafeMap();
    this.#readonly = false;
  }

  get readonly() {
    return this.#readonly;
  }
  get supportsWatch() {
    return false;
  }
  get supportsSymlinks() {
    return true;
  }

  setReadOnly() {
    this.#readonly = true;
  }

  #normalizePath(path) {
    let normalized = StringPrototypeReplaceAll(path, "\\", "/");
    if (!StringPrototypeStartsWith(normalized, "/")) {
      normalized = "/" + normalized;
    }
    return pathPosix.normalize(normalized);
  }

  #splitPath(path) {
    if (path === "/") return [];
    return StringPrototypeSplit(StringPrototypeSlice(path, 1), "/");
  }

  #getParentPath(path) {
    if (path === "/") return null;
    return pathPosix.dirname(path);
  }

  #getBaseName(path) {
    return pathPosix.basename(path);
  }

  #resolveSymlinkTarget(symlinkPath, target) {
    if (StringPrototypeStartsWith(target, "/")) {
      return this.#normalizePath(target);
    }
    const parentPath = this.#getParentPath(symlinkPath) || "/";
    return this.#normalizePath(pathPosix.join(parentPath, target));
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

      if (current.isSymbolicLink() && followSymlinks) {
        if (depth >= kMaxSymlinkDepth) {
          return { entry: null, resolvedPath: null, eloop: true };
        }
        const targetPath = this.#resolveSymlinkTarget(
          currentPath,
          current.target,
        );
        const result = this.#lookupEntry(targetPath, true, depth + 1);
        if (result.eloop) return result;
        if (!result.entry) return { entry: null, resolvedPath: null };
        current = result.entry;
        currentPath = result.resolvedPath;
      }

      if (!current.isDirectory()) {
        return { entry: null, resolvedPath: null };
      }

      this.#ensurePopulated(current);

      const entry = MapPrototypeGet(current.children, segment);
      if (!entry) return { entry: null, resolvedPath: null };

      currentPath = pathPosix.join(currentPath, segment);
      current = entry;
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
    const result = this.#lookupEntry(path, followSymlinks);
    if (result.eloop) throw createELOOP(syscall, path);
    if (!result.entry) throw createENOENT(syscall, path);
    return result.entry;
  }

  #ensureParent(path, create, syscall) {
    const parentPath = this.#getParentPath(path);
    if (parentPath === null) return this.#root;

    const segments = this.#splitPath(parentPath);
    let current = this.#root;

    for (let i = 0; i < segments.length; i++) {
      const segment = segments[i];

      if (current.isSymbolicLink()) {
        const currentPath = pathPosix.join(
          "/",
          ArrayPrototypeJoin(ArrayPrototypeSlice(segments, 0, i), "/"),
        );
        const targetPath = this.#resolveSymlinkTarget(
          currentPath,
          current.target,
        );
        const result = this.#lookupEntry(targetPath, true, 0);
        if (!result.entry) throw createENOENT(syscall, path);
        current = result.entry;
      }

      if (!current.isDirectory()) throw createENOTDIR(syscall, path);

      this.#ensurePopulated(current);

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

    if (!current.isDirectory()) throw createENOTDIR(syscall, path);
    this.#ensurePopulated(current);
    return current;
  }

  #createStats(entry, size) {
    const options = {
      mode: entry.mode,
      mtimeMs: entry.mtime,
      ctimeMs: entry.ctime,
      birthtimeMs: entry.birthtime,
    };
    if (entry.isFile()) {
      return createFileStats(
        size !== undefined ? size : entry.content.length,
        options,
      );
    } else if (entry.isDirectory()) {
      return createDirectoryStats(options);
    } else if (entry.isSymbolicLink()) {
      return createSymlinkStats(entry.target.length, options);
    }
    throw new ErrInvalidState("Unknown entry type");
  }

  #ensurePopulated(entry) {
    if (entry.isDirectory() && !entry.populated && entry.populate) {
      const scoped = {
        addFile: (name, content, opts) => {
          const fileEntry = new MemoryEntry(TYPE_FILE, opts);
          if (typeof content === "function") {
            fileEntry.content = Buffer.alloc(0);
            fileEntry.contentProvider = content;
          } else {
            fileEntry.content = typeof content === "string"
              ? Buffer.from(content)
              : content;
          }
          MapPrototypeSet(entry.children, name, fileEntry);
        },
        addDirectory: (name, populate, opts) => {
          const dirEntry = new MemoryEntry(TYPE_DIR, opts);
          dirEntry.children = new SafeMap();
          if (typeof populate === "function") {
            dirEntry.populate = populate;
            dirEntry.populated = false;
          }
          MapPrototypeSet(entry.children, name, dirEntry);
        },
        addSymlink: (name, target, opts) => {
          const symlinkEntry = new MemoryEntry(TYPE_SYMLINK, opts);
          symlinkEntry.target = target;
          MapPrototypeSet(entry.children, name, symlinkEntry);
        },
      };
      entry.populate(scoped);
      entry.populated = true;
    }
  }

  openSync(path, flags, mode) {
    const normalized = this.#normalizePath(path);
    const isCreate = flags === "w" || flags === "w+" ||
      flags === "a" || flags === "a+";

    if (this.readonly && isCreate) throw createEROFS("open", path);

    let entry;
    try {
      entry = this.#getEntry(normalized, "open");
    } catch (err) {
      if (err.code === "ENOENT" && isCreate) {
        const parent = this.#ensureParent(normalized, true, "open");
        const name = this.#getBaseName(normalized);
        entry = new MemoryEntry(TYPE_FILE, { mode });
        entry.content = Buffer.alloc(0);
        MapPrototypeSet(parent.children, name, entry);
      } else {
        throw err;
      }
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

  statSync(path, _options) {
    const entry = this.#getEntry(path, "stat", true);
    return this.#createStats(entry);
  }

  async stat(path, options) {
    return this.statSync(path, options);
  }

  lstatSync(path, _options) {
    const entry = this.#getEntry(path, "lstat", false);
    return this.#createStats(entry);
  }

  async lstat(path, options) {
    return this.lstatSync(path, options);
  }

  readdirSync(path, options) {
    const entry = this.#getEntry(path, "scandir", true);
    if (!entry.isDirectory()) throw createENOTDIR("scandir", path);
    this.#ensurePopulated(entry);

    const names = ArrayFrom(MapPrototypeKeys(entry.children));

    if (options?.withFileTypes) {
      const normalized = this.#normalizePath(path);
      const dirents = [];
      for (const name of names) {
        const childEntry = MapPrototypeGet(entry.children, name);
        let type;
        if (childEntry.isSymbolicLink()) type = UV_DIRENT_LINK;
        else if (childEntry.isDirectory()) type = UV_DIRENT_DIR;
        else type = UV_DIRENT_FILE;
        ArrayPrototypePush(
          dirents,
          new VirtualDirent(name, type, normalized),
        );
      }
      return dirents;
    }
    return names;
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
      for (const segment of segments) {
        let entry = MapPrototypeGet(current.children, segment);
        if (!entry) {
          entry = new MemoryEntry(TYPE_DIR, { mode: options?.mode });
          entry.children = new SafeMap();
          MapPrototypeSet(current.children, segment, entry);
        } else if (!entry.isDirectory()) {
          throw createENOTDIR("mkdir", path);
        }
        current = entry;
      }
    } else {
      const parent = this.#ensureParent(normalized, false, "mkdir");
      const name = this.#getBaseName(normalized);
      const entry = new MemoryEntry(TYPE_DIR, { mode: options?.mode });
      entry.children = new SafeMap();
      MapPrototypeSet(parent.children, name, entry);
    }

    return recursive ? normalized : undefined;
  }

  async mkdir(path, options) {
    return this.mkdirSync(path, options);
  }

  rmdirSync(path) {
    if (this.readonly) throw createEROFS("rmdir", path);
    const normalized = this.#normalizePath(path);
    const entry = this.#getEntry(normalized, "rmdir", true);
    if (!entry.isDirectory()) throw createENOTDIR("rmdir", path);
    if (entry.children.size > 0) throw createENOTEMPTY("rmdir", path);
    const parent = this.#ensureParent(normalized, false, "rmdir");
    const name = this.#getBaseName(normalized);
    MapPrototypeDelete(parent.children, name);
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
    const name = this.#getBaseName(normalized);
    MapPrototypeDelete(parent.children, name);
  }

  async unlink(path) {
    this.unlinkSync(path);
  }

  renameSync(oldPath, newPath) {
    if (this.readonly) throw createEROFS("rename", oldPath);
    const normalizedOld = this.#normalizePath(oldPath);
    const normalizedNew = this.#normalizePath(newPath);
    const entry = this.#getEntry(normalizedOld, "rename", false);
    const oldParent = this.#ensureParent(normalizedOld, false, "rename");
    const oldName = this.#getBaseName(normalizedOld);
    MapPrototypeDelete(oldParent.children, oldName);
    const newParent = this.#ensureParent(normalizedNew, true, "rename");
    const newName = this.#getBaseName(normalizedNew);
    MapPrototypeSet(newParent.children, newName, entry);
  }

  async rename(oldPath, newPath) {
    this.renameSync(oldPath, newPath);
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
    const parent = this.#ensureParent(normalized, true, "symlink");
    const name = this.#getBaseName(normalized);
    const entry = new MemoryEntry(TYPE_SYMLINK);
    entry.target = target;
    MapPrototypeSet(parent.children, name, entry);
  }

  async symlink(target, path, type) {
    this.symlinkSync(target, path, type);
  }

  realpathSync(path, _options) {
    const result = this.#lookupEntry(path, true, 0);
    if (result.eloop) throw createELOOP("realpath", path);
    if (!result.entry) throw createENOENT("realpath", path);
    return result.resolvedPath;
  }

  async realpath(path, options) {
    return this.realpathSync(path, options);
  }
}

// =====================================================================
// Routing helpers
// =====================================================================

function isPathSeparator(ch) {
  return ch === "/" || ch === "\\";
}

function isUnderMountPoint(normalizedPath, mountPoint) {
  if (normalizedPath === mountPoint) return true;
  if (!StringPrototypeStartsWith(normalizedPath, mountPoint)) return false;
  return isPathSeparator(normalizedPath[mountPoint.length]) ||
    isPathSeparator(mountPoint[mountPoint.length - 1]);
}

function getRelativePath(normalizedPath, mountPoint) {
  if (normalizedPath === mountPoint) return "/";
  if (mountPoint === "/") return normalizedPath;
  return StringPrototypeSlice(normalizedPath, mountPoint.length);
}

function normalizeVFSPath(inputPath) {
  if (StringPrototypeStartsWith(inputPath, "/")) {
    return pathPosix.normalize(inputPath);
  }
  return pathMod.normalize(inputPath);
}

function joinVFSPath(base, part) {
  if (StringPrototypeStartsWith(base, "/")) {
    return pathPosix.join(base, part);
  }
  return pathMod.join(base, part);
}

// =====================================================================
// VirtualReadStream - defined lazily so node:stream isn't pulled in until
// createReadStream() is actually called.
// =====================================================================

let VirtualReadStreamCtor = null;

function getVirtualReadStreamCtor() {
  if (VirtualReadStreamCtor !== null) return VirtualReadStreamCtor;
  const Readable = lazyStream().Readable;

  class VirtualReadStream extends Readable {
    #vfs;
    #path;
    #fd = null;
    #end;
    #pos;
    #content = null;
    #destroyed = false;
    #autoClose;

    constructor(vfs, filePath, options = {}) {
      const {
        start = 0,
        end = Infinity,
        highWaterMark = 64 * 1024,
        encoding,
        ...streamOptions
      } = options;

      super({ ...streamOptions, highWaterMark, encoding });

      this.#vfs = vfs;
      this.#path = filePath;
      this.#end = end;
      this.#pos = start;
      this.#autoClose = options.autoClose !== false;

      // Open synchronously here so subsequent _read calls always find a
      // valid fd. Errors are reported via the 'error' event on the next
      // microtask to match the Node fs.createReadStream contract.
      try {
        this.#fd = this.#vfs.openSync(this.#path);
        queueMicrotask(() => {
          if (!this.#destroyed) {
            this.emit("open", this.#fd);
            this.emit("ready");
          }
        });
      } catch (err) {
        queueMicrotask(() => this.destroy(err));
      }
    }

    get path() {
      return this.#path;
    }

    _read(size) {
      if (this.#destroyed || this.#fd === null) {
        return;
      }

      if (this.#content === null) {
        try {
          const vfd = getVirtualFd(this.#fd);
          if (!vfd) {
            this.destroy(createEBADF("read"));
            return;
          }
          this.#content = vfd.entry.readFileSync();
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
      const chunk = this.#content.subarray(
        this.#pos,
        this.#pos + bytesToRead,
      );
      this.#pos += bytesToRead;
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
      this.#destroyed = true;
      if (this.#autoClose) this.#close();
      callback(err);
    }
  }

  VirtualReadStreamCtor = VirtualReadStream;
  return VirtualReadStream;
}

// =====================================================================
// VirtualFileSystem
// =====================================================================

const kEmptyObject = ObjectFreeze({ __proto__: null });

class VirtualFileSystem {
  #provider;
  #mountPoint;
  #mounted;
  #overlay;
  #moduleHooks;
  #promises;
  #virtualCwd;
  #virtualCwdEnabled;
  #originalChdir;
  #originalCwd;

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

    if (
      options.moduleHooks !== undefined &&
      typeof options.moduleHooks !== "boolean"
    ) {
      throw new TypeError("options.moduleHooks must be a boolean");
    }
    if (
      options.virtualCwd !== undefined &&
      typeof options.virtualCwd !== "boolean"
    ) {
      throw new TypeError("options.virtualCwd must be a boolean");
    }
    if (
      options.overlay !== undefined && typeof options.overlay !== "boolean"
    ) {
      throw new TypeError("options.overlay must be a boolean");
    }

    this.#provider = provider ?? new MemoryProvider();
    this.#mountPoint = null;
    this.#mounted = false;
    this.#overlay = options.overlay === true;
    this.#moduleHooks = options.moduleHooks !== false;
    this.#promises = null;
    this.#virtualCwdEnabled = options.virtualCwd === true;
    this.#virtualCwd = null;
    this.#originalChdir = null;
    this.#originalCwd = null;
  }

  get provider() {
    return this.#provider;
  }
  get mountPoint() {
    return this.#mountPoint;
  }
  get mounted() {
    return this.#mounted;
  }
  get readonly() {
    return this.#provider.readonly;
  }
  get overlay() {
    return this.#overlay;
  }
  get virtualCwdEnabled() {
    return this.#virtualCwdEnabled;
  }

  cwd() {
    if (!this.#virtualCwdEnabled) {
      throw new ErrInvalidState("virtual cwd is not enabled");
    }
    return this.#virtualCwd;
  }

  chdir(dirPath) {
    if (!this.#virtualCwdEnabled) {
      throw new ErrInvalidState("virtual cwd is not enabled");
    }
    const providerPath = this.#toProviderPath(dirPath);
    const stats = this.#provider.statSync(providerPath);
    if (!stats.isDirectory()) throw createENOTDIR("chdir", dirPath);
    this.#virtualCwd = this.#toMountedPath(providerPath);
  }

  resolvePath(inputPath) {
    if (isAbsolute(inputPath)) return normalizeVFSPath(inputPath);
    if (this.#virtualCwdEnabled && this.#virtualCwd !== null) {
      const resolved = `${this.#virtualCwd}/${inputPath}`;
      return normalizeVFSPath(resolved);
    }
    return resolvePath(inputPath);
  }

  mount(prefix) {
    if (this.#mounted) {
      throw new ErrInvalidState("VFS is already mounted");
    }
    this.#mountPoint = normalizeVFSPath(prefix);
    this.#mounted = true;
    if (this.#virtualCwdEnabled) this.#hookProcessCwd();

    // deno-lint-ignore no-explicit-any
    const proc = (globalThis as any).process;
    if (proc && typeof proc.emit === "function") {
      proc.emit("vfs-mount", {
        mountPoint: this.#mountPoint,
        overlay: this.#overlay,
        readonly: this.#provider.readonly,
      });
    }
    return this;
  }

  unmount() {
    if (this.#mounted) {
      // deno-lint-ignore no-explicit-any
      const proc = (globalThis as any).process;
      if (proc && typeof proc.emit === "function") {
        proc.emit("vfs-unmount", {
          mountPoint: this.#mountPoint,
          overlay: this.#overlay,
          readonly: this.#provider.readonly,
        });
      }
    }
    this.#unhookProcessCwd();
    this.#mountPoint = null;
    this.#mounted = false;
    this.#virtualCwd = null;
  }

  [SymbolDispose]() {
    if (this.#mounted) this.unmount();
  }

  #hookProcessCwd() {
    if (this.#originalChdir !== null) return;
    // deno-lint-ignore no-explicit-any
    const proc = (globalThis as any).process;
    if (!proc) return;
    // deno-lint-ignore no-this-alias
    const vfs = this;
    this.#originalChdir = proc.chdir;
    this.#originalCwd = proc.cwd;
    proc.chdir = function chdir(directory) {
      const normalized = isAbsolute(directory)
        ? normalizeVFSPath(directory)
        : resolvePath(directory);
      if (vfs.shouldHandle(normalized)) {
        vfs.chdir(normalized);
        return;
      }
      return vfs.#originalChdir.call(proc, directory);
    };
    proc.cwd = function cwd() {
      if (vfs.#virtualCwd !== null) return vfs.#virtualCwd;
      return vfs.#originalCwd.call(proc);
    };
  }

  #unhookProcessCwd() {
    if (this.#originalChdir === null) return;
    // deno-lint-ignore no-explicit-any
    const proc = (globalThis as any).process;
    if (proc) {
      proc.chdir = this.#originalChdir;
      proc.cwd = this.#originalCwd;
    }
    this.#originalChdir = null;
    this.#originalCwd = null;
  }

  #toProviderPath(inputPath) {
    const resolved = this.resolvePath(inputPath);
    if (this.#mounted && this.#mountPoint) {
      if (!isUnderMountPoint(resolved, this.#mountPoint)) {
        throw createENOENT("open", inputPath);
      }
      return getRelativePath(resolved, this.#mountPoint);
    }
    return resolved;
  }

  #toMountedPath(providerPath) {
    if (this.#mounted && this.#mountPoint) {
      // Avoid producing a trailing slash when joining against root.
      if (providerPath === "/") return this.#mountPoint;
      return joinVFSPath(this.#mountPoint, providerPath);
    }
    return providerPath;
  }

  shouldHandle(inputPath) {
    if (!this.#mounted || !this.#mountPoint) return false;
    const normalized = normalizeVFSPath(inputPath);
    if (!isUnderMountPoint(normalized, this.#mountPoint)) return false;
    if (this.#overlay) {
      try {
        const providerPath = getRelativePath(normalized, this.#mountPoint);
        return this.#provider.existsSync(providerPath);
      } catch {
        return false;
      }
    }
    return true;
  }

  // === Sync API ===

  existsSync(filePath) {
    try {
      const providerPath = this.#toProviderPath(filePath);
      return this.#provider.existsSync(providerPath);
    } catch {
      return false;
    }
  }

  statSync(filePath, options) {
    return this.#provider.statSync(this.#toProviderPath(filePath), options);
  }

  lstatSync(filePath, options) {
    return this.#provider.lstatSync(this.#toProviderPath(filePath), options);
  }

  readFileSync(filePath, options) {
    return this.#provider.readFileSync(
      this.#toProviderPath(filePath),
      options,
    );
  }

  writeFileSync(filePath, data, options) {
    this.#provider.writeFileSync(
      this.#toProviderPath(filePath),
      data,
      options,
    );
  }

  appendFileSync(filePath, data, options) {
    this.#provider.appendFileSync(
      this.#toProviderPath(filePath),
      data,
      options,
    );
  }

  readdirSync(dirPath, options) {
    return this.#provider.readdirSync(this.#toProviderPath(dirPath), options);
  }

  mkdirSync(dirPath, options) {
    const result = this.#provider.mkdirSync(
      this.#toProviderPath(dirPath),
      options,
    );
    if (result !== undefined) return this.#toMountedPath(result);
    return undefined;
  }

  rmdirSync(dirPath) {
    this.#provider.rmdirSync(this.#toProviderPath(dirPath));
  }

  unlinkSync(filePath) {
    this.#provider.unlinkSync(this.#toProviderPath(filePath));
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

  realpathSync(filePath, options) {
    return this.#toMountedPath(
      this.#provider.realpathSync(
        this.#toProviderPath(filePath),
        options,
      ),
    );
  }

  readlinkSync(linkPath, options) {
    return this.#provider.readlinkSync(
      this.#toProviderPath(linkPath),
      options,
    );
  }

  symlinkSync(target, path, type) {
    this.#provider.symlinkSync(target, this.#toProviderPath(path), type);
  }

  accessSync(filePath, mode) {
    this.#provider.accessSync(this.#toProviderPath(filePath), mode);
  }

  internalModuleStat(filePath) {
    try {
      return this.#provider.internalModuleStat(
        this.#toProviderPath(filePath),
      );
    } catch {
      return -2;
    }
  }

  // === File descriptor ops ===

  openSync(filePath, flags = "r", mode) {
    const handle = this.#provider.openSync(
      this.#toProviderPath(filePath),
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

  fstatSync(fd, options) {
    const vfd = getVirtualFd(fd);
    if (!vfd) throw createEBADF("fstat");
    return vfd.entry.statSync(options);
  }

  // === Callback API ===

  readFile(filePath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.readFile(this.#toProviderPath(filePath), options),
      (data) => callback(null, data),
      (err) => callback(err),
    );
  }

  writeFile(filePath, data, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.writeFile(this.#toProviderPath(filePath), data, options),
      () => callback(null),
      (err) => callback(err),
    );
  }

  stat(filePath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.stat(this.#toProviderPath(filePath), options),
      (stats) => callback(null, stats),
      (err) => callback(err),
    );
  }

  lstat(filePath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.lstat(this.#toProviderPath(filePath), options),
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

  realpath(filePath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.realpath(this.#toProviderPath(filePath), options),
      (realPath) => callback(null, this.#toMountedPath(realPath)),
      (err) => callback(err),
    );
  }

  readlink(linkPath, options, callback) {
    if (typeof options === "function") {
      callback = options;
      options = undefined;
    }
    PromisePrototypeThen(
      this.#provider.readlink(this.#toProviderPath(linkPath), options),
      (target) => callback(null, target),
      (err) => callback(err),
    );
  }

  access(filePath, mode, callback) {
    if (typeof mode === "function") {
      callback = mode;
      mode = undefined;
    }
    PromisePrototypeThen(
      this.#provider.access(this.#toProviderPath(filePath), mode),
      () => callback(null),
      (err) => callback(err),
    );
  }

  open(filePath, flags, mode, callback) {
    if (typeof flags === "function") {
      callback = flags;
      flags = "r";
      mode = undefined;
    } else if (typeof mode === "function") {
      callback = mode;
      mode = undefined;
    }
    const providerPath = this.#toProviderPath(filePath);
    PromisePrototypeThen(
      this.#provider.open(providerPath, flags, mode),
      (handle) => {
        const fd = openVirtualFd(handle);
        callback(null, fd);
      },
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

  // === Streams ===

  createReadStream(filePath, options) {
    const Ctor = getVirtualReadStreamCtor();
    return new Ctor(this, filePath, options);
  }

  // === Watch (not supported) ===

  watch(_filePath, _options, _listener) {
    throw new ErrMethodNotImplemented("watch");
  }

  watchFile(_filePath, _options, _listener) {
    throw new ErrMethodNotImplemented("watchFile");
  }

  unwatchFile(_filePath, _listener) {
    throw new ErrMethodNotImplemented("unwatchFile");
  }

  // === Promises API ===

  get promises() {
    if (this.#promises === null) {
      this.#promises = this.#createPromisesAPI();
    }
    return this.#promises;
  }

  #createPromisesAPI() {
    const provider = this.#provider;
    const toProviderPath = (p) => this.#toProviderPath(p);
    const toMountedPath = (p) => this.#toMountedPath(p);

    return ObjectFreeze({
      async readFile(filePath, options) {
        return provider.readFile(toProviderPath(filePath), options);
      },
      async writeFile(filePath, data, options) {
        return provider.writeFile(toProviderPath(filePath), data, options);
      },
      async appendFile(filePath, data, options) {
        return provider.appendFile(toProviderPath(filePath), data, options);
      },
      async stat(filePath, options) {
        return provider.stat(toProviderPath(filePath), options);
      },
      async lstat(filePath, options) {
        return provider.lstat(toProviderPath(filePath), options);
      },
      async readdir(dirPath, options) {
        return provider.readdir(toProviderPath(dirPath), options);
      },
      async mkdir(dirPath, options) {
        const result = await provider.mkdir(toProviderPath(dirPath), options);
        if (result !== undefined) return toMountedPath(result);
        return undefined;
      },
      async rmdir(dirPath) {
        return provider.rmdir(toProviderPath(dirPath));
      },
      async unlink(filePath) {
        return provider.unlink(toProviderPath(filePath));
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
      async realpath(filePath, options) {
        const realPath = await provider.realpath(
          toProviderPath(filePath),
          options,
        );
        return toMountedPath(realPath);
      },
      async readlink(linkPath, options) {
        return provider.readlink(toProviderPath(linkPath), options);
      },
      async symlink(target, path, type) {
        return provider.symlink(target, toProviderPath(path), type);
      },
      async access(filePath, mode) {
        return provider.access(toProviderPath(filePath), mode);
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

return {
  create,
  VirtualFileSystem,
  VirtualProvider,
  MemoryProvider,
  VirtualFileHandle,
  MemoryFileHandle,
  VirtualStats,
  VirtualDirent,
  default: {
    create,
    VirtualFileSystem,
    VirtualProvider,
    MemoryProvider,
    VirtualFileHandle,
    MemoryFileHandle,
    VirtualStats,
    VirtualDirent,
  },
};
})();
