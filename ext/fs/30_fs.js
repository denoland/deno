// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const ops = core.ops;
const primordials = globalThis.__bootstrap.primordials;
const {
  ArrayPrototypeFilter,
  Date,
  DatePrototype,
  Error,
  Function,
  MathTrunc,
  ObjectEntries,
  ObjectPrototypeIsPrototypeOf,
  ObjectValues,
  SymbolAsyncIterator,
  SymbolIterator,
  Uint32Array,
} = primordials;
import { read, readSync, write, writeSync } from "ext:deno_io/12_io.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import {
  readableStreamForRid,
  ReadableStreamPrototype,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";
import { pathFromURL } from "ext:deno_web/00_infra.js";

function chmodSync(path, mode) {
  ops.op_chmod_sync(pathFromURL(path), mode);
}

async function chmod(path, mode) {
  await core.opAsync2("op_chmod_async", pathFromURL(path), mode);
}

function chownSync(
  path,
  uid,
  gid,
) {
  ops.op_chown_sync(pathFromURL(path), uid, gid);
}

async function chown(
  path,
  uid,
  gid,
) {
  await core.opAsync(
    "op_chown_async",
    pathFromURL(path),
    uid,
    gid,
  );
}

function copyFileSync(
  fromPath,
  toPath,
) {
  ops.op_copy_file_sync(
    pathFromURL(fromPath),
    pathFromURL(toPath),
  );
}

async function copyFile(
  fromPath,
  toPath,
) {
  await core.opAsync(
    "op_copy_file_async",
    pathFromURL(fromPath),
    pathFromURL(toPath),
  );
}

function cwd() {
  return ops.op_cwd();
}

function chdir(directory) {
  ops.op_chdir(pathFromURL(directory));
}

function makeTempDirSync(options = {}) {
  return ops.op_make_temp_dir_sync(options.dir, options.prefix, options.suffix);
}

