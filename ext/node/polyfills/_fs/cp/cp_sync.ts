// Copyright 2018-2025 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

import { dirname, isAbsolute, join, parse, resolve } from "node:path";
import { chmodSync } from "ext:deno_node/_fs/_fs_chmod.ts";
import { copyFileSync } from "ext:deno_node/_fs/_fs_copy.ts";
import { existsSync } from "ext:deno_node/_fs/_fs_exists.ts";
import { mkdirSync } from "ext:deno_node/_fs/_fs_mkdir.ts";
import { opendirSync } from "ext:deno_node/_fs/_fs_opendir.ts";
import { readlinkSync } from "ext:deno_node/_fs/_fs_readlink.ts";
import { symlinkSync } from "ext:deno_node/_fs/_fs_symlink.ts";
import { unlinkSync } from "ext:deno_node/_fs/_fs_unlink.ts";
import { utimesSync } from "ext:deno_node/_fs/_fs_utimes.ts";
import {
  ERR_FS_CP_DIR_TO_NON_DIR,
  ERR_FS_CP_EEXIST,
  ERR_FS_CP_EINVAL,
  ERR_FS_CP_FIFO_PIPE,
  ERR_FS_CP_NON_DIR_TO_DIR,
  ERR_FS_CP_SOCKET,
  ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY,
  ERR_FS_CP_UNKNOWN,
  ERR_FS_EISDIR,
  ERR_INVALID_RETURN_VALUE,
} from "ext:deno_node/internal/errors.ts";
import { core, primordials } from "ext:core/mod.js";
import { os } from "ext:deno_node/internal_binding/constants.ts";
import type { CopySyncOptions } from "ext:deno_node/_fs/cp/cp.d.ts";
import {
  areIdentical,
  type CheckPathsResult,
  isSrcSubdir,
} from "ext:deno_node/_fs/cp/cp.ts";

const {
  isPromise,
} = core;

const {
  ObjectPrototypeIsPrototypeOf,
} = primordials;

const {
  errno: {
    EEXIST,
    EISDIR,
    EINVAL,
    ENOTDIR,
  },
} = os;

function safeStatSyncFn<T extends typeof Deno.statSync>(
  statFn: T,
  path: string | URL,
): Deno.FileInfo | undefined {
  try {
    return statFn(path);
  } catch (error) {
    if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, error)) {
      return;
    }
    throw error;
  }
}

export function cpSyncFn(
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  if (opts.filter) {
    // `filter` is a option property from `cpSync`
    // deno-lint-ignore prefer-primordials
    const shouldCopy = opts.filter(src, dest);
    if (isPromise(shouldCopy)) {
      throw new ERR_INVALID_RETURN_VALUE("boolean", "filter", shouldCopy);
    }
    if (!shouldCopy) return;
  }

  const { srcStat, destStat, skipped } = checkPathsSync(src, dest, opts);
  if (skipped) return;
  checkParentPathsSync(src, srcStat, dest);
  return checkParentDir(destStat, src, dest, opts);
}

