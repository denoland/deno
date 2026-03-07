// Copyright 2018-2026 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

import { join, resolve, sep } from "node:path";
import { chmodPromise } from "ext:deno_node/_fs/_fs_chmod.ts";
import { mkdirPromise } from "ext:deno_node/_fs/_fs_mkdir.ts";
import { opendirPromise } from "ext:deno_node/_fs/_fs_opendir.ts";
import { EEXIST, EINVAL, EISDIR, ENOTDIR } from "node:constants";
import {
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
} from "ext:deno_node/internal/errors.ts";
import { primordials } from "ext:core/mod.js";
import {
  op_node_cp_check_paths_recursive,
  op_node_cp_on_file,
  op_node_cp_on_link,
  op_node_cp_validate_and_prepare,
} from "ext:core/ops";
import type { CopyOptions } from "ext:deno_node/_fs/cp/cp.d.ts";

const {
  ArrayPrototypeEvery,
  ArrayPrototypeFilter,
  Boolean,
  PromiseResolve,
  StringPrototypeSplit,
} = primordials;

export type CheckPathsResult = {
  __proto__: null;
  srcStat: Deno.FileInfo;
  destStat: Deno.FileInfo | undefined;
  skipped: false;
} | {
  __proto__: null;
  srcStat?: undefined;
  destStat?: undefined;
  skipped: true;
};

// deno-lint-ignore no-explicit-any
function throwCpError(err: any): never {
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

export async function cpFn(
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  try {
    // deno-lint-ignore prefer-primordials
    if (opts.filter && !(await opts.filter(src, dest))) return;
    const result = await op_node_cp_validate_and_prepare(
      src,
      dest,
      opts.dereference ?? false,
    );
    return await getStatsForCopy(result.dest_exists, src, dest, opts);
  } catch (err) {
    if (typeof err.os_errno === "number") {
      throw denoErrorToNodeError(err, {
        message: err.message,
        path: err.path,
        dest: err.dest,
        syscall: err.syscall,
      });
    }

    throwCpError(err);
  }
}

export function areIdentical(
  srcStat: Deno.FileInfo,
  destStat: Deno.FileInfo,
): boolean {
  return !!(destStat.ino && destStat.dev && destStat.ino === srcStat.ino &&
    destStat.dev === srcStat.dev);
}

const normalizePathToArray = (path: string): string[] =>
  ArrayPrototypeFilter(StringPrototypeSplit(resolve(path), sep), Boolean);

// Return true if dest is a subdir of src, otherwise false.
// It only checks the path strings.
export function isSrcSubdir(src: string, dest: string): boolean {
  const srcArr = normalizePathToArray(src);
  const destArr = normalizePathToArray(dest);
  return ArrayPrototypeEvery(srcArr, (cur, i) => destArr[i] === cur);
}

async function getStatsForCopy(
  destExists: boolean,
  src: string,
  dest: string,
  opts: CopyOptions,
) {
  const statFn = opts.dereference ? Deno.stat : Deno.lstat;
  const srcStat = await statFn(src);
  if (srcStat.isDirectory && opts.recursive) {
    return onDir(srcStat, destExists, src, dest, opts);
  } else if (srcStat.isDirectory) {
    throw new ERR_FS_EISDIR({
      message: `${src} is a directory (not copied)`,
      path: src,
      syscall: "cp",
      errno: EISDIR,
      code: "EISDIR",
    });
  } else if (
    srcStat.isFile ||
    srcStat.isCharDevice ||
    srcStat.isBlockDevice
  ) {
    return onFile(srcStat, destExists, src, dest, opts);
  } else if (srcStat.isSymlink) {
    return onLink(destExists, src, dest, opts);
  } else if (srcStat.isSocket) {
    throw new ERR_FS_CP_SOCKET({
      message: `cannot copy a socket file: ${dest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  } else if (srcStat.isFifo) {
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
  srcStat: Deno.FileInfo,
  destExists: boolean,
  src: string,
  dest: string,
  opts: CopyOptions,
) {
  await op_node_cp_on_file(
    src,
    dest,
    srcStat.mode ?? 0,
    destExists,
    opts.force ?? false,
    opts.errorOnExist ?? false,
    opts.preserveTimestamps ?? false,
  );
}

function setDestMode(dest: string, srcMode: number | null): Promise<void> {
  if (!srcMode) return PromiseResolve();
  return chmodPromise(dest, srcMode);
}

function onDir(
  srcStat: Deno.FileInfo,
  destExists: boolean,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  if (!destExists) return mkDirAndCopy(srcStat.mode, src, dest, opts);
  return copyDir(src, dest, opts);
}

async function mkDirAndCopy(
  srcMode: number | null,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  await mkdirPromise(dest);
  await copyDir(src, dest, opts);
  return setDestMode(dest, srcMode);
}

async function copyDir(
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  const dir = await opendirPromise(src);
  // deno-lint-ignore prefer-primordials
  for await (const { name } of dir) {
    const srcItem = join(src, name);
    const destItem = join(dest, name);
    // deno-lint-ignore prefer-primordials
    if (opts.filter && !(await opts.filter(srcItem, destItem))) continue;
    const result = await op_node_cp_check_paths_recursive(
      srcItem,
      destItem,
      opts.dereference ?? false,
    );
    await getStatsForCopy(result.dest_exists, srcItem, destItem, opts);
  }
}

async function onLink(
  destExists: boolean,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  await op_node_cp_on_link(
    src,
    dest,
    destExists,
    opts.verbatimSymlinks ?? false,
  );
}
