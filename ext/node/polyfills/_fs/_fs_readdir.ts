// Copyright 2018-2026 the Deno authors. MIT license.

import { core, primordials } from "ext:core/mod.js";
const { denoErrorToNodeError } = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
import {
  type Dirent,
  direntFromDeno,
  getValidatedPathToString,
} from "ext:deno_node/internal/fs/utils.mjs";
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
const { promisify } = core.loadExtScript("ext:deno_node/internal/util.mjs");
import {
  op_fs_read_dir_async,
  op_fs_read_dir_async_next,
  op_fs_read_dir_sync,
} from "ext:core/ops";
const lazyPath = core.createLazyLoader("node:path");

const {
  ArrayPrototypePush,
  ArrayPrototypeShift,
  ArrayPrototypeSort,
  Error,
  StringPrototypeCharCodeAt,
  StringPrototypeCodePointAt,
} = primordials;

// Node's `fs.readdir` returns entries sorted per directory, because it is backed
// by libuv's `uv__fs_scandir`, which sorts with a bytewise `strcmp` on the
// UTF-8 filenames. Because UTF-8 preserves code point ordering under a bytewise
// comparison, that is equivalent to comparing by Unicode code point.
//
// JavaScript's default `<` compares UTF-16 code units, which matches code point
// ordering for every name made only of BMP characters, the overwhelmingly common
// case. It only diverges once a name contains a surrogate (i.e. an astral
// character), whose lead code unit (0xD800-0xDBFF) sorts below the BMP
// characters in 0xE000-0xFFFF even though its code point is above all of them.
// So we scan a directory's entries once and fall back to the slower explicit
// code-point comparison only when a surrogate is present.
function compareEntriesFast(a: Deno.DirEntry, b: Deno.DirEntry): number {
  const an = a.name;
  const bn = b.name;
  return an < bn ? -1 : an > bn ? 1 : 0;
}

function compareEntriesUtf8(a: Deno.DirEntry, b: Deno.DirEntry): number {
  const an = a.name;
  const bn = b.name;
  if (an === bn) return 0;
  const aLen = an.length;
  const bLen = bn.length;
  let ai = 0;
  let bi = 0;
  while (ai < aLen && bi < bLen) {
    const ac = StringPrototypeCodePointAt(an, ai)!;
    const bc = StringPrototypeCodePointAt(bn, bi)!;
    if (ac !== bc) return ac < bc ? -1 : 1;
    ai += ac > 0xffff ? 2 : 1;
    bi += bc > 0xffff ? 2 : 1;
  }
  // One string is a prefix of the other; the shorter one sorts first.
  return (aLen - ai) - (bLen - bi);
}

function hasSurrogate(name: string): boolean {
  for (let i = 0; i < name.length; i++) {
    // 0xF800 masks off the low 11 bits, so this matches 0xD800-0xDFFF.
    if ((StringPrototypeCharCodeAt(name, i) & 0xf800) === 0xd800) return true;
  }
  return false;
}

function sortDirEntries(entries: Deno.DirEntry[]): Deno.DirEntry[] {
  if (entries.length < 2) return entries;
  let needsUtf8 = false;
  for (let i = 0; i < entries.length; i++) {
    if (hasSurrogate(entries[i].name)) {
      needsUtf8 = true;
      break;
    }
  }
  return ArrayPrototypeSort(
    entries,
    needsUtf8 ? compareEntriesUtf8 : compareEntriesFast,
  );
}

type readDirOptions = {
  encoding?: string;
  withFileTypes?: boolean;
  recursive?: boolean;
};

type readDirCallback = (err: Error | null, files: string[]) => void;

type readDirCallbackDirent = (err: Error | null, files: Dirent[]) => void;

type readDirBoth = (
  ...args: [Error] | [null, string[] | Dirent[] | Array<string | Dirent>]
) => void;

async function collectReadDir(path: string): Promise<Deno.DirEntry[]> {
  const rid = await op_fs_read_dir_async(path);
  const entries: Deno.DirEntry[] = [];
  try {
    while (true) {
      const entry = await op_fs_read_dir_async_next(rid);
      if (entry === null) {
        break;
      }
      ArrayPrototypePush(entries, entry);
    }
  } finally {
    core.close(rid);
  }
  return sortDirEntries(entries);
}

// Mirrors Node's lib/internal/fs/utils.js getOptions(): a bare string options
// arg is treated as { encoding: <string> }.
function normalizeOptions(
  options: readDirOptions | string | null | undefined,
): readDirOptions | null {
  if (typeof options === "string") {
    return { encoding: options };
  }
  return options ?? null;
}

function validateEncoding(encoding: string | undefined) {
  if (!encoding || encoding === "buffer") return;
  if (!Buffer.isEncoding(encoding)) {
    throw new Error(
      `TypeError [ERR_INVALID_OPT_VALUE_ENCODING]: The value "${encoding}" is invalid for option "encoding"`,
    );
  }
}

