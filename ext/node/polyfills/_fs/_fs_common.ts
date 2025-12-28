// Copyright 2018-2025 the Deno authors. MIT license.

import { primordials } from "ext:core/mod.js";
const {
  StringPrototypeToLowerCase,
  ArrayPrototypeIncludes,
  ReflectApply,
  Error,
} = primordials;
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import type { ErrnoException } from "ext:deno_node/_global.d.ts";
import {
  BinaryEncodings,
  Encodings,
  notImplemented,
  TextEncodings,
} from "ext:deno_node/_utils.ts";
import { assertEncoding } from "ext:deno_node/internal/fs/utils.mjs";

export type CallbackWithError = (err: ErrnoException | null) => void;

export interface FileOptions {
  encoding?: Encodings;
  flag?: string;
  signal?: AbortSignal;
}

export type TextOptionsArgument =
  | TextEncodings
  | ({ encoding: TextEncodings } & FileOptions);
export type BinaryOptionsArgument =
  | BinaryEncodings
  | ({ encoding: BinaryEncodings } & FileOptions);
export type FileOptionsArgument = Encodings | FileOptions;

export interface WriteFileOptions extends FileOptions {
  mode?: number;
}

export function isFileOptions(
  fileOptions: string | WriteFileOptions | undefined,
): fileOptions is FileOptions {
  if (!fileOptions) return false;

  return (
    (fileOptions as FileOptions).encoding != undefined ||
    (fileOptions as FileOptions).flag != undefined ||
    (fileOptions as FileOptions).signal != undefined ||
    (fileOptions as WriteFileOptions).mode != undefined
  );
}

export function getValidatedEncoding(
  optOrCallback?:
    | FileOptions
    | WriteFileOptions
    | ((...args: unknown[]) => unknown)
    | Encodings
    | null,
): Encodings | null {
  const encoding = getEncoding(optOrCallback);
  if (encoding) {
    assertEncoding(encoding);
  }
  return encoding;
}

export function getEncoding(
  optOrCallback?:
    | FileOptions
    | WriteFileOptions
    // deno-lint-ignore no-explicit-any
    | ((...args: any[]) => any)
    | Encodings
    | null,
): Encodings | null {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  }

  const encoding = typeof optOrCallback === "string"
    ? optOrCallback
    : optOrCallback.encoding;
  if (!encoding) return null;
  return encoding;
}

export function getSignal(optOrCallback?: FileOptions): AbortSignal | null {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  }

  const signal = typeof optOrCallback === "object" && optOrCallback.signal
    ? optOrCallback.signal
    : null;

  return signal;
}

export function checkEncoding(encoding: Encodings | null): Encodings | null {
  if (!encoding) return null;

  encoding = StringPrototypeToLowerCase(encoding) as Encodings;
  if (ArrayPrototypeIncludes(["utf8", "hex", "base64", "ascii"], encoding)) {
    return encoding;
  }

  if (encoding === "utf-8") {
    return "utf8";
  }
  if (encoding === "binary") {
    return "binary";
    // before this was buffer, however buffer is not used in Node
    // node -e "require('fs').readFile('../world.txt', 'buffer', console.log)"
  }

  const notImplementedEncodings = ["utf16le", "latin1", "ucs2"];

  if (ArrayPrototypeIncludes(notImplementedEncodings, encoding as string)) {
    notImplemented(`"${encoding}" encoding`);
  }

  throw new Error(`The value "${encoding}" is invalid for option "encoding"`);
}

export { isUint32 as isFd } from "ext:deno_node/internal/validators.mjs";

export function maybeCallback(cb: unknown) {
  validateFunction(cb, "cb");

  return cb as CallbackWithError;
}

// Ensure that callbacks run in the global context. Only use this function
// for callbacks that are passed to the binding layer, callbacks that are
// invoked from JS already run in the proper scope.
export function makeCallback<T extends unknown[]>(
  this: unknown,
  cb?: (...args: T) => void,
) {
  validateFunction(cb, "cb");

  return (...args: T) => ReflectApply(cb!, this, args);
}
