// Copyright 2018-2025 the Deno authors. MIT license.

import Dir from "ext:deno_node/_fs/_fs_dir.ts";
import { Buffer } from "node:buffer";
import {
  getOptions,
  getValidatedPath,
} from "ext:deno_node/internal/fs/utils.mjs";
import { denoErrorToNodeError } from "ext:deno_node/internal/errors.ts";
import {
  validateFunction,
  validateInteger,
} from "ext:deno_node/internal/validators.mjs";
import { promisify } from "ext:deno_node/internal/util.mjs";
import { primordials } from "ext:core/mod.js";

const {
  StringPrototypeToString,
} = primordials;

/** These options aren't functionally used right now, as `Dir` doesn't yet support them.
 * However, these values are still validated.
 */
type Options = {
  encoding?: string;
  bufferSize?: number;
};
type Callback = (err?: Error | null, dir?: Dir) => void;

function _validateFunction(callback: unknown): asserts callback is Callback {
  validateFunction(callback, "callback");
}

function getPathString(
  path: string | Buffer | URL,
): string {
  if (Buffer.isBuffer(path)) {
    // deno-lint-ignore prefer-primordials
    return path.toString();
  }

  return StringPrototypeToString(path);
}

/** @link https://nodejs.org/api/fs.html#fsopendirsyncpath-options */
export function opendir(
  path: string | Buffer | URL,
  options: Options | Callback,
  callback?: Callback,
) {
  callback = typeof options === "function" ? options : callback;
  _validateFunction(callback);

  path = getPathString(getValidatedPath(path));

  let err, dir;
  try {
    const { bufferSize } = getOptions(options, {
      encoding: "utf8",
      bufferSize: 32,
    });
    validateInteger(bufferSize, "options.bufferSize", 1, 4294967295);

    /** Throws if path is invalid */
    Deno.readDirSync(path);

    dir = new Dir(path);
  } catch (error) {
    err = denoErrorToNodeError(error as Error, { syscall: "opendir" });
  }
  if (err) {
    callback(err);
  } else {
    callback(null, dir);
  }
}

/** @link https://nodejs.org/api/fs.html#fspromisesopendirpath-options */
export const opendirPromise = promisify(opendir) as (
  path: string | Buffer | URL,
  options?: Options,
) => Promise<Dir>;

export function opendirSync(
  path: string | Buffer | URL,
  options?: Options,
): Dir {
  path = getPathString(getValidatedPath(path));

  const { bufferSize } = getOptions(options, {
    encoding: "utf8",
    bufferSize: 32,
  });

  validateInteger(bufferSize, "options.bufferSize", 1, 4294967295);

  try {
    /** Throws if path is invalid */
    Deno.readDirSync(path);

    return new Dir(path);
  } catch (err) {
    throw denoErrorToNodeError(err as Error, { syscall: "opendir" });
  }
}
