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
    SymbolAsyncIterator,
    SymbolIterator,
  } = window.__bootstrap.primordials;
  const { pathFromURL } = window.__bootstrap.util;
  const build = window.__bootstrap.build.build;

  function chmodSync(path, mode) {
    ops.op_chmod_sync({ path: pathFromURL(path), mode });
  }

  async function chmod(path, mode) {
    await core.opAsync("op_chmod_async", { path: pathFromURL(path), mode });
  }

  function chownSync(
    path,
    uid,
    gid,
  ) {
    ops.op_chown_sync({ path: pathFromURL(path), uid, gid });
  }

  async function chown(
    path,
    uid,
    gid,
  ) {
    await core.opAsync(
      "op_chown_async",
      { path: pathFromURL(path), uid, gid },
    );
  }

  function copyFileSync(
    fromPath,
    toPath,
  ) {
    ops.op_copy_file_sync({
      from: pathFromURL(fromPath),
      to: pathFromURL(toPath),
    });
  }

  async function copyFile(
    fromPath,
    toPath,
  ) {
    await core.opAsync("op_copy_file_async", {
      from: pathFromURL(fromPath),
      to: pathFromURL(toPath),
    });
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
    ops.op_remove_sync({
      path: pathFromURL(path),
      recursive: !!options.recursive,
    });
  }

  async function remove(
    path,
    options = {},
  ) {
    await core.opAsync("op_remove_async", {
      path: pathFromURL(path),
      recursive: !!options.recursive,
    });
  }

  function renameSync(oldpath, newpath) {
    ops.op_rename_sync({
      oldpath: pathFromURL(oldpath),
      newpath: pathFromURL(newpath),
    });
  }

  async function rename(oldpath, newpath) {
    await core.opAsync("op_rename_async", {
      oldpath: pathFromURL(oldpath),
      newpath: pathFromURL(newpath),
    });
  }

  function parseFileInfo(response) {
    const unix = build.os === "darwin" || build.os === "linux";
    return {
      isFile: response.isFile,
      isDirectory: response.isDirectory,
      isSymlink: response.isSymlink,
      size: response.size,
      mtime: response.mtime != null ? new Date(response.mtime) : null,
      atime: response.atime != null ? new Date(response.atime) : null,
      birthtime: response.birthtime != null
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
    return parseFileInfo(ops.op_fstat_sync(rid));
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
    const res = ops.op_stat_sync({
      path: pathFromURL(path),
      lstat: true,
    });
    return parseFileInfo(res);
  }

  async function stat(path) {
    const res = await core.opAsync("op_stat_async", {
      path: pathFromURL(path),
      lstat: false,
    });
    return parseFileInfo(res);
  }

  function statSync(path) {
    const res = ops.op_stat_sync({
      path: pathFromURL(path),
      lstat: false,
    });
    return parseFileInfo(res);
  }

  function coerceLen(len) {
    if (len == null || len < 0) {
      return 0;
    }

    return len;
  }

  function ftruncateSync(rid, len) {
    ops.op_ftruncate_sync({ rid, len: coerceLen(len) });
  }

  async function ftruncate(rid, len) {
    await core.opAsync("op_ftruncate_async", { rid, len: coerceLen(len) });
  }

  function truncateSync(path, len) {
    ops.op_truncate_sync({ path, len: coerceLen(len) });
  }

  async function truncate(path, len) {
    await core.opAsync("op_truncate_async", { path, len: coerceLen(len) });
  }

  function umask(mask) {
    return ops.op_umask(mask);
  }

  function linkSync(oldpath, newpath) {
    ops.op_link_sync({ oldpath, newpath });
  }

  async function link(oldpath, newpath) {
    await core.opAsync("op_link_async", { oldpath, newpath });
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
    ops.op_futime_sync({
      rid,
      atime: toUnixTimeFromEpoch(atime),
      mtime: toUnixTimeFromEpoch(mtime),
    });
  }

  async function futime(
    rid,
    atime,
    mtime,
  ) {
    await core.opAsync("op_futime_async", {
      rid,
      atime: toUnixTimeFromEpoch(atime),
      mtime: toUnixTimeFromEpoch(mtime),
    });
  }

  function utimeSync(
    path,
    atime,
    mtime,
  ) {
    ops.op_utime_sync({
      path: pathFromURL(path),
      atime: toUnixTimeFromEpoch(atime),
      mtime: toUnixTimeFromEpoch(mtime),
    });
  }

  async function utime(
    path,
    atime,
    mtime,
  ) {
    await core.opAsync("op_utime_async", {
      path: pathFromURL(path),
      atime: toUnixTimeFromEpoch(atime),
      mtime: toUnixTimeFromEpoch(mtime),
    });
  }

  function symlinkSync(
    oldpath,
    newpath,
    options,
  ) {
    ops.op_symlink_sync({
      oldpath: pathFromURL(oldpath),
      newpath: pathFromURL(newpath),
      options,
    });
  }

  async function symlink(
    oldpath,
    newpath,
    options,
  ) {
    await core.opAsync("op_symlink_async", {
      oldpath: pathFromURL(oldpath),
      newpath: pathFromURL(newpath),
      options,
    });
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
