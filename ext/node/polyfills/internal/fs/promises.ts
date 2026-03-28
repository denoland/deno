// Copyright 2018-2026 the Deno authors. MIT license.

import { type WriteFileOptions } from "ext:deno_node/_fs/_fs_common.ts";
import type { Encodings } from "ext:deno_node/_utils.ts";
import { promisify } from "ext:deno_node/internal/util.mjs";
import * as constants from "ext:deno_node/_fs/_fs_constants.ts";
import { copyFilePromise } from "ext:deno_node/_fs/_fs_copy.ts";
import { cpPromise } from "ext:deno_node/_fs/_fs_cp.ts";
import { lutimesPromise } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { readdirPromise } from "ext:deno_node/_fs/_fs_readdir.ts";
import { lstatPromise } from "ext:deno_node/_fs/_fs_lstat.ts";
import {
  access,
  appendFile,
  chmod,
  chown,
  lchown,
  link,
  mkdir,
  mkdtemp,
  open,
  opendir,
  readFile,
  readlink,
  realpath,
  rename,
  rm,
  rmdir,
  stat,
  statfs,
  symlink,
  truncate,
  unlink,
  utimes,
  watchPromise,
  writeFile,
} from "node:fs";
import { globPromise } from "ext:deno_node/_fs/_fs_glob.ts";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import { parseFileMode } from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "node:buffer";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { primordials } from "ext:core/mod.js";
import { op_node_lchmod } from "ext:core/ops";
import { isMacOS } from "ext:deno_node/_util/os.ts";
import { ERR_METHOD_NOT_IMPLEMENTED } from "ext:deno_node/internal/errors.ts";

const { Promise, PromiseReject } = primordials;

// -- access --

const accessPromise = promisify(access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

// -- appendFile --

const appendFilePromise = promisify(appendFile) as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// -- chmod --

const chmodPromise = promisify(chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

// -- chown --

const chownPromise = promisify(chown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

const lchmodPromise: (
  path: string | Buffer | URL,
  mode: number,
) => Promise<void> = !isMacOS
  ? () => PromiseReject(new ERR_METHOD_NOT_IMPLEMENTED("lchmod()"))
  : async (path: string | Buffer | URL, mode: number) => {
    path = getValidatedPathToString(path);
    mode = parseFileMode(mode, "mode");
    return await op_node_lchmod(path, mode);
  };

const lchownPromise = promisify(lchown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

const linkPromise = promisify(link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

const unlinkPromise = promisify(unlink) as (
  path: string | Buffer | URL,
) => Promise<void>;

const renamePromise = promisify(rename) as (
  oldPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

// -- rm --

type rmOptions = {
  force?: boolean;
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

const rmPromise = promisify(rm) as (
  path: string | URL,
  options?: rmOptions,
) => Promise<void>;

// -- rmdir --

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

const rmdirPromise = promisify(rmdir) as (
  path: string | Buffer | URL,
  options?: rmdirOptions,
) => Promise<void>;

type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

const mkdirPromise = promisify(mkdir) as (
  path: string | URL,
  options?: MkdirOptions,
) => Promise<string | undefined>;

const mkdtempPromise = promisify(mkdtemp) as (
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) => Promise<string>;

type OpenFlags =
  | "a"
  | "ax"
  | "a+"
  | "ax+"
  | "as"
  | "as+"
  | "r"
  | "r+"
  | "rs"
  | "rs+"
  | "w"
  | "wx"
  | "w+"
  | "wx+"
  | number
  | string;

function openPromise(
  path: string | Buffer | URL,
  flags: OpenFlags = "r",
  mode = 0o666,
): Promise<FileHandle> {
  return new Promise((resolve, reject) => {
    open(path, flags, mode, (err, fd) => {
      if (err) reject(err);
      else resolve(new FileHandle(fd as number));
    });
  });
}

type OpendirOptions = {
  encoding?: string;
  bufferSize?: number;
};

const opendirPromise = promisify(opendir) as (
  path: string | Buffer | URL,
  options?: OpendirOptions,
) => Promise<Dir>;

// -- symlink --

const symlinkPromise = promisify(symlink) as (
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: string,
) => Promise<void>;

// -- truncate --

const truncatePromise = promisify(truncate) as (
  path: string | URL,
  len?: number,
) => Promise<void>;

// -- utimes --

const utimesPromise = promisify(utimes) as (
  path: string | URL,
  atime: number | string | Date,
  mtime: number | string | Date,
) => Promise<void>;

// -- writeFile --

const writeFilePromise = promisify(writeFile) as (
  pathOrRid: string | number | URL | FileHandle,
  data:
    | string
    | DataView
    | NodeJS.TypedArray
    | AsyncIterable<NodeJS.TypedArray | string>,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// -- realpath --

const realpathPromise = promisify(realpath) as (
  path: string | Buffer,
  options?: string | { encoding?: string },
) => Promise<string | Buffer>;

// -- stat --

const statPromise = promisify(stat) as (
  path: string | Buffer | URL,
  options?: { bigint?: boolean },
) => Promise<unknown>;

// -- statfs --

const statfsPromise = promisify(statfs) as (
  path: string | Buffer | URL,
  options?: { bigint?: boolean },
) => Promise<unknown>;

// -- readFile / readlink --

const readFilePromise = promisify(readFile);

const readlinkPromise = promisify(readlink) as (
  path: string | Buffer | URL,
  opt?: { encoding?: string | null },
) => Promise<string | Uint8Array>;

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

export { mkdirPromise, opendirPromise };
