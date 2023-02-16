// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  access,
  accessPromise,
  accessSync,
} from "internal:deno_node/polyfills/_fs/_fs_access.ts";
import {
  appendFile,
  appendFilePromise,
  appendFileSync,
} from "internal:deno_node/polyfills/_fs/_fs_appendFile.ts";
import {
  chmod,
  chmodPromise,
  chmodSync,
} from "internal:deno_node/polyfills/_fs/_fs_chmod.ts";
import {
  chown,
  chownPromise,
  chownSync,
} from "internal:deno_node/polyfills/_fs/_fs_chown.ts";
import {
  close,
  closeSync,
} from "internal:deno_node/polyfills/_fs/_fs_close.ts";
import * as constants from "internal:deno_node/polyfills/_fs/_fs_constants.ts";
import {
  copyFile,
  copyFilePromise,
  copyFileSync,
} from "internal:deno_node/polyfills/_fs/_fs_copy.ts";
import Dir from "internal:deno_node/polyfills/_fs/_fs_dir.ts";
import Dirent from "internal:deno_node/polyfills/_fs/_fs_dirent.ts";
import {
  exists,
  existsSync,
} from "internal:deno_node/polyfills/_fs/_fs_exists.ts";
import {
  fdatasync,
  fdatasyncSync,
} from "internal:deno_node/polyfills/_fs/_fs_fdatasync.ts";
import {
  fstat,
  fstatSync,
} from "internal:deno_node/polyfills/_fs/_fs_fstat.ts";
import {
  fsync,
  fsyncSync,
} from "internal:deno_node/polyfills/_fs/_fs_fsync.ts";
import {
  ftruncate,
  ftruncateSync,
} from "internal:deno_node/polyfills/_fs/_fs_ftruncate.ts";
import {
  futimes,
  futimesSync,
} from "internal:deno_node/polyfills/_fs/_fs_futimes.ts";
import {
  link,
  linkPromise,
  linkSync,
} from "internal:deno_node/polyfills/_fs/_fs_link.ts";
import {
  lstat,
  lstatPromise,
  lstatSync,
} from "internal:deno_node/polyfills/_fs/_fs_lstat.ts";
import {
  mkdir,
  mkdirPromise,
  mkdirSync,
} from "internal:deno_node/polyfills/_fs/_fs_mkdir.ts";
import {
  mkdtemp,
  mkdtempPromise,
  mkdtempSync,
} from "internal:deno_node/polyfills/_fs/_fs_mkdtemp.ts";
import {
  open,
  openPromise,
  openSync,
} from "internal:deno_node/polyfills/_fs/_fs_open.ts";
import {
  opendir,
  opendirPromise,
  opendirSync,
} from "internal:deno_node/polyfills/_fs/_fs_opendir.ts";
import { read, readSync } from "internal:deno_node/polyfills/_fs/_fs_read.ts";
import {
  readdir,
  readdirPromise,
  readdirSync,
} from "internal:deno_node/polyfills/_fs/_fs_readdir.ts";
import {
  readFile,
  readFilePromise,
  readFileSync,
} from "internal:deno_node/polyfills/_fs/_fs_readFile.ts";
import {
  readlink,
  readlinkPromise,
  readlinkSync,
} from "internal:deno_node/polyfills/_fs/_fs_readlink.ts";
import {
  realpath,
  realpathPromise,
  realpathSync,
} from "internal:deno_node/polyfills/_fs/_fs_realpath.ts";
import {
  rename,
  renamePromise,
  renameSync,
} from "internal:deno_node/polyfills/_fs/_fs_rename.ts";
import {
  rmdir,
  rmdirPromise,
  rmdirSync,
} from "internal:deno_node/polyfills/_fs/_fs_rmdir.ts";
import {
  rm,
  rmPromise,
  rmSync,
} from "internal:deno_node/polyfills/_fs/_fs_rm.ts";
import {
  stat,
  statPromise,
  statSync,
} from "internal:deno_node/polyfills/_fs/_fs_stat.ts";
import {
  symlink,
  symlinkPromise,
  symlinkSync,
} from "internal:deno_node/polyfills/_fs/_fs_symlink.ts";
import {
  truncate,
  truncatePromise,
  truncateSync,
} from "internal:deno_node/polyfills/_fs/_fs_truncate.ts";
import {
  unlink,
  unlinkPromise,
  unlinkSync,
} from "internal:deno_node/polyfills/_fs/_fs_unlink.ts";
import {
  utimes,
  utimesPromise,
  utimesSync,
} from "internal:deno_node/polyfills/_fs/_fs_utimes.ts";
import {
  unwatchFile,
  watch,
  watchFile,
  watchPromise,
} from "internal:deno_node/polyfills/_fs/_fs_watch.ts";
// @deno-types="./_fs/_fs_write.d.ts"
import {
  write,
  writeSync,
} from "internal:deno_node/polyfills/_fs/_fs_write.mjs";
// @deno-types="./_fs/_fs_writev.d.ts"
import {
  writev,
  writevSync,
} from "internal:deno_node/polyfills/_fs/_fs_writev.mjs";
import {
  writeFile,
  writeFilePromise,
  writeFileSync,
} from "internal:deno_node/polyfills/_fs/_fs_writeFile.ts";
import { Stats } from "internal:deno_node/polyfills/internal/fs/utils.mjs";
// @deno-types="./internal/fs/streams.d.ts"
import {
  createReadStream,
  createWriteStream,
  ReadStream,
  WriteStream,
} from "internal:deno_node/polyfills/internal/fs/streams.mjs";

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

const promises = {
  access: accessPromise,
  copyFile: copyFilePromise,
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
  link: linkPromise,
  unlink: unlinkPromise,
  chmod: chmodPromise,
  // lchmod: promisify(lchmod),
  // lchown: promisify(lchown),
  chown: chownPromise,
  utimes: utimesPromise,
  // lutimes = promisify(lutimes),
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
  createReadStream,
  createWriteStream,
  Dir,
  Dirent,
  exists,
  existsSync,
  F_OK,
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
  link,
  linkSync,
  lstat,
  lstatSync,
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
  rename,
  renameSync,
  rmdir,
  rmdirSync,
  rm,
  rmSync,
  stat,
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
  writev,
  writevSync,
  writeFileSync,
  WriteStream,
  writeSync,
  X_OK,
};

export {
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
  createReadStream,
  createWriteStream,
  Dir,
  Dirent,
  exists,
  existsSync,
  F_OK,
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
  link,
  linkSync,
  lstat,
  lstatSync,
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
  realpath,
  realpathSync,
  rename,
  renameSync,
  rm,
  rmdir,
  rmdirSync,
  rmSync,
  stat,
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