function checkPathsSync(
  src: string,
  dest: string,
  opts: CopySyncOptions,
): CheckPathsResult {
  if (opts.filter) {
    // `filter` is a option property from `cpSync`
    // deno-lint-ignore prefer-primordials
    const shouldCopy = opts.filter(src, dest);
    if (isPromise(shouldCopy)) {
      throw new ERR_INVALID_RETURN_VALUE("boolean", "filter", shouldCopy);
    }
    if (!shouldCopy) return { __proto__: null, skipped: true };
  }
  const { srcStat, destStat } = getStatsSync(src, dest, opts);

  if (destStat) {
    if (areIdentical(srcStat, destStat)) {
      throw new ERR_FS_CP_EINVAL({
        message: "src and dest cannot be the same",
        path: dest,
        syscall: "cp",
        errno: EINVAL,
        code: "EINVAL",
      });
    }
    if (srcStat.isDirectory && !destStat.isDirectory) {
      throw new ERR_FS_CP_DIR_TO_NON_DIR({
        message: `cannot overwrite non-directory ${dest} ` +
          `with directory ${src}`,
        path: dest,
        syscall: "cp",
        errno: EISDIR,
        code: "EISDIR",
      });
    }
    if (!srcStat.isDirectory && destStat.isDirectory) {
      throw new ERR_FS_CP_NON_DIR_TO_DIR({
        message: `cannot overwrite directory ${dest} ` +
          `with non-directory ${src}`,
        path: dest,
        syscall: "cp",
        errno: ENOTDIR,
        code: "ENOTDIR",
      });
    }
  }

  if (srcStat.isDirectory && isSrcSubdir(src, dest)) {
    throw new ERR_FS_CP_EINVAL({
      message: `cannot copy ${src} to a subdirectory of self ${dest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  }
  return { __proto__: null, srcStat, destStat, skipped: false };
}

type GetStatsSyncResult = {
  srcStat: Deno.FileInfo;
  destStat: Deno.FileInfo | undefined;
};
function getStatsSync(
  src: string,
  dest: string,
  opts: CopySyncOptions,
): GetStatsSyncResult {
  const statFunc = opts.dereference ? Deno.statSync : Deno.lstatSync;
  const srcStat = statFunc(src);
  const destStat = safeStatSyncFn(statFunc, dest);
  return { srcStat, destStat };
}

function checkParentPathsSync(
  src: string,
  srcStat: Deno.FileInfo,
  dest: string,
): void {
  const srcParent = resolve(dirname(src));
  const destParent = resolve(dirname(dest));
  if (destParent === srcParent || destParent === parse(destParent).root) return;
  const destStat = safeStatSyncFn(Deno.statSync, destParent);

  if (destStat === undefined) {
    return;
  }

  if (areIdentical(srcStat, destStat)) {
    throw new ERR_FS_CP_EINVAL({
      message: `cannot copy ${src} to a subdirectory of self ${dest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  }
  return checkParentPathsSync(src, srcStat, destParent);
}

function checkParentDir(
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  const destParent = dirname(dest);
  if (!existsSync(destParent)) mkdirSync(destParent, { recursive: true });
  return getStats(destStat, src, dest, opts);
}

function getStats(
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  const statSyncFn = opts.dereference ? Deno.statSync : Deno.lstatSync;
  const srcStat = statSyncFn(src);

  if (srcStat.isDirectory && opts.recursive) {
    return onDir(srcStat, destStat, src, dest, opts);
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
    return onFile(srcStat, destStat, src, dest, opts);
  } else if (srcStat.isSymlink) {
    return onLink(destStat, src, dest, opts);
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

function onFile(
  srcStat: Deno.FileInfo,
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  if (!destStat) return copyFile(srcStat, src, dest, opts);
  return mayCopyFile(srcStat, src, dest, opts);
}

function mayCopyFile(
  srcStat: Deno.FileInfo,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  if (opts.force) {
    unlinkSync(dest);
    return copyFile(srcStat, src, dest, opts);
  } else if (opts.errorOnExist) {
    throw new ERR_FS_CP_EEXIST({
      message: `${dest} already exists`,
      path: dest,
      syscall: "cp",
      errno: EEXIST,
      code: "EEXIST",
    });
  }
}

function copyFile(
  srcStat: Deno.FileInfo,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  copyFileSync(src, dest, opts.mode);
  if (opts.preserveTimestamps) handleTimestamps(srcStat.mode, src, dest);
  return setDestMode(dest, srcStat.mode);
}

function handleTimestamps(srcMode: number, src: string, dest: string): void {
  // Make sure the file is writable before setting the timestamp
  // otherwise open fails with EPERM when invoked with 'r+'
  // (through utimes call)
  if (fileIsNotWritable(srcMode)) makeFileWritable(dest, srcMode);
  return setDestTimestamps(src, dest);
}

function fileIsNotWritable(srcMode: number): boolean {
  return (srcMode & 0o200) === 0;
}

function makeFileWritable(dest: string, srcMode: number): void {
  return setDestMode(dest, srcMode | 0o200);
}

function setDestMode(dest: string, srcMode: number): void {
  return chmodSync(dest, srcMode);
}

function setDestTimestamps(src: string, dest: string): void {
  // The initial srcStat.atime cannot be trusted
  // because it is modified by the read(2) system call
  // (See https://nodejs.org/api/fs.html#fs_stat_time_values)
  const updatedSrcStat = Deno.statSync(src);
  return utimesSync(dest, updatedSrcStat.atime, updatedSrcStat.mtime);
}

function onDir(
  srcStat: Deno.FileInfo,
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  if (!destStat) return mkDirAndCopy(srcStat.mode, src, dest, opts);
  return copyDir(src, dest, opts);
}

function mkDirAndCopy(
  srcMode: number,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  mkdirSync(dest);
  copyDir(src, dest, opts);
  return setDestMode(dest, srcMode);
}

function copyDir(src: string, dest: string, opts: CopySyncOptions): void {
  const dir = opendirSync(src);

  try {
    let dirent;

    while ((dirent = dir.readSync()) !== null) {
      const { name } = dirent;
      const srcItem = join(src, name);
      const destItem = join(dest, name);
      const { destStat, skipped } = checkPathsSync(srcItem, destItem, opts);
      if (!skipped) getStats(destStat, srcItem, destItem, opts);
    }
  } finally {
    dir.closeSync();
  }
}

function onLink(
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  let resolvedSrc = readlinkSync(src) as string;
  if (!opts.verbatimSymlinks && !isAbsolute(resolvedSrc)) {
    resolvedSrc = resolve(dirname(src), resolvedSrc);
  }
  if (!destStat) {
    return symlinkSync(resolvedSrc, dest);
  }
  let resolvedDest;
  try {
    resolvedDest = readlinkSync(dest) as string;
  } catch (err) {
    // Dest exists and is a regular file or directory,
    // Windows may throw UNKNOWN error. If dest already exists,
    // fs throws error anyway, so no need to guard against it here.
    //@ts-expect-error `err.code` always exists
    if (err.code === "EINVAL" || err.code === "UNKNOWN") {
      return symlinkSync(resolvedSrc, dest);
    }
    throw err;
  }
  if (!isAbsolute(resolvedDest)) {
    resolvedDest = resolve(dirname(dest), resolvedDest);
  }

  if (
    Deno.statSync(src).isDirectory && isSrcSubdir(resolvedSrc, resolvedDest)
  ) {
    throw new ERR_FS_CP_EINVAL({
      message: `cannot copy ${resolvedSrc} to a subdirectory of self ` +
        `${resolvedDest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  }
  // Prevent copy if src is a subdir of dest since unlinking
  // dest in this case would result in removing src contents
  // and therefore a broken symlink would be created.
  if (
    Deno.statSync(dest).isDirectory && isSrcSubdir(resolvedDest, resolvedSrc)
  ) {
    throw new ERR_FS_CP_SYMLINK_TO_SUBDIRECTORY({
      message: `cannot overwrite ${resolvedDest} with ${resolvedSrc}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  }
  return copyLink(resolvedSrc, dest);
}

function copyLink(resolvedSrc: string, dest: string): void {
  unlinkSync(dest);
  return symlinkSync(resolvedSrc, dest);
}
