// Copyright 2018-2026 the Deno authors. MIT license.
// Adapted from Node.js. Copyright Joyent, Inc. and other Node contributors.

import { join } from "node:path";
import { type CopySyncOptions, mkdirSync, opendirSync } from "node:fs";
import { EINVAL, EISDIR } from "node:constants";
import {
  denoErrorToNodeError,
  ERR_FS_CP_FIFO_PIPE,
  ERR_FS_CP_SOCKET,
  ERR_FS_CP_UNKNOWN,
  ERR_FS_EISDIR,
  ERR_INVALID_RETURN_VALUE,
} from "ext:deno_node/internal/errors.ts";
import { core } from "ext:core/mod.js";
import {
  op_node_cp_check_paths_recursive_sync,
  op_node_cp_on_file_sync,
  op_node_cp_on_link_sync,
  op_node_cp_validate_and_prepare_sync,
} from "ext:core/ops";
import {
  CpEntryFlags,
  StatInfo,
  throwCpError,
} from "ext:deno_node/_fs/cp/cp.ts";

const {
  isPromise,
} = core;

export function cpSyncFn(
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  try {
    if (opts.filter) {
      // deno-lint-ignore prefer-primordials
      const shouldCopy = opts.filter(src, dest);
      if (isPromise(shouldCopy)) {
        throw new ERR_INVALID_RETURN_VALUE("boolean", "filter", shouldCopy);
      }
      if (!shouldCopy) return;
    }

    const statInfo = op_node_cp_validate_and_prepare_sync(
      src,
      dest,
      opts.dereference,
    );
    return getStatsForCopy(statInfo, src, dest, opts);
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
  statInfo: StatInfo,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
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

function onFile(
  statInfo: StatInfo,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  op_node_cp_on_file_sync(
    src,
    dest,
    statInfo.mode,
    !!(statInfo.flags & CpEntryFlags.IsDestExists),
    opts.force,
    opts.errorOnExist,
    opts.preserveTimestamps,
  );
}

function setDestMode(dest: string, srcMode: number): void {
  if (!srcMode) return;
  Deno.chmodSync(dest, srcMode);
}

function onDir(
  statInfo: StatInfo,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  if (!(statInfo.flags & CpEntryFlags.IsDestExists)) {
    return mkDirAndCopy(statInfo.mode, src, dest, opts);
  }
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

      // deno-lint-ignore prefer-primordials
      if (opts.filter && !opts.filter(srcItem, destItem)) {
        continue;
      }

      const statInfo = op_node_cp_check_paths_recursive_sync(
        srcItem,
        destItem,
        opts.dereference,
      );
      getStatsForCopy(statInfo, srcItem, destItem, opts);
    }
  } finally {
    dir.closeSync();
  }
}

function onLink(
  destExists: boolean,
  src: string,
  dest: string,
  opts: CopySyncOptions,
): void {
  op_node_cp_on_link_sync(
    src,
    dest,
    destExists,
    opts.verbatimSymlinks,
  );
}