function makeTempDir(options = {}) {
  return core.opAsync(
    "op_make_temp_dir_async",
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function makeTempFileSync(options = {}) {
  return ops.op_make_temp_file_sync(
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function makeTempFile(options = {}) {
  return core.opAsync(
    "op_make_temp_file_async",
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function mkdirSync(path, options) {
  ops.op_mkdir_sync(
    pathFromURL(path),
    options?.recursive ?? false,
    options?.mode,
  );
}

async function mkdir(path, options) {
  await core.opAsync(
    "op_mkdir_async",
    pathFromURL(path),
    options?.recursive ?? false,
    options?.mode,
  );
}

function readDirSync(path) {
  return ops.op_read_dir_sync(pathFromURL(path))[
    SymbolIterator
  ]();
}

function readDir(path) {
  const array = core.opAsync(
    "op_read_dir_async",
    pathFromURL(path),
  );
  return {
    async *[SymbolAsyncIterator]() {
      const dir = await array;
      for (let i = 0; i < dir.length; ++i) {
        yield dir[i];
      }
    },
  };
}

function readLinkSync(path) {
  return ops.op_read_link_sync(pathFromURL(path));
}

function readLink(path) {
  return core.opAsync("op_read_link_async", pathFromURL(path));
}

function realPathSync(path) {
  return ops.op_realpath_sync(pathFromURL(path));
}

function realPath(path) {
  return core.opAsync("op_realpath_async", pathFromURL(path));
}

function removeSync(
  path,
  options = {},
) {
  ops.op_remove_sync(
    pathFromURL(path),
    !!options.recursive,
  );
}

async function remove(
  path,
  options = {},
) {
  await core.opAsync(
    "op_remove_async",
    pathFromURL(path),
    !!options.recursive,
  );
}

function renameSync(oldpath, newpath) {
  ops.op_rename_sync(
    pathFromURL(oldpath),
    pathFromURL(newpath),
  );
}

async function rename(oldpath, newpath) {
  await core.opAsync(
    "op_rename_async",
    pathFromURL(oldpath),
    pathFromURL(newpath),
  );
}

// Extract the FsStat object from the encoded buffer.
// See `runtime/ops/fs.rs` for the encoder.
//
// This is not a general purpose decoder. There are 4 types:
//
// 1. date
//  offset += 4
//  1/0 | extra padding | high u32 | low u32
//  if date[0] == 1, new Date(u64) else null
//
// 2. bool
//  offset += 2
//  1/0 | extra padding
//
// 3. u64
//  offset += 2
//  high u32 | low u32
//
// 4. ?u64 converts a zero u64 value to JS null on Windows.
function createByteStruct(types) {
  // types can be "date", "bool" or "u64".
  let offset = 0;
  let str =
    'const unix = Deno.build.os === "darwin" || Deno.build.os === "linux"; return {';
  const typeEntries = ObjectEntries(types);
  for (let i = 0; i < typeEntries.length; ++i) {
    let { 0: name, 1: type } = typeEntries[i];

    const optional = type.startsWith("?");
    if (optional) type = type.slice(1);

    if (type == "u64") {
      if (!optional) {
        str += `${name}: view[${offset}] + view[${offset + 1}] * 2**32,`;
      } else {
        str += `${name}: (unix ? (view[${offset}] + view[${
          offset + 1
        }] * 2**32) : (view[${offset}] + view[${
          offset + 1
        }] * 2**32) || null),`;
      }
    } else if (type == "date") {
      str += `${name}: view[${offset}] === 0 ? null : new Date(view[${
        offset + 2
      }] + view[${offset + 3}] * 2**32),`;
      offset += 2;
    } else {
      str += `${name}: !!(view[${offset}] + view[${offset + 1}] * 2**32),`;
    }
    offset += 2;
  }
  str += "};";
  // ...so you don't like eval huh? don't worry, it only executes during snapshot :)
  return [new Function("view", str), new Uint32Array(offset)];
}

const { 0: statStruct, 1: statBuf } = createByteStruct({
  isFile: "bool",
  isDirectory: "bool",
  isSymlink: "bool",
  size: "u64",
  mtime: "date",
  atime: "date",
  birthtime: "date",
  dev: "u64",
  ino: "?u64",
  mode: "?u64",
  nlink: "?u64",
  uid: "?u64",
  gid: "?u64",
  rdev: "?u64",
  blksize: "?u64",
  blocks: "?u64",
});

function parseFileInfo(response) {
  const unix = core.build.os === "darwin" || core.build.os === "linux";
  return {
    isFile: response.isFile,
    isDirectory: response.isDirectory,
    isSymlink: response.isSymlink,
    size: response.size,
    mtime: response.mtimeSet !== null ? new Date(response.mtime) : null,
    atime: response.atimeSet !== null ? new Date(response.atime) : null,
    birthtime: response.birthtimeSet !== null
      ? new Date(response.birthtime)
      : null,
    dev: response.dev,
    ino: unix ? response.ino : null,
    mode: unix ? response.mode : null,
    nlink: unix ? response.nlink : null,
    uid: unix ? response.uid : null,
    gid: unix ? response.gid : null,
    rdev: unix ? response.rdev : null,
    blksize: unix ? response.blksize : null,
    blocks: unix ? response.blocks : null,
  };
}

function fstatSync(rid) {
  ops.op_fstat_sync(rid, statBuf);
  return statStruct(statBuf);
}

async function fstat(rid) {
  return parseFileInfo(await core.opAsync("op_fstat_async", rid));
}

async function lstat(path) {
  const res = await core.opAsync("op_lstat_async", pathFromURL(path));
  return parseFileInfo(res);
}

function lstatSync(path) {
  ops.op_lstat_sync(pathFromURL(path), statBuf);
  return statStruct(statBuf);
}

async function stat(path) {
  const res = await core.opAsync("op_stat_async", pathFromURL(path));
  return parseFileInfo(res);
}

function statSync(path) {
  ops.op_stat_sync(pathFromURL(path), statBuf);
  return statStruct(statBuf);
}

function coerceLen(len) {
  if (len == null || len < 0) {
    return 0;
  }
  return len;
}

function ftruncateSync(rid, len) {
  ops.op_ftruncate_sync(rid, coerceLen(len));
}

async function ftruncate(rid, len) {
  await core.opAsync2("op_ftruncate_async", rid, coerceLen(len));
}

function truncateSync(path, len) {
  ops.op_truncate_sync(path, coerceLen(len));
}

async function truncate(path, len) {
  await core.opAsync2("op_truncate_async", path, coerceLen(len));
}

function umask(mask) {
  return ops.op_umask(mask);
}

function linkSync(oldpath, newpath) {
  ops.op_link_sync(oldpath, newpath);
}

async function link(oldpath, newpath) {
  await core.opAsync2("op_link_async", oldpath, newpath);
}

function toUnixTimeFromEpoch(value) {
  if (ObjectPrototypeIsPrototypeOf(DatePrototype, value)) {
    const time = value.valueOf();
    const seconds = MathTrunc(time / 1e3);
    const nanoseconds = MathTrunc(time - (seconds * 1e3)) * 1e6;

    return [
      seconds,
      nanoseconds,
    ];
  }

  const seconds = value;
  const nanoseconds = 0;

  return [
    seconds,
    nanoseconds,
  ];
}

function futimeSync(
  rid,
  atime,
  mtime,
) {
  const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
  const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
  ops.op_futime_sync(rid, atimeSec, atimeNsec, mtimeSec, mtimeNsec);
}

async function futime(
  rid,
  atime,
  mtime,
) {
  const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
  const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
  await core.opAsync(
    "op_futime_async",
    rid,
    atimeSec,
    atimeNsec,
    mtimeSec,
    mtimeNsec,
  );
}

function utimeSync(
  path,
  atime,
  mtime,
) {
  const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
  const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
  ops.op_utime_sync(
    pathFromURL(path),
    atimeSec,
    atimeNsec,
    mtimeSec,
    mtimeNsec,
  );
}

async function utime(
  path,
  atime,
  mtime,
) {
  const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
  const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
  await core.opAsync(
    "op_utime_async",
    pathFromURL(path),
    atimeSec,
    atimeNsec,
    mtimeSec,
    mtimeNsec,
  );
}

function symlinkSync(
  oldpath,
  newpath,
  options,
) {
  ops.op_symlink_sync(
    pathFromURL(oldpath),
    pathFromURL(newpath),
    options?.type,
  );
}

async function symlink(
  oldpath,
  newpath,
  options,
) {
  await core.opAsync(
    "op_symlink_async",
    pathFromURL(oldpath),
    pathFromURL(newpath),
    options?.type,
  );
}

function fdatasyncSync(rid) {
  ops.op_fdatasync_sync(rid);
}

async function fdatasync(rid) {
  await core.opAsync("op_fdatasync_async", rid);
}

function fsyncSync(rid) {
  ops.op_fsync_sync(rid);
}

async function fsync(rid) {
  await core.opAsync("op_fsync_async", rid);
}

function flockSync(rid, exclusive) {
  ops.op_flock_sync(rid, exclusive === true);
}

async function flock(rid, exclusive) {
  await core.opAsync2("op_flock_async", rid, exclusive === true);
}

function funlockSync(rid) {
  ops.op_funlock_sync(rid);
}

async function funlock(rid) {
  await core.opAsync("op_funlock_async", rid);
}

function seekSync(
  rid,
  offset,
  whence,
) {
  return ops.op_seek_sync(rid, offset, whence);
}

function seek(
  rid,
  offset,
  whence,
) {
  return core.opAsync("op_seek_async", rid, offset, whence);
}

function openSync(
  path,
  options,
) {
  if (options) checkOpenOptions(options);
  const rid = ops.op_open_sync(
    pathFromURL(path),
    options,
  );

  return new FsFile(rid);
}

async function open(
  path,
  options,
) {
  if (options) checkOpenOptions(options);
  const rid = await core.opAsync(
    "op_open_async",
    pathFromURL(path),
    options,
  );

  return new FsFile(rid);
}

function createSync(path) {
  return openSync(path, {
    read: true,
    write: true,
    truncate: true,
    create: true,
  });
}

function create(path) {
  return open(path, {
    read: true,
    write: true,
    truncate: true,
    create: true,
  });
}

class FsFile {
  #rid = 0;

  #readable;
  #writable;

  constructor(rid) {
    this.#rid = rid;
  }

  get rid() {
    return this.#rid;
  }

  write(p) {
    return write(this.rid, p);
  }

  writeSync(p) {
    return writeSync(this.rid, p);
  }

  truncate(len) {
    return ftruncate(this.rid, len);
  }

  truncateSync(len) {
    return ftruncateSync(this.rid, len);
  }

  read(p) {
    return read(this.rid, p);
  }

  readSync(p) {
    return readSync(this.rid, p);
  }

  seek(offset, whence) {
    return seek(this.rid, offset, whence);
  }

  seekSync(offset, whence) {
    return seekSync(this.rid, offset, whence);
  }

  stat() {
    return fstat(this.rid);
  }

  statSync() {
    return fstatSync(this.rid);
  }

  close() {
    core.close(this.rid);
  }

  get readable() {
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRid(this.rid);
    }
    return this.#readable;
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.rid);
    }
    return this.#writable;
  }
}

function checkOpenOptions(options) {
  if (
    ArrayPrototypeFilter(
      ObjectValues(options),
      (val) => val === true,
    ).length === 0
  ) {
    throw new Error("OpenOptions requires at least one option to be true");
  }

  if (options.truncate && !options.write) {
    throw new Error("'truncate' option requires 'write' option");
  }

  const createOrCreateNewWithoutWriteOrAppend =
    (options.create || options.createNew) &&
    !(options.write || options.append);

  if (createOrCreateNewWithoutWriteOrAppend) {
    throw new Error(
      "'create' or 'createNew' options require 'write' or 'append' option",
    );
  }
}

const File = FsFile;

function readFileSync(path) {
  return ops.op_read_file_sync(pathFromURL(path));
}

async function readFile(path, options) {
  let cancelRid;
  let abortHandler;
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = ops.op_cancel_handle();
    abortHandler = () => core.tryClose(cancelRid);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    const read = await core.opAsync(
      "op_read_file_async",
      pathFromURL(path),
      cancelRid,
    );
    return read;
  } finally {
    if (options?.signal) {
      options.signal[abortSignal.remove](abortHandler);

      // always throw the abort error when aborted
      options.signal.throwIfAborted();
    }
  }
}

function readTextFileSync(path) {
  return ops.op_read_file_text_sync(pathFromURL(path));
}

async function readTextFile(path, options) {
  let cancelRid;
  let abortHandler;
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = ops.op_cancel_handle();
    abortHandler = () => core.tryClose(cancelRid);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    const read = await core.opAsync(
      "op_read_file_text_async",
      pathFromURL(path),
      cancelRid,
    );
    return read;
  } finally {
    if (options?.signal) {
      options.signal[abortSignal.remove](abortHandler);

      // always throw the abort error when aborted
      options.signal.throwIfAborted();
    }
  }
}

function writeFileSync(
  path,
  data,
  options = {},
) {
  options.signal?.throwIfAborted();
  ops.op_write_file_sync(
    pathFromURL(path),
    options.mode,
    options.append ?? false,
    options.create ?? true,
    options.createNew ?? false,
    data,
  );
}

async function writeFile(
  path,
  data,
  options = {},
) {
  let cancelRid;
  let abortHandler;
  if (options.signal) {
    options.signal.throwIfAborted();
    cancelRid = ops.op_cancel_handle();
    abortHandler = () => core.tryClose(cancelRid);
    options.signal[abortSignal.add](abortHandler);
  }
  try {
    if (ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, data)) {
      const file = await open(path, {
        mode: options.mode,
        append: options.append ?? false,
        create: options.create ?? true,
        createNew: options.createNew ?? false,
        write: true,
      });
      await data.pipeTo(file.writable, {
        signal: options.signal,
      });
    } else {
      await core.opAsync(
        "op_write_file_async",
        pathFromURL(path),
        options.mode,
        options.append ?? false,
        options.create ?? true,
        options.createNew ?? false,
        data,
        cancelRid,
      );
    }
  } finally {
    if (options.signal) {
      options.signal[abortSignal.remove](abortHandler);

      // always throw the abort error when aborted
      options.signal.throwIfAborted();
    }
  }
}

