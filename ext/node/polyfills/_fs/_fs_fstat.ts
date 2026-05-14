// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

(function () {
const { core } = globalThis.__bootstrap;
const { op_node_fs_fstat, op_node_fs_fstat_sync } = core.ops;

const lazyStatUtils = core.createLazyLoader(
  "ext:deno_node/internal/fs/stat_utils.ts",
);
const lazyFsUtils = core.createLazyLoader(
  "ext:deno_node/internal/fs/utils.mjs",
);
const { denoErrorToNodeError } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);

function nodeFsStatToFileInfo(stat) {
  return {
    isFile: stat.isFile,
    isDirectory: stat.isDirectory,
    isSymlink: stat.isSymlink,
    size: stat.size,
    mtime: stat.mtimeMs != null ? new Date(stat.mtimeMs) : null,
    atime: stat.atimeMs != null ? new Date(stat.atimeMs) : null,
    birthtime: stat.birthtimeMs != null ? new Date(stat.birthtimeMs) : null,
    ctime: stat.ctimeMs != null ? new Date(stat.ctimeMs) : null,
    dev: stat.dev,
    ino: stat.ino ?? 0,
    mode: stat.mode,
    nlink: stat.nlink ?? 0,
    uid: stat.uid,
    gid: stat.gid,
    rdev: stat.rdev,
    blksize: stat.blksize,
    blocks: stat.blocks ?? 0,
    isBlockDevice: stat.isBlockDevice,
    isCharDevice: stat.isCharDevice,
    isFifo: stat.isFifo,
    isSocket: stat.isSocket,
  };
}

function fstat(
  fd,
  optionsOrCallback,
  maybeCallback,
) {
  fd = lazyFsUtils().getValidatedFd(fd);
  const callback = typeof optionsOrCallback === "function"
    ? optionsOrCallback
    : maybeCallback;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : { bigint: false };

  if (!callback) throw new Error("No callback function supplied");

  op_node_fs_fstat(fd).then(
    (stat) =>
      callback(
        null,
        lazyStatUtils().CFISBIS(nodeFsStatToFileInfo(stat), options.bigint),
      ),
    (err) => callback(denoErrorToNodeError(err, { syscall: "fstat" })),
  );
}

function fstatSync(
  fd,
  options,
) {
  fd = lazyFsUtils().getValidatedFd(fd);
  try {
    const stat = op_node_fs_fstat_sync(fd);
    return lazyStatUtils().CFISBIS(
      nodeFsStatToFileInfo(stat),
      options?.bigint || false,
    );
  } catch (err) {
    throw denoErrorToNodeError(err, { syscall: "fstat" });
  }
}

function fstatPromise(
  fd,
  options,
) {
  return new Promise((resolve, reject) => {
    fstat(fd, options, (err, stats) => {
      if (err) reject(err);
      else resolve(stats);
    });
  });
}

return { fstat, fstatSync, fstatPromise };
})();
