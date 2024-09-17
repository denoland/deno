// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { core, primordials } from "ext:core/mod.js";
const {
  isDate,
  internalRidSymbol,
  createCancelHandle,
} = core;
import {
  op_fs_chdir,
  op_fs_chmod_async,
  op_fs_chmod_sync,
  op_fs_chown_async,
  op_fs_chown_sync,
  op_fs_copy_file_async,
  op_fs_copy_file_sync,
  op_fs_cwd,
  op_fs_file_stat_async,
  op_fs_file_stat_sync,
  op_fs_file_sync_async,
  op_fs_file_sync_data_async,
  op_fs_file_sync_data_sync,
  op_fs_file_sync_sync,
  op_fs_file_truncate_async,
  op_fs_flock_async,
  op_fs_flock_sync,
  op_fs_ftruncate_sync,
  op_fs_funlock_async,
  op_fs_funlock_sync,
  op_fs_futime_async,
  op_fs_futime_sync,
  op_fs_link_async,
  op_fs_link_sync,
  op_fs_lstat_async,
  op_fs_lstat_sync,
  op_fs_make_temp_dir_async,
  op_fs_make_temp_dir_sync,
  op_fs_make_temp_file_async,
  op_fs_make_temp_file_sync,
  op_fs_mkdir_async,
  op_fs_mkdir_sync,
  op_fs_open_async,
  op_fs_open_sync,
  op_fs_read_dir_async,
  op_fs_read_dir_sync,
  op_fs_read_file_async,
  op_fs_read_file_sync,
  op_fs_read_file_text_async,
  op_fs_read_file_text_sync,
  op_fs_read_link_async,
  op_fs_read_link_sync,
  op_fs_realpath_async,
  op_fs_realpath_sync,
  op_fs_remove_async,
  op_fs_remove_sync,
  op_fs_rename_async,
  op_fs_rename_sync,
  op_fs_seek_async,
  op_fs_seek_sync,
  op_fs_stat_async,
  op_fs_stat_sync,
  op_fs_symlink_async,
  op_fs_symlink_sync,
  op_fs_truncate_async,
  op_fs_truncate_sync,
  op_fs_umask,
  op_fs_utime_async,
  op_fs_utime_sync,
  op_fs_write_file_async,
  op_fs_write_file_sync,
  op_set_raw,
} from "ext:core/ops";
const {
  ArrayPrototypeFilter,
  Date,
  DatePrototypeGetTime,
  Error,
  Function,
  MathTrunc,
  ObjectEntries,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  ObjectValues,
  StringPrototypeSlice,
  StringPrototypeStartsWith,
  SymbolAsyncIterator,
  SymbolIterator,
  SymbolFor,
  TypeError,
  Uint32Array,
} = primordials;

import { read, readSync, write, writeSync } from "ext:deno_io/12_io.js";
import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import {
  readableStreamForRid,
  ReadableStreamPrototype,
  writableStreamForRid,
} from "ext:deno_web/06_streams.js";
import { pathFromURL, SymbolDispose } from "ext:deno_web/00_infra.js";

function chmodSync(path, mode) {
  op_fs_chmod_sync(pathFromURL(path), mode);
}

async function chmod(path, mode) {
  await op_fs_chmod_async(pathFromURL(path), mode);
}

function chownSync(
  path,
  uid,
  gid,
) {
  op_fs_chown_sync(pathFromURL(path), uid, gid);
}

async function chown(
  path,
  uid,
  gid,
) {
  await op_fs_chown_async(
    pathFromURL(path),
    uid,
    gid,
  );
}

function copyFileSync(
  fromPath,
  toPath,
) {
  op_fs_copy_file_sync(
    pathFromURL(fromPath),
    pathFromURL(toPath),
  );
}

async function copyFile(
  fromPath,
  toPath,
) {
  await op_fs_copy_file_async(
    pathFromURL(fromPath),
    pathFromURL(toPath),
  );
}

function cwd() {
  return op_fs_cwd();
}

function chdir(directory) {
  op_fs_chdir(pathFromURL(directory));
}

