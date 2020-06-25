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
import { copyFile, copyFileSync } from "./_fs/_fs_copy.ts";
import { writeFile, writeFileSync } from "./_fs/_fs_writeFile.ts";
import * as promises from "./_fs/promises/mod.ts";

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
  readFile,
  readFileSync,
  readlink,
  readlinkSync,
  mkdir,
  mkdirSync,
  writeFile,
  writeFileSync,
  promises,
};