export function readdir(
  path: string | Buffer | URL,
  options: readDirOptions | string,
  callback: readDirCallback,
): void;
export function readdir(
  path: string | Buffer | URL,
  options: readDirOptions | string,
  callback: readDirCallbackDirent,
): void;
export function readdir(path: string | URL, callback: readDirCallback): void;
export function readdir(
  path: string | Buffer | URL,
  optionsOrCallback:
    | readDirOptions
    | string
    | readDirCallback
    | readDirCallbackDirent,
  maybeCallback?: readDirCallback | readDirCallbackDirent,
) {
  const callback =
    (typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback) as readDirBoth | undefined;
  const options = normalizeOptions(
    typeof optionsOrCallback === "function" ? null : optionsOrCallback,
  );
  path = getValidatedPathToString(path);

  if (!callback) throw new Error("No callback function supplied");

  validateEncoding(options?.encoding);

  const { join, relative } = lazyPath();
  const result: Array<string | Dirent> = [];
  const dirs = [path];
  let current: string | undefined;
  (async () => {
    while ((current = ArrayPrototypeShift(dirs)) !== undefined) {
      try {
        const entries = await collectReadDir(current);

        for (let i = 0; i < entries.length; i++) {
          const entry = entries[i];
          if (options?.recursive && entry.isDirectory) {
            ArrayPrototypePush(dirs, join(current, entry.name));
          }

          if (options?.withFileTypes) {
            entry.parentPath = current;
            ArrayPrototypePush(
              result,
              applyDirentEncoding(
                direntFromDeno(entry),
                options?.encoding,
              ),
            );
          } else {
            let name = entry.name;
            if (options?.recursive) {
              name = relative(path, join(current, name));
            }
            ArrayPrototypePush(result, decode(name, options?.encoding));
          }
        }
      } catch (err) {
        callback(
          denoErrorToNodeError(err as Error, {
            syscall: "readdir",
            path: current,
          }),
        );
        return;
      }
    }

    callback(null, result);
  })();
}

function applyDirentEncoding(dirent: Dirent, encoding?: string): Dirent {
  if (!encoding || encoding === "utf8" || encoding === "utf-8") {
    return dirent;
  }
  dirent.name = decode(dirent.name as string, encoding);
  if (typeof dirent.parentPath === "string") {
    dirent.parentPath = decode(dirent.parentPath, encoding);
  }
  return dirent;
}

function decode(str: string, encoding?: string): string | Buffer {
  if (!encoding || encoding === "utf8" || encoding === "utf-8") {
    return str;
  }
  // "buffer" returns Buffer instances; every other (Node-supported) encoding
  // re-encodes the UTF-8 filename through Buffer to match Node's
  // lib/internal/fs/utils.js getDirent / readdir output.
  const buf = Buffer.from(str, "utf8");
  if (encoding === "buffer") return buf;
  // No primordial exists for Buffer.prototype.toString with an encoding.
  // deno-lint-ignore deno-internal/prefer-primordials
  return buf.toString(encoding as BufferEncoding);
}

export const readdirPromise = promisify(readdir) as (
  & ((path: string | Buffer | URL, options: {
    withFileTypes: true;
    encoding?: string;
  }) => Promise<Dirent[]>)
  & ((path: string | Buffer | URL, options?: {
    withFileTypes?: false;
    encoding?: string;
  }) => Promise<string[]>)
);

export function readdirSync(
  path: string | Buffer | URL,
  options: { withFileTypes: true; encoding?: string } | string,
): Dirent[];
export function readdirSync(
  path: string | Buffer | URL,
  options?: { withFileTypes?: false; encoding?: string } | string,
): string[];
export function readdirSync(
  path: string | Buffer | URL,
  rawOptions?: readDirOptions | string,
): Array<string | Dirent> {
  const options = normalizeOptions(rawOptions);
  path = getValidatedPathToString(path);

  validateEncoding(options?.encoding);

  const { join, relative } = lazyPath();
  const result: Array<string | Dirent> = [];
  const dirs = [path];
  let current: string | undefined;
  while ((current = ArrayPrototypeShift(dirs)) !== undefined) {
    try {
      const entries = sortDirEntries(op_fs_read_dir_sync(current));

      for (let i = 0; i < entries.length; i++) {
        const entry = entries[i];
        if (options?.recursive && entry.isDirectory) {
          ArrayPrototypePush(dirs, join(current, entry.name));
        }

        if (options?.withFileTypes) {
          entry.parentPath = current;
          ArrayPrototypePush(
            result,
            applyDirentEncoding(
              direntFromDeno(entry),
              options?.encoding,
            ),
          );
        } else {
          let name = entry.name;
          if (options?.recursive) {
            name = relative(path, join(current, name));
          }
          ArrayPrototypePush(result, decode(name, options?.encoding));
        }
      }
    } catch (e) {
      throw denoErrorToNodeError(e as Error, {
        syscall: "readdir",
        path: current,
      });
    }
  }

  return result;
}
