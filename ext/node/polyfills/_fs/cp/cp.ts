// Copyright 2018-2025 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

import { dirname, isAbsolute, join, parse, resolve, sep } from "node:path";
import { chmodPromise } from "ext:deno_node/_fs/_fs_chmod.ts";
import { copyFilePromise } from "ext:deno_node/_fs/_fs_copy.ts";
import { mkdirPromise } from "ext:deno_node/_fs/_fs_mkdir.ts";
import { opendirPromise } from "ext:deno_node/_fs/_fs_opendir.ts";
import { readlinkPromise } from "ext:deno_node/_fs/_fs_readlink.ts";
import { symlinkPromise } from "ext:deno_node/_fs/_fs_symlink.ts";
import { unlinkPromise } from "ext:deno_node/_fs/_fs_unlink.ts";
import { utimesPromise } from "ext:deno_node/_fs/_fs_utimes.ts";
import { EEXIST, EINVAL, EISDIR, ENOTDIR } from "node:constants";
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
} from "ext:deno_node/internal/errors.ts";
import { primordials } from "ext:core/mod.js";
import type { CopyOptions } from "ext:deno_node/_fs/cp/cp.d.ts";

const {
  ArrayPrototypeEvery,
  ArrayPrototypeFilter,
  Boolean,
  ObjectPrototypeIsPrototypeOf,
  PromiseResolve,
  SafePromiseAll,
  StringPrototypeSplit,
} = primordials;

// Deno.stat and Deno.lstat are preferred over node:fs versions because
// they are more performant, as the node:fs versions use the Deno implementation
// under the hood, and adds overhead by converting the result to a node fs.Stats object.

async function safeStatFn<T extends typeof Deno.stat>(
  statFn: T,
  path: string | URL,
): Promise<Deno.FileInfo | undefined> {
  try {
    return await statFn(path);
  } catch (error) {
    if (ObjectPrototypeIsPrototypeOf(Deno.errors.NotFound.prototype, error)) {
      return;
    }
    throw error;
  }
}

export async function cpFn(
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  const stats = await checkPaths(src, dest, opts);
  const { srcStat, destStat, skipped } = stats;
  if (skipped) return;
  await checkParentPaths(src, srcStat, dest);
  return checkParentDir(destStat, src, dest, opts);
}

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

