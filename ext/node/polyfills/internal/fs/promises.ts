// Copyright 2018-2026 the Deno authors. MIT license.

import { fs as fsConstants } from "ext:deno_node/internal_binding/constants.ts";
import { codeMap } from "ext:deno_node/internal_binding/uv.ts";
import {
  type CallbackWithError,
  isFd,
  makeCallback,
  maybeCallback,
  type WriteFileOptions,
} from "ext:deno_node/_fs/_fs_common.ts";
import type { Encodings } from "ext:deno_node/_utils.ts";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";
import { copyFilePromise } from "ext:deno_node/_fs/_fs_copy.ts";
import { cpPromise } from "ext:deno_node/_fs/_fs_cp.ts";
import { lchmodPromise } from "ext:deno_node/_fs/_fs_lchmod.ts";
import { lchownPromise } from "ext:deno_node/_fs/_fs_lchown.ts";
import { lstatPromise } from "ext:deno_node/_fs/_fs_lstat.ts";
import { lutimesPromise } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { mkdirPromise } from "ext:deno_node/_fs/_fs_mkdir.ts";
import { mkdtempPromise } from "ext:deno_node/_fs/_fs_mkdtemp.ts";
import { openPromise } from "ext:deno_node/_fs/_fs_open.ts";
import { opendirPromise } from "ext:deno_node/_fs/_fs_opendir.ts";
import { readdirPromise } from "ext:deno_node/_fs/_fs_readdir.ts";
import { readFilePromise } from "ext:deno_node/_fs/_fs_readFile.ts";
import { readlinkPromise } from "ext:deno_node/_fs/_fs_readlink.ts";
import { realpathPromise } from "ext:deno_node/_fs/_fs_realpath.ts";
import { renamePromise } from "ext:deno_node/_fs/_fs_rename.ts";
import { rmdirPromise } from "ext:deno_node/_fs/_fs_rmdir.ts";
import { rmPromise } from "ext:deno_node/_fs/_fs_rm.ts";
import { statPromise } from "ext:deno_node/_fs/_fs_stat.ts";
import { statfsPromise } from "ext:deno_node/_fs/_fs_statfs.ts";
import { symlinkPromise } from "ext:deno_node/_fs/_fs_symlink.ts";
import { truncatePromise } from "ext:deno_node/_fs/_fs_truncate.ts";
import { utimesPromise } from "ext:deno_node/_fs/_fs_utimes.ts";
import { watchPromise } from "ext:deno_node/_fs/_fs_watch.ts";
import {
  writeFile,
  writeFilePromise,
} from "ext:deno_node/_fs/_fs_writeFile.ts";
import { globPromise } from "ext:deno_node/_fs/_fs_glob.ts";
import {
  copyObject,
  getOptions,
  getValidatedPath,
  getValidatedPathToString,
  getValidMode,
  kMaxUserId,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  parseFileMode,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";
import type { Buffer } from "node:buffer";
import { primordials } from "ext:core/mod.js";

const { Error, PromisePrototypeThen } = primordials;

// -- access --

function access(
  path: string | Buffer | URL,
  mode: number | CallbackWithError,
  callback?: CallbackWithError,
) {
  if (typeof mode === "function") {
    callback = mode;
    mode = fsConstants.F_OK;
  }

  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  mode = getValidMode(mode, "access");
  const cb = makeCallback(callback);

  // deno-lint-ignore prefer-primordials
  Deno.lstat(path).then(
    (info) => {
      if (info.mode === null) {
        cb(null);
        return;
      }
      let m = +mode || 0;
      let fileMode = +info.mode || 0;

      if (Deno.build.os === "windows") {
        m &= ~fsConstants.X_OK;
      } else if (info.uid === Deno.uid()) {
        fileMode >>= 6;
      }

      if ((m & fileMode) === m) {
        cb(null);
      } else {
        // deno-lint-ignore no-explicit-any
        const e: any = new Error(`EACCES: permission denied, access '${path}'`);
        e.path = path;
        e.syscall = "access";
        e.errno = codeMap.get("EACCES");
        e.code = "EACCES";
        cb(e);
      }
    },
    (err) => {
      // deno-lint-ignore prefer-primordials
      if (err instanceof Deno.errors.NotFound) {
        // deno-lint-ignore no-explicit-any
        const e: any = new Error(
          `ENOENT: no such file or directory, access '${path}'`,
        );
        e.path = path;
        e.syscall = "access";
        e.errno = codeMap.get("ENOENT");
        e.code = "ENOENT";
        cb(e);
      } else {
        cb(err);
      }
    },
  );
}

const accessPromise = promisify(access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

// -- appendFile --

function appendFile(
  path: string | number | URL,
  data: string | Uint8Array,
  options: Encodings | WriteFileOptions | CallbackWithError,
  callback?: CallbackWithError,
) {
  callback = maybeCallback(callback || options);
  options = getOptions(options, { encoding: "utf8", mode: 0o666, flag: "a" });

  // Don't make changes directly on options object
  options = copyObject(options);

  // Force append behavior when using a supplied file descriptor
  if (!options.flag || isFd(path)) {
    options.flag = "a";
  }

  writeFile(path, data, options, callback);
}

const appendFilePromise = promisify(appendFile) as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// -- chmod --

function chmod(
  path: string | Buffer | URL,
  mode: string | number,
  callback: CallbackWithError,
) {
  path = getValidatedPathToString(path);
  mode = parseFileMode(mode, "mode");

  PromisePrototypeThen(
    Deno.chmod(path, mode),
    () => callback(null),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "chmod", path })),
  );
}

const chmodPromise = promisify(chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

// -- chown --

function chown(
  path: string | Buffer | URL,
  uid: number,
  gid: number,
  callback: CallbackWithError,
) {
  callback = makeCallback(callback);
  // deno-lint-ignore prefer-primordials
  path = getValidatedPath(path).toString();
  validateInteger(uid, "uid", -1, kMaxUserId);
  validateInteger(gid, "gid", -1, kMaxUserId);

  // deno-lint-ignore prefer-primordials
  Deno.chown(path, uid, gid).then(
    () => callback(null),
    callback,
  );
}

const chownPromise = promisify(chown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

// -- link --

function link(
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
  callback: CallbackWithError,
) {
  existingPath = getValidatedPathToString(existingPath);
  newPath = getValidatedPathToString(newPath);

  PromisePrototypeThen(
    Deno.link(existingPath, newPath),
    () => callback(null),
    callback,
  );
}

const linkPromise = promisify(link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

// -- unlink --

function unlink(
  path: string | Buffer | URL,
  callback: (err?: Error) => void,
): void {
  path = getValidatedPathToString(path);

  PromisePrototypeThen(
    Deno.remove(path),
    () => callback(),
    (err: Error) =>
      callback(denoErrorToNodeError(err, { syscall: "unlink", path })),
  );
}

const unlinkPromise = promisify(unlink) as (
  path: string | Buffer | URL,
) => Promise<void>;

// -- promises object --

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

export default promises;
