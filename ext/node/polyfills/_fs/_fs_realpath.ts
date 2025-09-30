// Copyright 2018-2025 the Deno authors. MIT license.

import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";
import { Buffer } from "node:buffer";
import {
  getOptions,
  getValidatedPathToString,
} from "ext:deno_node/internal/fs/utils.mjs";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import { BufferEncoding } from "ext:deno_node/_global.d.ts";

type Encoding = BufferEncoding | "buffer";
type EncodingObj = { encoding?: Encoding };
type Options = Encoding | EncodingObj;
type Callback = (err: Error | null, path?: string | Buffer) => void;

const { PromisePrototypeThen } = primordials;

function encodeRealpathResult(
  result: string,
  options?: EncodingObj,
): string | Buffer {
  if (!options || !options.encoding || options.encoding === "utf8") {
    return result;
  }

  const asBuffer = Buffer.from(result);
  if (options.encoding === "buffer") {
    return asBuffer;
  }
  // deno-lint-ignore prefer-primordials
  return asBuffer.toString(options.encoding);
}

export function realpath(
  path: string | Buffer,
  options?: Options | Callback | Encoding,
  callback?: Callback,
) {
  if (typeof options === "function") {
    callback = options;
  }
  validateFunction(callback, "cb");
  options = getOptions(options) as EncodingObj;
  path = getValidatedPathToString(path);

  PromisePrototypeThen(
    Deno.realPath(path),
    (path) => callback!(null, encodeRealpathResult(path, options)),
    (err) => callback!(err),
  );
}

realpath.native = realpath;

export const realpathPromise = promisify(realpath) as (
  path: string | Buffer,
  options?: Options,
) => Promise<string | Buffer>;

export function realpathSync(
  path: string,
  options?: Options | Encoding,
): string | Buffer {
  options = getOptions(options) as EncodingObj;
  path = getValidatedPathToString(path);
  const realPath = Deno.realPathSync(path);
  return encodeRealpathResult(realPath, options);
}

realpathSync.native = realpathSync;
