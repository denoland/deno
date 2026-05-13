// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
import type { WriteFileOptions } from "ext:deno_node/_fs/_fs_common.ts";
import type { Encodings } from "ext:deno_node/_utils.ts";
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");
const constants = core.loadExtScript("ext:deno_node/_fs/_fs_constants.ts");
import { copyFilePromise } from "ext:deno_node/_fs/_fs_copy.ts";
const { cpPromise } = core.loadExtScript("ext:deno_node/_fs/_fs_cp.ts");
import { lutimesPromise } from "ext:deno_node/_fs/_fs_lutimes.ts";
import { readdirPromise } from "ext:deno_node/_fs/_fs_readdir.ts";
const { lstatPromise } = core.loadExtScript("ext:deno_node/_fs/_fs_lstat.ts");
const lazyFs = core.createLazyLoader("node:fs");
import { globPromise } from "ext:deno_node/_fs/_fs_glob.ts";
import { getValidatedPathToString } from "ext:deno_node/internal/fs/utils.mjs";
import type { Buffer } from "node:buffer";
import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { FileHandle } from "ext:deno_node/internal/fs/handle.ts";
import { primordials } from "ext:core/mod.js";
const { parseFileMode } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
import { op_node_lchmod } from "ext:core/ops";
const { isMacOS } = core.loadExtScript("ext:deno_node/_util/os.ts");
const { ERR_METHOD_NOT_IMPLEMENTED } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const lazyPath = core.createLazyLoader("node:path");
const lazyProcess = core.createLazyLoader("node:process");

const { Promise, PromiseReject, SymbolAsyncDispose } = primordials;

// -- access --

const accessPromise = promisify(lazyFs().access) as (
  path: string | Buffer | URL,
  mode?: number,
) => Promise<void>;

// -- appendFile --

const appendFilePromise = promisify(lazyFs().appendFile) as (
  path: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// -- chmod --

const chmodPromise = promisify(lazyFs().chmod) as (
  path: string | Buffer | URL,
  mode: string | number,
) => Promise<void>;

// -- chown --

const chownPromise = promisify(lazyFs().chown) as (
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

const lchownPromise = promisify(lazyFs().lchown) as (
  path: string | Buffer | URL,
  uid: number,
  gid: number,
) => Promise<void>;

const linkPromise = promisify(lazyFs().link) as (
  existingPath: string | Buffer | URL,
  newPath: string | Buffer | URL,
) => Promise<void>;

const unlinkPromise = promisify(lazyFs().unlink) as (
  path: string | Buffer | URL,
) => Promise<void>;

const renamePromise = promisify(lazyFs().rename) as (
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

const rmPromise = promisify(lazyFs().rm) as (
  path: string | URL,
  options?: rmOptions,
) => Promise<void>;

// -- rmdir --

type rmdirOptions = {
  maxRetries?: number;
  recursive?: boolean;
  retryDelay?: number;
};

const rmdirPromise = promisify(lazyFs().rmdir) as (
  path: string | Buffer | URL,
  options?: rmdirOptions,
) => Promise<void>;

type MkdirOptions =
  | { recursive?: boolean; mode?: number | undefined }
  | number
  | boolean;

const mkdirPromise = promisify(lazyFs().mkdir) as (
  path: string | URL,
  options?: MkdirOptions,
) => Promise<string | undefined>;

const mkdtempPromise = promisify(lazyFs().mkdtemp) as (
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) => Promise<string>;

// Mirrors Node's lib/internal/fs/promises.js mkdtempDisposable(): create the
// temp dir, then return an object with .path, .remove(), and Symbol.asyncDispose
// that recursively removes the directory. Capture cwd at creation time so a
// later process.chdir() doesn't break removal.
async function mkdtempDisposablePromise(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) {
  const cwd = lazyProcess().default.cwd();
  const path = await mkdtempPromise(prefix, options);
  const fullPath = lazyPath().resolve(cwd, path);
  // `force: true` makes the second remove() a no-op when the dir is already
  // gone (Node's rimraf-based implementation treats ENOENT as success); other
  // errors (EACCES, EPERM, ...) still propagate.
  const remove = async () => {
    await rmPromise(fullPath, {
      force: true,
      maxRetries: 0,
      recursive: true,
      retryDelay: 0,
    });
  };
  return {
    __proto__: null,
    path,
    remove,
    async [SymbolAsyncDispose]() {
      await remove();
    },
  };
}

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
    lazyFs().open(path, flags, mode, (err, fd) => {
      if (err) reject(err);
      else resolve(new FileHandle(fd as number));
    });
  });
}

type OpendirOptions = {
  encoding?: string;
  bufferSize?: number;
};

const opendirPromise = promisify(lazyFs().opendir) as (
  path: string | Buffer | URL,
  options?: OpendirOptions,
) => Promise<Dir>;

// -- symlink --

const symlinkPromise = promisify(lazyFs().symlink) as (
  target: string | Buffer | URL,
  path: string | Buffer | URL,
  type?: string,
) => Promise<void>;

// -- truncate --

const truncatePromise = promisify(lazyFs().truncate) as (
  path: string | URL,
  len?: number,
) => Promise<void>;

// -- utimes --

const utimesPromise = promisify(lazyFs().utimes) as (
  path: string | URL,
  atime: number | string | Date,
  mtime: number | string | Date,
) => Promise<void>;

// -- writeFile --

const writeFilePromise = promisify(lazyFs().writeFile) as (
  pathOrRid: string | number | URL | FileHandle,
  data:
    | string
    | DataView
    | NodeJS.TypedArray
    | AsyncIterable<NodeJS.TypedArray | string>,
  options?: Encodings | WriteFileOptions,
) => Promise<void>;

// -- realpath --

const realpathPromise = promisify(lazyFs().realpath) as (
  path: string | Buffer,
  options?: string | { encoding?: string },
) => Promise<string | Buffer>;

// -- stat --

const statPromise = promisify(lazyFs().stat) as (
  path: string | Buffer | URL,
  options?: { bigint?: boolean },
) => Promise<unknown>;

// -- statfs --

const statfsPromise = promisify(lazyFs().statfs) as (
  path: string | Buffer | URL,
  options?: { bigint?: boolean },
) => Promise<unknown>;

// -- readFile / readlink --

const readFilePromise = promisify(lazyFs().readFile);

const readlinkPromise = promisify(lazyFs().readlink) as (
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
  mkdtempDisposable: mkdtempDisposablePromise,
  writeFile: writeFilePromise,
  appendFile: appendFilePromise,
  readFile: readFilePromise,
  watch: lazyFs().watchPromise,
};

export default promises;

export { mkdirPromise, opendirPromise };
