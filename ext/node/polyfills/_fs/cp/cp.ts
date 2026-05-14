// Copyright 2018-2026 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

// deno-lint-ignore-file prefer-primordials

(function () {
const { core, primordials } = globalThis.__bootstrap;
const lazyPath = core.createLazyLoader("node:path");
const lazyFsPromises = core.createLazyLoader(
  "ext:deno_node/internal/fs/promises.ts",
);
const lazyConstants = core.createLazyLoader("node:constants");
const {
  denoErrorToNodeError,
  ERR_FS_CP_DIR_TO_NON_DIR,
  ERR_FS_CP_EEXIST,
  ERR_FS_CP_EINVAL,
  ERR_FS_CP_FIFO_PIPE,
  ERR_FS_CP_NON_DIR_TO_DIR,
  ERR_FS_CP_SOCKET,
  ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY,
  ERR_FS_CP_UNKNOWN,
  ERR_FS_EISDIR,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const {
  op_node_cp_check_paths_recursive,
  op_node_cp_on_file,
  op_node_cp_on_link,
  op_node_cp_validate_and_prepare,
} = core.ops;

/**
  Layout (bigint):
  - bits 0..31  : source mode (`st_mode`) masked by `SrcModeMask`
  - bit 32      : whether destination exists
  - bits 33..39 : source entry type flags (dir, file, char/block device,
                  symlink, socket, fifo)
*/
const CpEntryFlags = {
  SrcModeMask: (1n << 32n) - 1n,
  IsDestExists: 1n << 32n,
  IsSrcDirectory: 1n << 33n,
  IsSrcFile: 1n << 34n,
  IsSrcCharDevice: 1n << 35n,
  IsSrcBlockDevice: 1n << 36n,
  IsSrcSymlink: 1n << 37n,
  IsSrcSocket: 1n << 38n,
  IsSrcFifo: 1n << 39n,
};

const {
  Number,
  PromiseResolve,
} = primordials;

function throwCpError(err) {
  const { EEXIST, EINVAL, EISDIR, ENOTDIR } = lazyConstants();
  switch (err.kind) {
    case "EINVAL":
      throw new ERR_FS_CP_EINVAL({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EINVAL,
        code: "EINVAL",
      });
    case "DIR_TO_NON_DIR":
      throw new ERR_FS_CP_DIR_TO_NON_DIR({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EISDIR,
        code: "EISDIR",
      });
    case "NON_DIR_TO_DIR":
      throw new ERR_FS_CP_NON_DIR_TO_DIR({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: ENOTDIR,
        code: "ENOTDIR",
      });
    case "EEXIST":
      throw new ERR_FS_CP_EEXIST({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EEXIST,
        code: "EEXIST",
      });
    case "EISDIR":
      throw new ERR_FS_EISDIR({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EISDIR,
        code: "EISDIR",
      });
    case "SOCKET":
      throw new ERR_FS_CP_SOCKET({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EINVAL,
        code: "EINVAL",
      });
    case "FIFO":
      throw new ERR_FS_CP_FIFO_PIPE({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EINVAL,
        code: "EINVAL",
      });
    case "UNKNOWN":
      throw new ERR_FS_CP_UNKNOWN({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EINVAL,
        code: "EINVAL",
      });
    case "SYMLINK_TO_SUBDIRECTORY":
      throw new ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY({
        message: err.message,
        path: err.path,
        syscall: "cp",
        errno: EINVAL,
        code: "EINVAL",
      });
    default:
      throw err;
  }
}

async function cpFn(
  src,
  dest,
  opts,
) {
  try {
    if (opts.filter && !(await opts.filter(src, dest))) return;
    const statInfo = await op_node_cp_validate_and_prepare(
      src,
      dest,
      opts.dereference,
    );
    return await getStatsForCopy(statInfo, src, dest, opts);
  } catch (err) {
    if (typeof err?.os_errno === "number") {
      throw denoErrorToNodeError(err, {
        path: err.path,
        dest: err.dest,
        syscall: err.syscall,
      });
    }

    throwCpError(err);
  }
}

function getStatsForCopy(
  statInfo,
  src,
  dest,
  opts,
) {
  const { EISDIR, EINVAL } = lazyConstants();
  if ((statInfo & CpEntryFlags.IsSrcDirectory) && opts.recursive) {
    return onDir(statInfo, src, dest, opts);
  } else if (statInfo & CpEntryFlags.IsSrcDirectory) {
    throw new ERR_FS_EISDIR({
      message: `${src} is a directory (not copied)`,
      path: src,
      syscall: "cp",
      errno: EISDIR,
      code: "EISDIR",
    });
  } else if (
    (statInfo & CpEntryFlags.IsSrcFile) ||
    (statInfo & CpEntryFlags.IsSrcCharDevice) ||
    (statInfo & CpEntryFlags.IsSrcBlockDevice)
  ) {
    return onFile(statInfo, src, dest, opts);
  } else if (statInfo & CpEntryFlags.IsSrcSymlink) {
    const isDestExists = !!(statInfo & CpEntryFlags.IsDestExists);
    return onLink(isDestExists, src, dest, opts);
  } else if (statInfo & CpEntryFlags.IsSrcSocket) {
    throw new ERR_FS_CP_SOCKET({
      message: `cannot copy a socket file: ${dest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  } else if (statInfo & CpEntryFlags.IsSrcFifo) {
    throw new ERR_FS_CP_FIFO_PIPE({
      message: `cannot copy a FIFO pipe: ${dest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  }
  throw new ERR_FS_CP_UNKNOWN({
    message: `cannot copy an unknown file type: ${dest}`,
    path: dest,
    syscall: "cp",
    errno: EINVAL,
    code: "EINVAL",
  });
}

async function onFile(
  statInfo,
  src,
  dest,
  opts,
) {
  const srcMode = Number(statInfo & CpEntryFlags.SrcModeMask);
  const isDestExists = !!(statInfo & CpEntryFlags.IsDestExists);
  await op_node_cp_on_file(
    src,
    dest,
    srcMode,
    isDestExists,
    opts.force,
    opts.errorOnExist,
    opts.preserveTimestamps,
    opts.mode ?? 0,
  );
}

function setDestMode(dest, srcMode) {
  if (!srcMode) return PromiseResolve();
  return Deno.chmod(dest, srcMode);
}

function onDir(
  statInfo,
  src,
  dest,
  opts,
) {
  if (!(statInfo & CpEntryFlags.IsDestExists)) {
    const srcMode = Number(statInfo & CpEntryFlags.SrcModeMask);
    return mkDirAndCopy(srcMode, src, dest, opts);
  }
  return copyDir(src, dest, opts);
}

async function mkDirAndCopy(
  srcMode,
  src,
  dest,
  opts,
) {
  await lazyFsPromises().mkdirPromise(dest);
  await copyDir(src, dest, opts);
  return setDestMode(dest, srcMode);
}

async function copyDir(
  src,
  dest,
  opts,
) {
  const dir = await lazyFsPromises().opendirPromise(src);
  for await (const { name } of dir) {
    const srcItem = lazyPath().join(src, name);
    const destItem = lazyPath().join(dest, name);
    if (opts.filter && !(await opts.filter(srcItem, destItem))) continue;
    const statInfo = await op_node_cp_check_paths_recursive(
      srcItem,
      destItem,
      opts.dereference,
    );
    await getStatsForCopy(statInfo, srcItem, destItem, opts);
  }
}

async function onLink(
  destExists,
  src,
  dest,
  opts,
) {
  await op_node_cp_on_link(
    src,
    dest,
    destExists,
    opts.verbatimSymlinks,
  );
}

return { throwCpError, cpFn };
})();