async function checkPaths(
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<CheckPathsResult> {
  // `filter` is a option property from `cpSync`
  // deno-lint-ignore prefer-primordials
  if (opts.filter && !(await opts.filter(src, dest))) {
    return { __proto__: null, skipped: true };
  }
  const { 0: srcStat, 1: destStat } = await getStats(src, dest, opts);
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

export function areIdentical(
  srcStat: Deno.FileInfo,
  destStat: Deno.FileInfo,
): boolean {
  return !!(destStat.ino && destStat.dev && destStat.ino === srcStat.ino &&
    destStat.dev === srcStat.dev);
}

function getStats(
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<[Deno.FileInfo, Deno.FileInfo | undefined]> {
  const statFunc = opts.dereference
    ? (file: string) => Deno.stat(file)
    : (file: string) => Deno.lstat(file);

  return SafePromiseAll([
    statFunc(src),
    safeStatFn(statFunc, dest),
  ]);
}

async function checkParentDir(
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  const destParent = dirname(dest);
  const dirExists = await pathExists(destParent);
  if (dirExists) return getStatsForCopy(destStat, src, dest, opts);
  await mkdirPromise(destParent, { recursive: true });
  return getStatsForCopy(destStat, src, dest, opts);
}

async function pathExists(dest: string): Promise<boolean> {
  const hasStat = await safeStatFn(Deno.stat, dest);
  return hasStat !== undefined;
}

// Recursively check if dest parent is a subdirectory of src.
// It works for all file types including symlinks since it
// checks the src and dest inodes. It starts from the deepest
// parent and stops once it reaches the src parent or the root path.
async function checkParentPaths(
  src: string,
  srcStat: Deno.FileInfo,
  dest: string,
): Promise<void> {
  const srcParent = resolve(dirname(src));
  const destParent = resolve(dirname(dest));
  if (destParent === srcParent || destParent === parse(destParent).root) {
    return;
  }
  const destStat = await safeStatFn(Deno.stat, destParent);
  if (!destStat) {
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
  return checkParentPaths(src, srcStat, destParent);
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
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopyOptions,
) {
  const statFn = opts.dereference ? Deno.stat : Deno.lstat;
  const srcStat = await statFn(src);
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
  opts: CopyOptions,
) {
  if (!destStat) return _copyFile(srcStat, src, dest, opts);
  return mayCopyFile(srcStat, src, dest, opts);
}

async function mayCopyFile(
  srcStat: Deno.FileInfo,
  src: string,
  dest: string,
  opts: CopyOptions,
) {
  if (opts.force) {
    await unlinkPromise(dest);
    return _copyFile(srcStat, src, dest, opts);
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

async function _copyFile(
  srcStat: Deno.FileInfo,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  await copyFilePromise(src, dest, opts.mode);
  if (opts.preserveTimestamps) {
    return handleTimestampsAndMode(srcStat.mode, src, dest);
  }
  return setDestMode(dest, srcStat.mode);
}

async function handleTimestampsAndMode(
  srcMode: number | null,
  src: string,
  dest: string,
): Promise<void> {
  // Make sure the file is writable before setting the timestamp
  // otherwise open fails with EPERM when invoked with 'r+'
  // (through utimes call)
  if (fileIsNotWritable(srcMode)) {
    await makeFileWritable(dest, srcMode);
  }
  return setDestTimestampsAndMode(srcMode, src, dest);
}

function fileIsNotWritable(srcMode: number | null): boolean {
  return (srcMode! & 0o200) === 0;
}

function makeFileWritable(dest: string, srcMode: number | null): Promise<void> {
  return setDestMode(dest, srcMode! | 0o200);
}

async function setDestTimestampsAndMode(
  srcMode: number | null,
  src: string,
  dest: string,
): Promise<void> {
  await setDestTimestamps(src, dest);
  return setDestMode(dest, srcMode);
}

function setDestMode(dest: string, srcMode: number | null): Promise<void> {
  if (!srcMode) return PromiseResolve();
  return chmodPromise(dest, srcMode);
}

async function setDestTimestamps(src: string, dest: string): Promise<void> {
  // The initial srcStat.atime cannot be trusted
  // because it is modified by the read(2) system call
  // (See https://nodejs.org/api/fs.html#fs_stat_time_values)
  const updatedSrcStat = await Deno.stat(src);
  if (!updatedSrcStat.atime || !updatedSrcStat.mtime) return;
  return utimesPromise(dest, updatedSrcStat.atime, updatedSrcStat.mtime);
}

function onDir(
  srcStat: Deno.FileInfo,
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  if (!destStat) return mkDirAndCopy(srcStat.mode, src, dest, opts);
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
    const { destStat, skipped } = await checkPaths(srcItem, destItem, opts);
    if (!skipped) await getStatsForCopy(destStat, srcItem, destItem, opts);
  }
}

async function onLink(
  destStat: Deno.FileInfo | undefined,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  let resolvedSrc = await readlinkPromise(src) as string;
  if (!opts.verbatimSymlinks && !isAbsolute(resolvedSrc)) {
    resolvedSrc = resolve(dirname(src), resolvedSrc);
  }
  if (!destStat) {
    return symlinkPromise(resolvedSrc, dest);
  }
  let resolvedDest;
  try {
    resolvedDest = await readlinkPromise(dest) as string;
  } catch (err) {
    // Dest exists and is a regular file or directory,
    // Windows may throw UNKNOWN error. If dest already exists,
    // fs throws error anyway, so no need to guard against it here.
    //@ts-expect-error readlink error always contains a code
    if (err.code === "EINVAL" || err.code === "UNKNOWN") {
      return symlinkPromise(resolvedSrc, dest);
    }
    throw err;
  }
  if (!isAbsolute(resolvedDest)) {
    resolvedDest = resolve(dirname(dest), resolvedDest);
  }

  const srcStat = await Deno.stat(src);
  const srcIsDir = srcStat.isDirectory;
  if (srcIsDir && isSrcSubdir(resolvedSrc, resolvedDest)) {
    throw new ERR_FS_CP_EINVAL({
      message: `cannot copy ${resolvedSrc} to a subdirectory of self ` +
        `${resolvedDest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  }
  // Do not copy if src is a subdir of dest since unlinking
  // dest in this case would result in removing src contents
  // and therefore a broken symlink would be created.
  if (srcIsDir && isSrcSubdir(resolvedDest, resolvedSrc)) {
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

async function copyLink(resolvedSrc: string, dest: string): Promise<void> {
  await unlinkPromise(dest);
  return symlinkPromise(resolvedSrc, dest);
}
