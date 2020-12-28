// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const { pathFromURL } = window.__bootstrap.util;
  const build = window.__bootstrap.build.build;

  function chmodSync(path, mode) {
    core.jsonOpSync("op_chmod_sync", { path: pathFromURL(path), mode });
  }

  async function chmod(path, mode) {
    await core.jsonOpAsync("op_chmod_async", { path: pathFromURL(path), mode });
  }

  function chownSync(
    path,
    uid,
    gid,
  ) {
    core.jsonOpSync("op_chown_sync", { path: pathFromURL(path), uid, gid });
  }

  async function chown(
    path,
    uid,
    gid,
  ) {
    await core.jsonOpAsync(
      "op_chown_async",
      { path: pathFromURL(path), uid, gid },
    );
  }

  function copyFileSync(
    fromPath,
    toPath,
  ) {
    core.jsonOpSync("op_copy_file_sync", {
      from: pathFromURL(fromPath),
      to: pathFromURL(toPath),
    });
  }

  async function copyFile(
    fromPath,
    toPath,
  ) {
    await core.jsonOpAsync("op_copy_file_async", {
      from: pathFromURL(fromPath),
      to: pathFromURL(toPath),
    });
  }

  function cwd() {
    return core.jsonOpSync("op_cwd");
  }

  function chdir(directory) {
    core.jsonOpSync("op_chdir", { directory });
  }

  function makeTempDirSync(options = {}) {
    return core.jsonOpSync("op_make_temp_dir_sync", options);
  }

  function makeTempDir(options = {}) {
    return core.jsonOpAsync("op_make_temp_dir_async", options);
  }

  function makeTempFileSync(options = {}) {
    return core.jsonOpSync("op_make_temp_file_sync", options);
  }

  function makeTempFile(options = {}) {
    return core.jsonOpAsync("op_make_temp_file_async", options);
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
    core.jsonOpSync("op_mkdir_sync", mkdirArgs(path, options));
  }

  async function mkdir(
    path,
    options,
  ) {
    await core.jsonOpAsync("op_mkdir_async", mkdirArgs(path, options));
  }

  function res(response) {
    return response.entries;
  }

  function readDirSync(path) {
    return res(
      core.jsonOpSync("op_read_dir_sync", { path: pathFromURL(path) }),
    )[
      Symbol.iterator
    ]();
  }

  function readDir(path) {
    const array = core.jsonOpAsync(
      "op_read_dir_async",
      { path: pathFromURL(path) },
    )
      .then(
        res,
      );
    return {
      async *[Symbol.asyncIterator]() {
        yield* await array;
      },
    };
  }

  function readLinkSync(path) {
    return core.jsonOpSync("op_read_link_sync", { path });
  }

  function readLink(path) {
    return core.jsonOpAsync("op_read_link_async", { path });
  }

  function realPathSync(path) {
    return core.jsonOpSync("op_realpath_sync", { path });
  }

  function realPath(path) {
    return core.jsonOpAsync("op_realpath_async", { path });
  }

  function removeSync(
    path,
    options = {},
  ) {
    core.jsonOpSync("op_remove_sync", {
      path: pathFromURL(path),
      recursive: !!options.recursive,
    });
  }

  async function remove(
    path,
    options = {},
  ) {
    await core.jsonOpAsync("op_remove_async", {
      path: pathFromURL(path),
      recursive: !!options.recursive,
    });
  }

  function renameSync(oldpath, newpath) {
    core.jsonOpSync("op_rename_sync", { oldpath, newpath });
  }

  async function rename(oldpath, newpath) {
    await core.jsonOpAsync("op_rename_async", { oldpath, newpath });
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
    return parseFileInfo(core.jsonOpSync("op_fstat_sync", { rid }));
  }

  async function fstat(rid) {
    return parseFileInfo(await core.jsonOpAsync("op_fstat_async", { rid }));
  }

  async function lstat(path) {
    const res = await core.jsonOpAsync("op_stat_async", {
      path: pathFromURL(path),
      lstat: true,
    });
    return parseFileInfo(res);
  }

  function lstatSync(path) {
    const res = core.jsonOpSync("op_stat_sync", {
      path: pathFromURL(path),
      lstat: true,
    });
    return parseFileInfo(res);
  }

  async function stat(path) {
    const res = await core.jsonOpAsync("op_stat_async", {
      path: pathFromURL(path),
      lstat: false,
    });
    return parseFileInfo(res);
  }

  function statSync(path) {
    const res = core.jsonOpSync("op_stat_sync", {
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
    core.jsonOpSync("op_ftruncate_sync", { rid, len: coerceLen(len) });
  }

  async function ftruncate(rid, len) {
    await core.jsonOpAsync("op_ftruncate_async", { rid, len: coerceLen(len) });
  }

  function truncateSync(path, len) {
    core.jsonOpSync("op_truncate_sync", { path, len: coerceLen(len) });
  }

  async function truncate(path, len) {
    await core.jsonOpAsync("op_truncate_async", { path, len: coerceLen(len) });
  }

  function umask(mask) {
    return core.jsonOpSync("op_umask", { mask });
  }

  function linkSync(oldpath, newpath) {
    core.jsonOpSync("op_link_sync", { oldpath, newpath });
  }

  async function link(oldpath, newpath) {
    await core.jsonOpAsync("op_link_async", { oldpath, newpath });
  }

  function toUnixTimeFromEpoch(value) {
    if (value instanceof Date) {
      const time = value.valueOf();
      const seconds = Math.trunc(time / 1e3);
      const nanoseconds = Math.trunc(time - (seconds * 1e3)) * 1e6;

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
    core.jsonOpSync("op_futime_sync", {
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
    await core.jsonOpAsync("op_futime_async", {
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
    core.jsonOpSync("op_utime_sync", {
      path,
      atime: toUnixTimeFromEpoch(atime),
      mtime: toUnixTimeFromEpoch(mtime),
    });
  }

  async function utime(
    path,
    atime,
    mtime,
  ) {
    await core.jsonOpAsync("op_utime_async", {
      path,
      atime: toUnixTimeFromEpoch(atime),
      mtime: toUnixTimeFromEpoch(mtime),
    });
  }

  function symlinkSync(
    oldpath,
    newpath,
    options,
  ) {
    core.jsonOpSync("op_symlink_sync", { oldpath, newpath, options });
  }

  async function symlink(
    oldpath,
    newpath,
    options,
  ) {
    await core.jsonOpAsync("op_symlink_async", { oldpath, newpath, options });
  }

  function fdatasyncSync(rid) {
    core.jsonOpSync("op_fdatasync_sync", { rid });
  }

  async function fdatasync(rid) {
    await core.jsonOpAsync("op_fdatasync_async", { rid });
  }

  function fsyncSync(rid) {
    core.jsonOpSync("op_fsync_sync", { rid });
  }

  async function fsync(rid) {
    await core.jsonOpAsync("op_fsync_async", { rid });
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
  };
})(this);
