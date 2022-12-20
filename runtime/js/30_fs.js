// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
"use strict";

((window) => {
  const core = window.Deno.core;
  const ops = core.ops;
  const {
    Date,
    DatePrototype,
    MathTrunc,
    ObjectPrototypeIsPrototypeOf,
    SafeArrayIterator,
    SymbolAsyncIterator,
    SymbolIterator,
    Function,
    ObjectEntries,
    Uint32Array,
  } = window.__bootstrap.primordials;
  const { pathFromURL } = window.__bootstrap.util;
  const build = window.__bootstrap.build.build;

  function chmodSync(path, mode) {
    ops.op_chmod_sync(pathFromURL(path), mode);
  }

  async function chmod(path, mode) {
    await core.opAsync("op_chmod_async", pathFromURL(path), mode);
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
    return ops.op_make_temp_dir_sync(options);
  }

  function makeTempDir(options = {}) {
    return core.opAsync("op_make_temp_dir_async", options);
  }

  function makeTempFileSync(options = {}) {
    return ops.op_make_temp_file_sync(options);
  }

  function makeTempFile(options = {}) {
    return core.opAsync("op_make_temp_file_async", options);
  }

  function mkdirArgs(path, options) {
    const args = { path: pathFromURL(path), recursive: false };
    if (options != null) {
      if (typeof options.recursive == "boolean") {
        args.recursive = options.recursive;
      }
      if (options.mode) {
        args.mode = options.mode;
      }
    }
    return args;
  }

  function mkdirSync(path, options) {
    ops.op_mkdir_sync(mkdirArgs(path, options));
  }

  async function mkdir(
    path,
    options,
  ) {
    await core.opAsync("op_mkdir_async", mkdirArgs(path, options));
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
        yield* await array;
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
    // `?` prefix means optional on windows.
    let offset = 0;
    let str =
      'const unix = Deno.build.os === "darwin" || Deno.build.os === "linux"; return {';
    for (let [name, type] of new SafeArrayIterator(ObjectEntries(types))) {
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

  const [statStruct, statBuf] = createByteStruct({
    isFile: "bool",
    isDirectory: "bool",
    isSymlink: "bool",
    size: "u64",
    mtime: "date",
    atime: "date",
    birthtime: "date",
    dev: "?u64",
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
    const unix = build.os === "darwin" || build.os === "linux";
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
      // Only non-null if on Unix
      dev: unix ? response.dev : null,
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
    const res = await core.opAsync("op_stat_async", {
      path: pathFromURL(path),
      lstat: true,
    });
    return parseFileInfo(res);
  }

  function lstatSync(path) {
    ops.op_stat_sync(
      pathFromURL(path),
      true,
      statBuf,
    );
    return statStruct(statBuf);
  }

  async function stat(path) {
    const res = await core.opAsync("op_stat_async", {
      path: pathFromURL(path),
      lstat: false,
    });
    return parseFileInfo(res);
  }

  function statSync(path) {
    ops.op_stat_sync(
      pathFromURL(path),
      false,
      statBuf,
    );
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
    await core.opAsync("op_ftruncate_async", rid, coerceLen(len));
  }

  function truncateSync(path, len) {
    ops.op_truncate_sync(path, coerceLen(len));
  }

  async function truncate(path, len) {
    await core.opAsync("op_truncate_async", path, coerceLen(len));
  }

  function umask(mask) {
    return ops.op_umask(mask);
  }

  function linkSync(oldpath, newpath) {
    ops.op_link_sync(oldpath, newpath);
  }

  async function link(oldpath, newpath) {
    await core.opAsync("op_link_async", oldpath, newpath);
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
    const [atimeSec, atimeNsec] = toUnixTimeFromEpoch(atime);
    const [mtimeSec, mtimeNsec] = toUnixTimeFromEpoch(mtime);
    ops.op_futime_sync(rid, atimeSec, atimeNsec, mtimeSec, mtimeNsec);
  }

  async function futime(
    rid,
    atime,
    mtime,
  ) {
    const [atimeSec, atimeNsec] = toUnixTimeFromEpoch(atime);
    const [mtimeSec, mtimeNsec] = toUnixTimeFromEpoch(mtime);
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
    const [atimeSec, atimeNsec] = toUnixTimeFromEpoch(atime);
    const [mtimeSec, mtimeNsec] = toUnixTimeFromEpoch(mtime);
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
    const [atimeSec, atimeNsec] = toUnixTimeFromEpoch(atime);
    const [mtimeSec, mtimeNsec] = toUnixTimeFromEpoch(mtime);
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
    await core.opAsync("op_flock_async", rid, exclusive === true);
  }

  function funlockSync(rid) {
    ops.op_funlock_sync(rid);
  }

  async function funlock(rid) {
    await core.opAsync("op_funlock_async", rid);
  }

  window.__bootstrap.fs = {
    cwd,
    chdir,
    chmodSync,
    chmod,
    chown,
    chownSync,
    copyFile,
    copyFileSync,
    makeTempFile,
    makeTempDir,
    makeTempFileSync,
    makeTempDirSync,
    mkdir,
    mkdirSync,
    readDir,
    readDirSync,
    readLinkSync,
    readLink,
    realPathSync,
    realPath,
    remove,
    removeSync,
    renameSync,
    rename,
    lstat,
    lstatSync,
    stat,
    statSync,
    ftruncate,
    ftruncateSync,
    truncate,
    truncateSync,
    umask,
    link,
    linkSync,
    fstatSync,
    fstat,
    futime,
    futimeSync,
    utime,
    utimeSync,
    symlink,
    symlinkSync,
    fdatasync,
    fdatasyncSync,
    fsync,
    fsyncSync,
    flock,
    flockSync,
    funlock,
    funlockSync,
  };
})(this);
