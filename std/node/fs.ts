// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { access, accessSync } from "./_fs/_fs_access.ts";
import { appendFile, appendFileSync } from "./_fs/_fs_appendFile.ts";
import { chmod, chmodSync } from "./_fs/_fs_chmod.ts";
import { chown, chownSync } from "./_fs/_fs_chown.ts";
import { close, closeSync } from "./_fs/_fs_close.ts";
import * as constants from "./_fs/_fs_constants.ts";
import { readFile, readFileSync } from "./_fs/_fs_readFile.ts";
import { readlink, readlinkSync } from "./_fs/_fs_readlink.ts";
import { exists, existsSync } from "./_fs/_fs_exists.ts";
import { mkdir, mkdirSync } from "./_fs/_fs_mkdir.ts";
import { mkdtemp } from "./_fs/_fs_mkdtemp.ts";
import { copyFile, copyFileSync } from "./_fs/_fs_copy.ts";
import { writeFile, writeFileSync } from "./_fs/_fs_writeFile.ts";
import { readdir, readdirSync } from "./_fs/_fs_readdir.ts";
import { realpath, realpathSync } from "./_fs/_fs_realpath.ts";
import { rename, renameSync } from "./_fs/_fs_rename.ts";
import { rmdir, rmdirSync } from "./_fs/_fs_rmdir.ts";
import { unlink, unlinkSync } from "./_fs/_fs_unlink.ts";
import { watch } from "./_fs/_fs_watch.ts";
import { open, openSync } from "./_fs/_fs_open.ts";
import { stat, statSync } from "./_fs/_fs_stat.ts";
import { lstat, lstatSync } from "./_fs/_fs_lstat.ts";

import * as promises from "./_fs/promises/mod.ts";

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
  exists,
  existsSync,
  lstat,
  lstatSync,
  mkdir,
  mkdirSync,
  mkdtemp,
  open,
  openSync,
  promises,
  readdir,
  readdirSync,
  readFile,
  readFileSync,
  readlink,
  readlinkSync,
  realpath,
  realpathSync,
  rename,
  renameSync,
  rmdir,
  rmdirSync,
  stat,
  statSync,
  unlink,
  unlinkSync,
  watch,
  writeFile,
  writeFileSync,
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
  exists,
  existsSync,
  lstat,
  lstatSync,
  mkdir,
  mkdirSync,
  mkdtemp,
  open,
  openSync,
  promises,
  readdir,
  readdirSync,
  readFile,
  readFileSync,
  readlink,
  readlinkSync,
  realpath,
  realpathSync,
  rename,
  renameSync,
  rmdir,
  rmdirSync,
  stat,
  statSync,
  unlink,
  unlinkSync,
  watch,
  writeFile,
  writeFileSync,
};
