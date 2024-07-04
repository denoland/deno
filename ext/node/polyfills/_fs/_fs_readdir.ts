// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { TextDecoder, TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { asyncIterableToCallback } from "ext:deno_node/_fs/_fs_watch.ts";
import Dirent from "ext:deno_node/_fs/_fs_dirent.ts";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { getValidatedPath } from "ext:deno_node/internal/fs/utils.mjs";
import { Buffer } from "node:buffer";
import { promisify } from "ext:deno_node/internal/util.mjs";

function toDirent(val: Deno.DirEntry & { parentPath: string }): Dirent {
  return new Dirent(val);
}

type readDirOptions = {
  encoding?: string;
  withFileTypes?: boolean;
};

type readDirCallback = (err: Error | null, files: string[]) => void;

type readDirCallbackDirent = (err: Error | null, files: Dirent[]) => void;

type readDirBoth = (
  ...args: [Error] | [null, string[] | Dirent[] | Array<string | Dirent>]
) => void;

export function readdir(
  path: string | Buffer | URL,
  options: { withFileTypes?: false; encoding?: string },
  callback: readDirCallback,
): void;
export function readdir(
  path: string | Buffer | URL,
  options: { withFileTypes: true; encoding?: string },
  callback: readDirCallbackDirent,
): void;
export function readdir(path: string | URL, callback: readDirCallback): void;
export function readdir(
  path: string | Buffer | URL,
  optionsOrCallback: readDirOptions | readDirCallback | readDirCallbackDirent,
  maybeCallback?: readDirCallback | readDirCallbackDirent,
) {
  const callback =
    (typeof optionsOrCallback === "function"
      ? optionsOrCallback
      : maybeCallback) as readDirBoth | undefined;
  const options = typeof optionsOrCallback === "object"
    ? optionsOrCallback
    : null;
  const result: Array<string | Dirent> = [];
  path = getValidatedPath(path);

  if (!callback) throw new Error("No callback function supplied");

  if (options?.encoding) {
    try {
      new TextDecoder(options.encoding);
    } catch {
      throw new Error(
        `TypeError [ERR_INVALID_OPT_VALUE_ENCODING]: The value "${options.encoding}" is invalid for option "encoding"`,
      );
    }
  }

  try {
    path = path.toString();
    asyncIterableToCallback(Deno.readDir(path), (val, done) => {
      if (typeof path !== "string") return;
      if (done) {
        callback(null, result);
        return;
      }
      if (options?.withFileTypes) {
        val.parentPath = path;
        result.push(toDirent(val));
      } else result.push(decode(val.name));
    }, (e) => {
      callback(denoErrorToNodeError(e as Error, { syscall: "readdir" }));
    });
  } catch (e) {
    callback(denoErrorToNodeError(e as Error, { syscall: "readdir" }));
  }
}

function decode(str: string, encoding?: string): string {
  if (!encoding) return str;
  else {
    const decoder = new TextDecoder(encoding);
    const encoder = new TextEncoder();
    return decoder.decode(encoder.encode(str));
  }
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
  options: { withFileTypes: true; encoding?: string },
): Dirent[];
export function readdirSync(
  path: string | Buffer | URL,
  options?: { withFileTypes?: false; encoding?: string },
): string[];
export function readdirSync(
  path: string | Buffer | URL,
  options?: readDirOptions,
): Array<string | Dirent> {
  const result = [];
  path = getValidatedPath(path);

  if (options?.encoding) {
    try {
      new TextDecoder(options.encoding);
    } catch {
      throw new Error(
        `TypeError [ERR_INVALID_OPT_VALUE_ENCODING]: The value "${options.encoding}" is invalid for option "encoding"`,
      );
    }
  }

  try {
    path = path.toString();
    for (const file of Deno.readDirSync(path)) {
      if (options?.withFileTypes) {
        file.parentPath = path;
        result.push(toDirent(file));
      } else result.push(decode(file.name));
    }
  } catch (e) {
    throw denoErrorToNodeError(e as Error, { syscall: "readdir" });
  }
  return result;
}
