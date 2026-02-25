// Copyright 2018-2026 the Deno authors. MIT license.
import {
  access,
  accessPromise,
  accessSync,
} from "ext:deno_node/_fs/_fs_access.ts";
import {
  appendFile,
  appendFilePromise,
  appendFileSync,
} from "ext:deno_node/_fs/_fs_appendFile.ts";
import { chmod, chmodPromise, chmodSync } from "ext:deno_node/_fs/_fs_chmod.ts";
import { chown, chownPromise, chownSync } from "ext:deno_node/_fs/_fs_chown.ts";
import { close, closeSync } from "ext:deno_node/_fs/_fs_close.ts";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";
import {
  copyFile,
  copyFilePromise,
  copyFileSync,
} from "ext:deno_node/_fs/_fs_copy.ts";
import { cp, cpPromise, cpSync } from "ext:deno_node/_fs/_fs_cp.ts";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { exists, existsSync } from "ext:deno_node/_fs/_fs_exists.ts";
import { fchmod, fchmodSync } from "ext:deno_node/_fs/_fs_fchmod.ts";
import { fchown, fchownSync } from "ext:deno_node/_fs/_fs_fchown.ts";
import { fdatasync, fdatasyncSync } from "ext:deno_node/_fs/_fs_fdatasync.ts";
import { fstat, fstatSync } from "ext:deno_node/_fs/_fs_fstat.ts";
import { fsync, fsyncSync } from "ext:deno_node/_fs/_fs_fsync.ts";
import { ftruncate, ftruncateSync } from "ext:deno_node/_fs/_fs_ftruncate.ts";
import { futimes, futimesSync } from "ext:deno_node/_fs/_fs_futimes.ts";
import {
  lchmod,
  lchmodPromise,
  lchmodSync,
} from "ext:deno_node/_fs/_fs_lchmod.ts";
import {
  lchown,
  lchownPromise,
  lchownSync,
} from "ext:deno_node/_fs/_fs_lchown.ts";
import { link, linkPromise, linkSync } from "ext:deno_node/_fs/_fs_link.ts";
import { lstat, lstatPromise, lstatSync } from "ext:deno_node/_fs/_fs_lstat.ts";
import {
  lutimes,
  lutimesPromise,
  lutimesSync,
} from "ext:deno_node/_fs/_fs_lutimes.ts";
import { mkdir, mkdirPromise, mkdirSync } from "ext:deno_node/_fs/_fs_mkdir.ts";
import {
  mkdtemp,
  mkdtempPromise,
  mkdtempSync,
} from "ext:deno_node/_fs/_fs_mkdtemp.ts";
import { open, openPromise, openSync } from "ext:deno_node/_fs/_fs_open.ts";
import {
  opendir,
  opendirPromise,
  opendirSync,
} from "ext:deno_node/_fs/_fs_opendir.ts";
import { read, readSync } from "ext:deno_node/_fs/_fs_read.ts";
import {
  readdir,
  readdirPromise,
  readdirSync,
} from "ext:deno_node/_fs/_fs_readdir.ts";
import {
  readFile,
  readFilePromise,
  readFileSync,
} from "ext:deno_node/_fs/_fs_readFile.ts";
import {
  readlink,
  readlinkPromise,
  readlinkSync,
} from "ext:deno_node/_fs/_fs_readlink.ts";
import {
  realpath,
  realpathPromise,
  realpathSync,
} from "ext:deno_node/_fs/_fs_realpath.ts";
import {
  rename,
  renamePromise,
  renameSync,
} from "ext:deno_node/_fs/_fs_rename.ts";
import { rmdir, rmdirPromise, rmdirSync } from "ext:deno_node/_fs/_fs_rmdir.ts";
import { rm, rmPromise, rmSync } from "ext:deno_node/_fs/_fs_rm.ts";
import {
  stat,
  statPromise,
  Stats,
  statSync,
} from "ext:deno_node/_fs/_fs_stat.ts";
import {
  statfs,
  statfsPromise,
  statfsSync,
} from "ext:deno_node/_fs/_fs_statfs.ts";
import {
  symlink,
  symlinkPromise,
  symlinkSync,
} from "ext:deno_node/_fs/_fs_symlink.ts";
import {
  truncate,
  truncatePromise,
  truncateSync,
} from "ext:deno_node/_fs/_fs_truncate.ts";
import {
  unlink,
  unlinkPromise,
  unlinkSync,
} from "ext:deno_node/_fs/_fs_unlink.ts";
import {
  utimes,
  utimesPromise,
  utimesSync,
} from "ext:deno_node/_fs/_fs_utimes.ts";
import {
  unwatchFile,
  watch,
  watchFile,
  watchPromise,
} from "ext:deno_node/_fs/_fs_watch.ts";
// @deno-types="./_fs/_fs_write.d.ts"
import { write, writeSync } from "ext:deno_node/_fs/_fs_write.ts";
// @deno-types="./_fs/_fs_writev.d.ts"
import { writev, writevSync } from "ext:deno_node/_fs/_fs_writev.ts";
import { readv, readvSync } from "ext:deno_node/_fs/_fs_readv.ts";
import {
  writeFile,
  writeFilePromise,
  writeFileSync,
} from "ext:deno_node/_fs/_fs_writeFile.ts";
// @deno-types="./internal/fs/streams.d.ts"
import {
  createReadStream,
  createWriteStream,
  ReadStream,
  WriteStream,
} from "ext:deno_node/internal/fs/streams.mjs";
import {
  Dirent,
  getValidatedPath,
  toUnixTimestamp as _toUnixTimestamp,
} from "ext:deno_node/internal/fs/utils.mjs";
import { glob, globPromise, globSync } from "ext:deno_node/_fs/_fs_glob.ts";
import {
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
import {
  op_fs_file_stat_async,
  op_fs_open_async,
  op_fs_seek_async,
  op_fs_stat_async,
} from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";
import { createLazyBlob } from "ext:deno_web/09_file.js";

const { MathMin, Uint8Array } = primordials;

const {
  F_OK,
  R_OK,
  W_OK,
  X_OK,
  O_RDONLY,
  O_WRONLY,
  O_RDWR,
  O_NOCTTY,
  O_TRUNC,
  O_APPEND,
  O_DIRECTORY,
  O_NOFOLLOW,
  O_SYNC,
  O_DSYNC,
  O_SYMLINK,
  O_NONBLOCK,
  O_CREAT,
  O_EXCL,
} = constants;

/**
 * A blob part backed by a file on disk. Reads data lazily from the file
 * only when stream()/text()/arrayBuffer() is called on the containing Blob.
 * Detects file modifications by comparing the current size against the
 * expected size recorded at blob creation time.
 */
class FileBlobPart {
  #path: string;
  #start: number;
  size: number;
  #expectedFileSize: number;

  constructor(
    path: string,
    expectedFileSize: number,
    start: number,
    length: number,
  ) {
    this.#path = path;
    this.#start = start;
    this.size = length;
    this.#expectedFileSize = expectedFileSize;
  }

  slice(start: number, end: number): FileBlobPart {
    return new FileBlobPart(
      this.#path,
      this.#expectedFileSize,
      this.#start + start,
      end - start,
    );
  }

  #throwNotReadable(): never {
    throw new DOMException(
      "The requested file could not be read, " +
        "typically due to permission problems that have occurred after " +
        "a reference to a file was acquired.",
      "NotReadableError",
    );
  }

  async *stream(): AsyncGenerator<Uint8Array> {
    // Check that the file hasn't been modified since the blob was created
    const stat = await op_fs_stat_async(this.#path);
    if (stat.size !== this.#expectedFileSize) {
      this.#throwNotReadable();
    }

    if (this.size === 0) {
      return;
    }

    const rid = await op_fs_open_async(this.#path, undefined);
    try {
      if (this.#start > 0) {
        await op_fs_seek_async(rid, this.#start, 0); // SeekMode.Start
      }

      let remaining = this.size;
      while (remaining > 0) {
        const chunkSize = MathMin(remaining, 65536);
        const buf = new Uint8Array(chunkSize);
        const nread = await core.read(rid, buf);
        if (nread === 0) break;
        remaining -= nread;
        yield nread < chunkSize ? buf.subarray(0, nread) : buf;

        // Check that the file hasn't been modified during reading
        if (remaining > 0) {
          const currentStat = await op_fs_file_stat_async(rid);
          if (currentStat.size !== this.#expectedFileSize) {
            this.#throwNotReadable();
          }
        }
      }
    } finally {
      core.close(rid);
    }
  }
}

/**
 * Returns a `Blob` whose data is read lazily from the given file.
 */
async function openAsBlob(
  path: string | Buffer | URL,
  options: { type?: string } = { __proto__: null },
): Promise<Blob> {
  validateObject(options, "options");
  const type = options.type || "";
  validateString(type, "options.type");
  path = getValidatedPath(path);

  const stat = await op_fs_stat_async(path as string);
  const fileSize = stat.size;
  const part = new FileBlobPart(path as string, fileSize, 0, fileSize);

  return createLazyBlob([part], fileSize, type);
}

const promises = {
  access: accessPromise,
  constants,
  copyFile: copyFilePromise,
  cp: cpPromise,
  glob: globPromise,
  open: openPromise,
  opendir: opendirPromise,
  rename: renamePromise,
  truncate: truncatePromise,
  rm: rmPromise,
  rmdir: rmdirPromise,
  mkdir: mkdirPromise,
  readdir: readdirPromise,
  readlink: readlinkPromise,
  symlink: symlinkPromise,
  lstat: lstatPromise,
  stat: statPromise,
  statfs: statfsPromise,
  link: linkPromise,
  unlink: unlinkPromise,
  chmod: chmodPromise,
  lchmod: lchmodPromise,
  lchown: lchownPromise,
  chown: chownPromise,
  utimes: utimesPromise,
  lutimes: lutimesPromise,
  realpath: realpathPromise,
  mkdtemp: mkdtempPromise,
  writeFile: writeFilePromise,
  appendFile: appendFilePromise,
  readFile: readFilePromise,
  watch: watchPromise,
};

export default {
  access,
  accessSync,
  appendFile,
  appendFileSync,
  chmod,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFile,
  copyFileSync,
  cp,
  cpSync,
  createReadStream,
  createWriteStream,
  Dir,
  Dirent,
  exists,
  existsSync,
  F_OK,
  fchmod,
  fchmodSync,
  fchown,
  fchownSync,
  fdatasync,
  fdatasyncSync,
  fstat,
  fstatSync,
  fsync,
  fsyncSync,
  ftruncate,
  ftruncateSync,
  futimes,
  futimesSync,
  glob,
  globSync,
  lchmod,
  lchmodSync,
  lchown,
  lchownSync,
  link,
  linkSync,
  lstat,
  lstatSync,
  lutimes,
  lutimesSync,
  mkdir,
  mkdirSync,
  mkdtemp,
  mkdtempSync,
  O_APPEND,
  O_CREAT,
  O_DIRECTORY,
  O_DSYNC,
  O_EXCL,
  O_NOCTTY,
  O_NOFOLLOW,
  O_NONBLOCK,
  O_RDONLY,
  O_RDWR,
  O_SYMLINK,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
  open,
  openAsBlob,
  openSync,
  opendir,
  opendirSync,
  read,
  readSync,
  promises,
  R_OK,
  readdir,
  readdirSync,
  readFile,
  readFileSync,
  readlink,
  readlinkSync,
  ReadStream,
  realpath,
  realpathSync,
  readv,
  readvSync,
  rename,
  renameSync,
  rmdir,
  rmdirSync,
  rm,
  rmSync,
  stat,
  Stats,
  statSync,
  statfs,
  statfsSync,
  symlink,
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkSync,
  unwatchFile,
  utimes,
  utimesSync,
  W_OK,
  watch,
  watchFile,
  write,
  writeFile,
  writev,
  writevSync,
  writeFileSync,
  WriteStream,
  writeSync,
  X_OK,
  // For tests
  _toUnixTimestamp,
};

export {
  // For tests
  _toUnixTimestamp,
  access,
  accessSync,
  appendFile,
  appendFileSync,
  chmod,
  chmodSync,
  chown,
  chownSync,
  close,
  closeSync,
  constants,
  copyFile,
  copyFileSync,
  cp,
  cpSync,
  createReadStream,
  createWriteStream,
  Dir,
  Dirent,
  exists,
  existsSync,
  F_OK,
  fchmod,
  fchmodSync,
  fchown,
  fchownSync,
  fdatasync,
  fdatasyncSync,
  fstat,
  fstatSync,
  fsync,
  fsyncSync,
  ftruncate,
  ftruncateSync,
  futimes,
  futimesSync,
  glob,
  globSync,
  lchmod,
  lchmodSync,
  link,
  linkSync,
  lstat,
  lstatSync,
  lutimes,
  lutimesSync,
  mkdir,
  mkdirSync,
  mkdtemp,
  mkdtempSync,
  O_APPEND,
  O_CREAT,
  O_DIRECTORY,
  O_DSYNC,
  O_EXCL,
  O_NOCTTY,
  O_NOFOLLOW,
  O_NONBLOCK,
  O_RDONLY,
  O_RDWR,
  O_SYMLINK,
  O_SYNC,
  O_TRUNC,
  O_WRONLY,
  open,
  openAsBlob,
  opendir,
  opendirSync,
  openSync,
  promises,
  R_OK,
  read,
  readdir,
  readdirSync,
  readFile,
  readFileSync,
  readlink,
  readlinkSync,
  ReadStream,
  readSync,
  readv,
  readvSync,
  realpath,
  realpathSync,
  rename,
  renameSync,
  rm,
  rmdir,
  rmdirSync,
  rmSync,
  stat,
  statfs,
  statfsSync,
  Stats,
  statSync,
  symlink,
  symlinkSync,
  truncate,
  truncateSync,
  unlink,
  unlinkSync,
  unwatchFile,
  utimes,
  utimesSync,
  W_OK,
  watch,
  watchFile,
  write,
  writeFile,
  writeFileSync,
  WriteStream,
  writeSync,
  writev,
  writevSync,
  X_OK,
};
