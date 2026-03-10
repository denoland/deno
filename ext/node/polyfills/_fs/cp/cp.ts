// Copyright 2018-2026 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

import { join } from "node:path";
import {
  mkdirPromise,
  opendirPromise,
} from "ext:deno_node/internal/fs/promises.ts";
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
import type { CopyOptions } from "node:fs";

export enum CpEntryFlags {
  IsDestExists = 1 << 0,
  IsDirectory = 1 << 1,
  IsFile = 1 << 2,
  IsCharDevice = 1 << 3,
  IsBlockDevice = 1 << 4,
  IsSymlink = 1 << 5,
  IsSocket = 1 << 6,
  IsFifo = 1 << 7,
}

export interface StatInfo {
  flags: number;
  mode: number;
}

const {
  PromiseResolve,
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
export function throwCpError(err: any): never {
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
    const statInfo = await op_node_cp_validate_and_prepare(
      src,
      dest,
      opts.dereference,
    );
    return await getStatsForCopy(statInfo, src, dest, opts);
  } catch (err) {
    if (typeof err?.os_errno === "number") {
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

function getStatsForCopy(
  statInfo: StatInfo,
  src: string,
  dest: string,
  opts: CopyOptions,
) {
  if (statInfo.flags & CpEntryFlags.IsDirectory && opts.recursive) {
    return onDir(statInfo, src, dest, opts);
  } else if (statInfo.flags & CpEntryFlags.IsDirectory) {
    throw new ERR_FS_EISDIR({
      message: `${src} is a directory (not copied)`,
      path: src,
      syscall: "cp",
      errno: EISDIR,
      code: "EISDIR",
    });
  } else if (
    statInfo.flags & CpEntryFlags.IsFile ||
    statInfo.flags & CpEntryFlags.IsCharDevice ||
    statInfo.flags & CpEntryFlags.IsBlockDevice
  ) {
    return onFile(statInfo, src, dest, opts);
  } else if (statInfo.flags & CpEntryFlags.IsSymlink) {
    return onLink(
      !!(statInfo.flags & CpEntryFlags.IsDestExists),
      src,
      dest,
      opts,
    );
  } else if (statInfo.flags & CpEntryFlags.IsSocket) {
    throw new ERR_FS_CP_SOCKET({
      message: `cannot copy a socket file: ${dest}`,
      path: dest,
      syscall: "cp",
      errno: EINVAL,
      code: "EINVAL",
    });
  } else if (statInfo.flags & CpEntryFlags.IsFifo) {
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
  statInfo: StatInfo,
  src: string,
  dest: string,
  opts: CopyOptions,
) {
  await op_node_cp_on_file(
    src,
    dest,
    statInfo.mode,
    !!(statInfo.flags & CpEntryFlags.IsDestExists),
    opts.force,
    opts.errorOnExist,
    opts.preserveTimestamps,
  );
}

function setDestMode(dest: string, srcMode: number | null): Promise<void> {
  if (!srcMode) return PromiseResolve();
  return Deno.chmod(dest, srcMode);
}

function onDir(
  statInfo: StatInfo,
  src: string,
  dest: string,
  opts: CopyOptions,
): Promise<void> {
  if (!(statInfo.flags & CpEntryFlags.IsDestExists)) {
    return mkDirAndCopy(statInfo.mode, src, dest, opts);
  }
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
    const statInfo = await op_node_cp_check_paths_recursive(
      srcItem,
      destItem,
      opts.dereference,
    );
    await getStatsForCopy(statInfo, srcItem, destItem, opts);
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
    opts.verbatimSymlinks,
  );
}
