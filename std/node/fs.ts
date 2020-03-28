// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { access, accessSync } from "./_fs/_fs_access.ts";
import { appendFile, appendFileSync } from "./_fs/_fs_appendFile.ts";
import { chmod, chmodSync } from "./_fs/_fs_chmod.ts";
import { chown, chownSync } from "./_fs/_fs_chown.ts";
import { close, closeSync } from "./_fs/_fs_close.ts";
import * as constants from "./_fs/_fs_constants.ts";
import { readFile, readFileSync } from "./_fs/_fs_readFile.ts";
import { readlink, readlinkSync } from "./_fs/_fs_readlink.ts";

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
  readFile,
  readFileSync,
  readlink,
  readlinkSync,
};
