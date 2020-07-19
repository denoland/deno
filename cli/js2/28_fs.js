// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const { sendSync, sendAsync } = window.__bootstrap.dispatchJson;
  const { pathFromURL } = window.__bootstrap.util;
  const build = window.__bootstrap.build.build;

  function chmodSync(path, mode) {
    sendSync("op_chmod", { path: pathFromURL(path), mode });
  }

  async function chmod(path, mode) {
    await sendAsync("op_chmod", { path: pathFromURL(path), mode });
  }

  function chownSync(
    path,
    uid,
    gid,
  ) {
    sendSync("op_chown", { path: pathFromURL(path), uid, gid });
  }

  async function chown(
    path,
    uid,
    gid,
  ) {
    await sendAsync("op_chown", { path: pathFromURL(path), uid, gid });
  }

  function copyFileSync(
    fromPath,
    toPath,
  ) {
    sendSync("op_copy_file", {
      from: pathFromURL(fromPath),
      to: pathFromURL(toPath),
    });
  }

  async function copyFile(
    fromPath,
    toPath,
  ) {
    await sendAsync("op_copy_file", {
      from: pathFromURL(fromPath),
      to: pathFromURL(toPath),
    });
  }

  function cwd() {
    return sendSync("op_cwd");
  }

  function chdir(directory) {
    sendSync("op_chdir", { directory });
  }

  function makeTempDirSync(options = {}) {
    return sendSync("op_make_temp_dir", options);
  }

  function makeTempDir(options = {}) {
    return sendAsync("op_make_temp_dir", options);
  }

  function makeTempFileSync(options = {}) {
    return sendSync("op_make_temp_file", options);
  }

  function makeTempFile(options = {}) {
    return sendAsync("op_make_temp_file", options);
  }

  function mkdirArgs(path, options) {
    const args = { path, recursive: false };
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
    sendSync("op_mkdir", mkdirArgs(path, options));
  }

  async function mkdir(
    path,
    options,
  ) {
    await sendAsync("op_mkdir", mkdirArgs(path, options));
  }

  function res(response) {
    return response.entries;
  }

  function readDirSync(path) {
    return res(sendSync("op_read_dir", { path: pathFromURL(path) }))[
      Symbol.iterator
    ]();
  }

  function readDir(path) {
    const array = sendAsync("op_read_dir", { path: pathFromURL(path) }).then(
      res,
    );
    return {
      async *[Symbol.asyncIterator]() {
        yield* await array;
      },
    };
  }

  function readLinkSync(path) {
    return sendSync("op_read_link", { path });
  }

  function readLink(path) {
    return sendAsync("op_read_link", { path });
  }

  function realPathSync(path) {
    return sendSync("op_realpath", { path });
  }

  function realPath(path) {
    return sendAsync("op_realpath", { path });
  }

  function removeSync(
    path,
    options = {},
  ) {
    sendSync("op_remove", {
      path: pathFromURL(path),
      recursive: !!options.recursive,
    });
  }

  async function remove(
    path,
    options = {},
  ) {
    await sendAsync("op_remove", {
      path: pathFromURL(path),
      recursive: !!options.recursive,
    });
  }

  function renameSync(oldpath, newpath) {
    sendSync("op_rename", { oldpath, newpath });
  }

  async function rename(oldpath, newpath) {
    await sendAsync("op_rename", { oldpath, newpath });
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
    return parseFileInfo(sendSync("op_fstat", { rid }));
  }

  async function fstat(rid) {
    return parseFileInfo(await sendAsync("op_fstat", { rid }));
  }

  async function lstat(path) {
    const res = await sendAsync("op_stat", {
      path: pathFromURL(path),
      lstat: true,
    });
    return parseFileInfo(res);
  }

  function lstatSync(path) {
    const res = sendSync("op_stat", {
      path: pathFromURL(path),
      lstat: true,
    });
    return parseFileInfo(res);
  }

  async function stat(path) {
    const res = await sendAsync("op_stat", {
      path: pathFromURL(path),
      lstat: false,
    });
    return parseFileInfo(res);
  }

  function statSync(path) {
    const res = sendSync("op_stat", {
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
    sendSync("op_ftruncate", { rid, len: coerceLen(len) });
  }

  async function ftruncate(rid, len) {
    await sendAsync("op_ftruncate", { rid, len: coerceLen(len) });
  }

  function truncateSync(path, len) {
    sendSync("op_truncate", { path, len: coerceLen(len) });
  }

  async function truncate(path, len) {
    await sendAsync("op_truncate", { path, len: coerceLen(len) });
  }

  function umask(mask) {
    return sendSync("op_umask", { mask });
  }

  function linkSync(oldpath, newpath) {
    sendSync("op_link", { oldpath, newpath });
  }

  async function link(oldpath, newpath) {
    await sendAsync("op_link", { oldpath, newpath });
  }

  function toSecondsFromEpoch(v) {
    return v instanceof Date ? Math.trunc(v.valueOf() / 1000) : v;
  }

  function utimeSync(
    path,
    atime,
    mtime,
  ) {
    sendSync("op_utime", {
      path,
      // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
      atime: toSecondsFromEpoch(atime),
      mtime: toSecondsFromEpoch(mtime),
    });
  }

  async function utime(
    path,
    atime,
    mtime,
  ) {
    await sendAsync("op_utime", {
      path,
      // TODO(ry) split atime, mtime into [seconds, nanoseconds] tuple
      atime: toSecondsFromEpoch(atime),
      mtime: toSecondsFromEpoch(mtime),
    });
  }

  function symlinkSync(
    oldpath,
    newpath,
    options,
  ) {
    sendSync("op_symlink", { oldpath, newpath, options });
  }

  async function symlink(
    oldpath,
    newpath,
    options,
  ) {
    await sendAsync("op_symlink", { oldpath, newpath, options });
  }

  function fdatasyncSync(rid) {
    sendSync("op_fdatasync", { rid });
  }

  async function fdatasync(rid) {
    await sendAsync("op_fdatasync", { rid });
  }

  function fsyncSync(rid) {
    sendSync("op_fsync", { rid });
  }

  async function fsync(rid) {
    await sendAsync("op_fsync", { rid });
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
    fstatSync,
    fstat,
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
    utime,
    utimeSync,
    symlink,
    symlinkSync,
    fdatasync,
    fdatasyncSync,
    fsync,
    fsyncSync,
  };
})(this);