function writeTextFileSync(
  path,
  data,
  options = {},
) {
  const encoder = new TextEncoder();
  return writeFileSync(path, encoder.encode(data), options);
}

function writeTextFile(
  path,
  data,
  options = {},
) {
  if (ObjectPrototypeIsPrototypeOf(ReadableStreamPrototype, data)) {
    return writeFile(
      path,
      data.pipeThrough(new TextEncoderStream()),
      options,
    );
  } else {
    const encoder = new TextEncoder();
    return writeFile(path, encoder.encode(data), options);
  }
}

export {
  chdir,
  chmod,
  chmodSync,
  chown,
  chownSync,
  copyFile,
  copyFileSync,
  create,
  createSync,
  cwd,
  fdatasync,
  fdatasyncSync,
  File,
  flock,
  flockSync,
  FsFile,
  fstat,
  fstatSync,
  fsync,
  fsyncSync,
  ftruncate,
  ftruncateSync,
  funlock,
  funlockSync,
  futime,
  futimeSync,
  link,
  linkSync,
  lstat,
  lstatSync,
  makeTempDir,
  makeTempDirSync,
  makeTempFile,
  makeTempFileSync,
  mkdir,
  mkdirSync,
  open,
  openSync,
  readDir,
  readDirSync,
  readFile,
  readFileSync,
  readLink,
  readLinkSync,
  readTextFile,
  readTextFileSync,
  realPath,
  realPathSync,
  remove,
  removeSync,
  rename,
  renameSync,
  seek,
  seekSync,
  stat,
  statSync,
  symlink,
  symlinkSync,
  truncate,
  truncateSync,
  umask,
  utime,
  utimeSync,
  writeFile,
  writeFileSync,
  writeTextFile,
  writeTextFileSync,
};