function makeTempDirSync(options = { __proto__: null }) {
  return op_fs_make_temp_dir_sync(
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function makeTempDir(options = { __proto__: null }) {
  return op_fs_make_temp_dir_async(
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function makeTempFileSync(options = { __proto__: null }) {
  return op_fs_make_temp_file_sync(
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function makeTempFile(options = { __proto__: null }) {
  return op_fs_make_temp_file_async(
    options.dir,
    options.prefix,
    options.suffix,
  );
}

function mkdirSync(path, options) {
  op_fs_mkdir_sync(
    pathFromURL(path),
    options?.recursive ?? false,
    options?.mode,
  );
}

async function mkdir(path, options) {
  await op_fs_mkdir_async(
    pathFromURL(path),
    options?.recursive ?? false,
    options?.mode,
  );
}

function readDirSync(path) {
  return op_fs_read_dir_sync(pathFromURL(path))[
    SymbolIterator
  ]();
}

function readDir(path) {
  const array = op_fs_read_dir_async(
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
  return op_fs_read_link_sync(pathFromURL(path));
}

function readLink(path) {
  return op_fs_read_link_async(pathFromURL(path));
}

function realPathSync(path) {
  return op_fs_realpath_sync(pathFromURL(path));
}

function realPath(path) {
  return op_fs_realpath_async(pathFromURL(path));
}

function removeSync(
  path,
  options = { __proto__: null },
) {
  op_fs_remove_sync(
    pathFromURL(path),
    !!options.recursive,
  );
}

async function remove(
  path,
  options = { __proto__: null },
) {
  await op_fs_remove_async(
    pathFromURL(path),
    !!options.recursive,
  );
}

function renameSync(oldpath, newpath) {
  op_fs_rename_sync(
    pathFromURL(oldpath),
    pathFromURL(newpath),
  );
}

async function rename(oldpath, newpath) {
  await op_fs_rename_async(
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
//    ?bool converts a false bool value to JS null on Windows.
function createByteStruct(types) {
  // types can be "date", "bool" or "u64".
  let offset = 0;
  let str =
    'const unix = Deno.build.os === "darwin" || Deno.build.os === "linux" || Deno.build.os === "android" || Deno.build.os === "openbsd" || Deno.build.os === "freebsd"; return {';
  const typeEntries = ObjectEntries(types);
  for (let i = 0; i < typeEntries.length; ++i) {
    let { 0: name, 1: type } = typeEntries[i];

    const optional = StringPrototypeStartsWith(type, "?");
    if (optional) type = StringPrototypeSlice(type, 1);

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
      if (!optional) {
        str += `${name}: !!(view[${offset}] + view[${offset + 1}] * 2**32),`;
      } else {
        str += `${name}: (unix ? !!((view[${offset}] + view[${
          offset + 1
        }] * 2**32)) : !!((view[${offset}] + view[${
          offset + 1
        }] * 2**32)) || null),`;
      }
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
  isBlockDevice: "?bool",
  isCharDevice: "?bool",
  isFifo: "?bool",
  isSocket: "?bool",
});

function parseFileInfo(response) {
  const unix = core.build.os === "darwin" ||
    core.build.os === "linux" ||
    core.build.os === "android" ||
    core.build.os === "freebsd" ||
    core.build.os === "openbsd";
  return {
    isFile: response.isFile,
    isDirectory: response.isDirectory,
    isSymlink: response.isSymlink,
    size: response.size,
    mtime: response.mtimeSet === true ? new Date(response.mtime) : null,
    atime: response.atimeSet === true ? new Date(response.atime) : null,
    birthtime: response.birthtimeSet === true
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
    isBlockDevice: unix ? response.isBlockDevice : null,
    isCharDevice: unix ? response.isCharDevice : null,
    isFifo: unix ? response.isFifo : null,
    isSocket: unix ? response.isSocket : null,
  };
}

async function lstat(path) {
  const res = await op_fs_lstat_async(pathFromURL(path));
  return parseFileInfo(res);
}

function lstatSync(path) {
  op_fs_lstat_sync(pathFromURL(path), statBuf);
  return statStruct(statBuf);
}

async function stat(path) {
  const res = await op_fs_stat_async(pathFromURL(path));
  return parseFileInfo(res);
}

function statSync(path) {
  op_fs_stat_sync(pathFromURL(path), statBuf);
  return statStruct(statBuf);
}

function coerceLen(len) {
  if (len == null || len < 0) {
    return 0;
  }
  return len;
}

function truncateSync(path, len) {
  op_fs_truncate_sync(path, coerceLen(len));
}

async function truncate(path, len) {
  await op_fs_truncate_async(path, coerceLen(len));
}

function umask(mask) {
  return op_fs_umask(mask);
}

function linkSync(oldpath, newpath) {
  op_fs_link_sync(oldpath, newpath);
}

async function link(oldpath, newpath) {
  await op_fs_link_async(oldpath, newpath);
}

function toUnixTimeFromEpoch(value) {
  if (isDate(value)) {
    const time = DatePrototypeGetTime(value);
    const seconds = MathTrunc(time / 1e3);
    const nanoseconds = MathTrunc(time - (seconds * 1e3)) * 1e6;

    return [
      seconds,
      nanoseconds,
    ];
  }

  const seconds = MathTrunc(value);
  const nanoseconds = MathTrunc((value * 1e3) - (seconds * 1e3)) * 1e6;

  return [
    seconds,
    nanoseconds,
  ];
}

function utimeSync(
  path,
  atime,
  mtime,
) {
  const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
  const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
  op_fs_utime_sync(
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
  await op_fs_utime_async(
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
  op_fs_symlink_sync(
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
  await op_fs_symlink_async(
    pathFromURL(oldpath),
    pathFromURL(newpath),
    options?.type,
  );
}

function openSync(
  path,
  options,
) {
  if (options) checkOpenOptions(options);
  const rid = op_fs_open_sync(
    pathFromURL(path),
    options,
  );

  return new FsFile(rid, SymbolFor("Deno.internal.FsFile"));
}

async function open(
  path,
  options,
) {
  if (options) checkOpenOptions(options);
  const rid = await op_fs_open_async(
    pathFromURL(path),
    options,
  );

  return new FsFile(rid, SymbolFor("Deno.internal.FsFile"));
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

  constructor(rid, symbol) {
    ObjectDefineProperty(this, internalRidSymbol, {
      __proto__: null,
      enumerable: false,
      value: rid,
    });
    this.#rid = rid;
    if (!symbol || symbol !== SymbolFor("Deno.internal.FsFile")) {
      throw new TypeError(
        "`Deno.FsFile` cannot be constructed, use `Deno.open()` or `Deno.openSync()` instead.",
      );
    }
  }

  write(p) {
    return write(this.#rid, p);
  }

  writeSync(p) {
    return writeSync(this.#rid, p);
  }

  truncate(len) {
    return op_fs_file_truncate_async(this.#rid, coerceLen(len));
  }

  truncateSync(len) {
    return op_fs_ftruncate_sync(this.#rid, coerceLen(len));
  }

  read(p) {
    return read(this.#rid, p);
  }

  readSync(p) {
    return readSync(this.#rid, p);
  }

  seek(offset, whence) {
    return op_fs_seek_async(this.#rid, offset, whence);
  }

  seekSync(offset, whence) {
    return op_fs_seek_sync(this.#rid, offset, whence);
  }

  async stat() {
    return parseFileInfo(await op_fs_file_stat_async(this.#rid));
  }

  statSync() {
    op_fs_file_stat_sync(this.#rid, statBuf);
    return statStruct(statBuf);
  }

  async syncData() {
    await op_fs_file_sync_data_async(this.#rid);
  }

  syncDataSync() {
    op_fs_file_sync_data_sync(this.#rid);
  }

  close() {
    core.close(this.#rid);
  }

  get readable() {
    if (this.#readable === undefined) {
      this.#readable = readableStreamForRid(this.#rid);
    }
    return this.#readable;
  }

  get writable() {
    if (this.#writable === undefined) {
      this.#writable = writableStreamForRid(this.#rid);
    }
    return this.#writable;
  }

  async sync() {
    await op_fs_file_sync_async(this.#rid);
  }

  syncSync() {
    op_fs_file_sync_sync(this.#rid);
  }

  async utime(atime, mtime) {
    const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
    const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
    await op_fs_futime_async(
      this.#rid,
      atimeSec,
      atimeNsec,
      mtimeSec,
      mtimeNsec,
    );
  }

  utimeSync(atime, mtime) {
    const { 0: atimeSec, 1: atimeNsec } = toUnixTimeFromEpoch(atime);
    const { 0: mtimeSec, 1: mtimeNsec } = toUnixTimeFromEpoch(mtime);
    op_fs_futime_sync(this.#rid, atimeSec, atimeNsec, mtimeSec, mtimeNsec);
  }

  isTerminal() {
    return core.isTerminal(this.#rid);
  }

  setRaw(mode, options = { __proto__: null }) {
    const cbreak = !!(options.cbreak ?? false);
    op_set_raw(this.#rid, mode, cbreak);
  }

  lockSync(exclusive = false) {
    op_fs_flock_sync(this.#rid, exclusive);
  }

  async lock(exclusive = false) {
    await op_fs_flock_async(this.#rid, exclusive);
  }

  unlockSync() {
    op_fs_funlock_sync(this.#rid);
  }

  async unlock() {
    await op_fs_funlock_async(this.#rid);
  }

  [SymbolDispose]() {
    core.tryClose(this.#rid);
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

function readFileSync(path) {
  return op_fs_read_file_sync(pathFromURL(path));
}

async function readFile(path, options) {
  let cancelRid;
  let abortHandler;
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = createCancelHandle();
    abortHandler = () => core.tryClose(cancelRid);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    const read = await op_fs_read_file_async(
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
  return op_fs_read_file_text_sync(pathFromURL(path));
}

async function readTextFile(path, options) {
  let cancelRid;
  let abortHandler;
  if (options?.signal) {
    options.signal.throwIfAborted();
    cancelRid = createCancelHandle();
    abortHandler = () => core.tryClose(cancelRid);
    options.signal[abortSignal.add](abortHandler);
  }

  try {
    const read = await op_fs_read_file_text_async(
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
  options = { __proto__: null },
) {
  options.signal?.throwIfAborted();
  op_fs_write_file_sync(
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
  options = { __proto__: null },
) {
  let cancelRid;
  let abortHandler;
  if (options.signal) {
    options.signal.throwIfAborted();
    cancelRid = createCancelHandle();
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
        truncate: !(options.append ?? false),
        write: true,
      });
      await data.pipeTo(file.writable, {
        signal: options.signal,
      });
    } else {
      await op_fs_write_file_async(
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
  options = { __proto__: null },
) {
  const encoder = new TextEncoder();
  return writeFileSync(path, encoder.encode(data), options);
}

function writeTextFile(
  path,
  data,
  options = { __proto__: null },
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
  FsFile,
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
