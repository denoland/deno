// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { TextDecoder } from "ext:deno_web/08_text_encoding.js";
import Dirent from "ext:deno_node/_fs/_fs_dirent.ts";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import { getValidatedPath } from "ext:deno_node/internal/fs/utils.mjs";
import { Buffer } from "node:buffer";
import { promisify } from "ext:deno_node/internal/util.mjs";
import {
  op_fs_read_dir_async,
  op_fs_read_dir_names_async,
  op_fs_read_dir_names_sync,
  op_fs_read_dir_sync,
} from "ext:core/ops";

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

  path = path.toString();
  if (options?.withFileTypes) {
    op_fs_read_dir_async(path)
      .then(
        (files) => {
          const result: Dirent[] = [];

          try {
            for (let i = 0; i < files.length; i++) {
              const file = files[i];
              result.push(new Dirent(file.name, path, file));
            }
            callback(null, result);
          } catch (e) {
            callback(denoErrorToNodeError(e as Error, { syscall: "readdir" }));
          }
        },
        (e: Error) => callback(denoErrorToNodeError(e, { syscall: "readdir" })),
      );
  } else {
    op_fs_read_dir_names_async(path)
      .then(
        (fileNames) => callback(null, fileNames),
        (e: Error) => callback(denoErrorToNodeError(e, { syscall: "readdir" })),
      );
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
    if (options?.withFileTypes) {
      const result = [];
      const files = op_fs_read_dir_sync(path);
      for (let i = 0; i < files.length; i++) {
        const file = files[i];
        result.push(new Dirent(file.name, path, file));
      }
      return result;
    } else {
      return op_fs_read_dir_names_sync(path);
    }
  } catch (e) {
    throw denoErrorToNodeError(e as Error, { syscall: "readdir" });
  }
}
