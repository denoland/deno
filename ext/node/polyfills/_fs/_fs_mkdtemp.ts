// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

import { normalizeEncoding, promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";
import { makeCallback } from "ext:deno_node/_fs/_fs_common.ts";
import { Buffer } from "node:buffer";
import {
  getValidatedPathToString,
  warnOnNonPortableTemplate,
} from "ext:deno_node/internal/fs/utils.mjs";
import {
  denoErrorToNodeError,
  ERR_INVALID_ARG_TYPE,
} from "ext:deno_node/internal/errors.ts";
import { op_node_mkdtemp, op_node_mkdtemp_sync } from "ext:core/ops";
import type { Encoding } from "node:crypto";

const { PromisePrototypeThen } = primordials;

export type MkdtempCallback = (
  err: Error | null,
  directory?: string,
) => void;
export type MkdtempBufferCallback = (
  err: Error | null,
  directory?: Buffer<ArrayBufferLike>,
) => void;
type MkdTempPromise = (
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
) => Promise<string>;
type MkdTempPromiseBuffer = (
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: "buffer" } | "buffer",
) => Promise<Buffer<ArrayBufferLike>>;

// https://nodejs.org/dist/latest-v15.x/docs/api/fs.html#fs_fs_mkdtemp_prefix_options_callback
export function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  callback: MkdtempCallback,
): void;
export function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: "buffer" } | "buffer",
  callback: MkdtempBufferCallback,
): void;
export function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: string } | string,
  callback: MkdtempCallback,
): void;
export function mkdtemp(
  prefix: string | Buffer | Uint8Array | URL,
  options: { encoding: string } | string | MkdtempCallback | undefined,
  callback?: MkdtempCallback | MkdtempBufferCallback,
) {
  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }
  callback = makeCallback(callback);
  const encoding = parseEncoding(options);
  prefix = getValidatedPathToString(prefix, "prefix");

  warnOnNonPortableTemplate(prefix);

  PromisePrototypeThen(
    op_node_mkdtemp(prefix),
    (path: string) => callback(null, decode(path, encoding)),
    (err: Error) =>
      callback(denoErrorToNodeError(err, {
        syscall: "mkdtemp",
        path: `${prefix}XXXXXX`,
      })),
  );
}

export const mkdtempPromise = promisify(mkdtemp) as
  | MkdTempPromise
  | MkdTempPromiseBuffer;

// https://nodejs.org/dist/latest-v15.x/docs/api/fs.html#fs_fs_mkdtempsync_prefix_options
export function mkdtempSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: "buffer" } | "buffer",
): Buffer<ArrayBufferLike>;
export function mkdtempSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
): string;
export function mkdtempSync(
  prefix: string | Buffer | Uint8Array | URL,
  options?: { encoding: string } | string,
): string | Buffer<ArrayBufferLike> {
  const encoding = parseEncoding(options);
  prefix = getValidatedPathToString(prefix, "prefix");

  warnOnNonPortableTemplate(prefix);

  try {
    const path = op_node_mkdtemp_sync(prefix) as string;
    return decode(path, encoding);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, {
      syscall: "mkdtemp",
      path: `${prefix}XXXXXX`,
    });
  }
}

function decode(str: string, encoding: Encoding): string;
function decode(str: string, encoding: "buffer"): Buffer<ArrayBufferLike>;
function decode(
  str: string,
  encoding: Encoding | "buffer",
): string | Buffer<ArrayBufferLike> {
  if (encoding === "utf8") return str;
  const buffer = Buffer.from(str);
  if (encoding === "buffer") return buffer;
  // deno-lint-ignore prefer-primordials
  return buffer.toString(encoding);
}

function parseEncoding(
  options: string | { encoding?: string } | undefined,
): Encoding | "buffer" {
  let encoding: string | undefined;

  if (typeof options === "undefined" || options === null) {
    encoding = "utf8";
  } else if (typeof options === "string") {
    encoding = options;
  } else if (typeof options === "object") {
    encoding = options.encoding ?? "utf8";
  } else {
    throw new ERR_INVALID_ARG_TYPE("options", ["string", "Object"], options);
  }

  if (encoding === "buffer") {
    return encoding;
  }

  const parsedEncoding = normalizeEncoding(encoding);
  if (!parsedEncoding) {
    throw new ERR_INVALID_ARG_TYPE("encoding", encoding, "is invalid encoding");
  }

  return parsedEncoding;
}
