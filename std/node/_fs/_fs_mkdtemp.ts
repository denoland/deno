// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { existsSync } from "./_fs_exists.ts";
import {
  ERR_INVALID_CALLBACK,
  ERR_INVALID_OPT_VALUE_ENCODING,
} from "../_errors.ts";

export type mkdtempCallback = (
  err: Error | undefined,
  directory?: string,
) => void;

// https://nodejs.org/dist/latest-v15.x/docs/api/fs.html#fs_fs_mkdtemp_prefix_options_callback
export function mkdtemp(prefix: string, callback: mkdtempCallback): void;
export function mkdtemp(
  prefix: string,
  options: { encoding: string } | string,
  callback: mkdtempCallback,
): void;
export function mkdtemp(
  prefix: string,
  optionsOrCallback: { encoding: string } | string | mkdtempCallback,
  maybeCallback?: mkdtempCallback,
): void {
  const callback: mkdtempCallback | undefined =
    optionsOrCallback instanceof Function ? optionsOrCallback : maybeCallback;
  if (!callback) throw new ERR_INVALID_CALLBACK(callback);

  const encoding: string | undefined = parseEncoding(optionsOrCallback);
  const path = tempDirPath(prefix);

  Deno.mkdir(path, { recursive: false, mode: 0o700 })
    .then(() => callback(undefined, decode(path, encoding)))
    .catch((error) => callback(error));
}

// https://nodejs.org/dist/latest-v15.x/docs/api/fs.html#fs_fs_mkdtempsync_prefix_options
export function mkdtempSync(
  prefix: string,
  options?: { encoding: string } | string,
): string {
  const encoding: string | undefined = parseEncoding(options);
  const path = tempDirPath(prefix);

  Deno.mkdirSync(path, { recursive: false, mode: 0o700 });
  return decode(path, encoding);
}

function parseEncoding(
  optionsOrCallback?: { encoding: string } | string | mkdtempCallback,
): string | undefined {
  let encoding: string | undefined;
  if (optionsOrCallback instanceof Function) encoding = undefined;
  else if (optionsOrCallback instanceof Object) {
    encoding = optionsOrCallback?.encoding;
  } else encoding = optionsOrCallback;

  if (encoding) {
    try {
      new TextDecoder(encoding);
    } catch (error) {
      throw new ERR_INVALID_OPT_VALUE_ENCODING(encoding);
    }
  }

  return encoding;
}

function decode(str: string, encoding?: string): string {
  if (!encoding) return str;
  else {
    const decoder = new TextDecoder(encoding);
    const encoder = new TextEncoder();
    return decoder.decode(encoder.encode(str));
  }
}

const CHARS = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
function randomName(): string {
  return [...Array(6)].map(() =>
    CHARS[Math.floor(Math.random() * CHARS.length)]
  ).join("");
}

function tempDirPath(prefix: string): string {
  let path: string;
  do {
    path = prefix + randomName();
  } while (existsSync(path));

  return path;
}
