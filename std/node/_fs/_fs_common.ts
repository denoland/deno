// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { notImplemented } from "../_utils.ts";

export type CallbackWithError = (err?: Error | null) => void;

export interface FileOptions {
  encoding?: string;
  flag?: string;
}

export interface WriteFileOptions extends FileOptions {
  mode?: number;
}

export function isFileOptions(
  fileOptions: string | WriteFileOptions | undefined
): fileOptions is FileOptions {
  if (!fileOptions) return false;

  return (
    (fileOptions as FileOptions).encoding != undefined ||
    (fileOptions as FileOptions).flag != undefined ||
    (fileOptions as WriteFileOptions).mode != undefined
  );
}

export function getEncoding(
  optOrCallback?: FileOptions | WriteFileOptions | Function | string
): string | null {
  if (!optOrCallback || typeof optOrCallback === "function") {
    return null;
  }

  const encoding =
    typeof optOrCallback === "string" ? optOrCallback : optOrCallback.encoding;
  if (!encoding) return null;
  return encoding;
}

export function checkEncoding(encoding: string | null): string | null {
  if (!encoding) return null;
  if (encoding === "utf8" || encoding === "utf-8") {
    return "utf8";
  }
  if (encoding === "buffer") {
    return "buffer";
  }

  const notImplementedEncodings = [
    "utf16le",
    "latin1",
    "base64",
    "hex",
    "ascii",
    "binary",
    "ucs2",
  ];

  if (notImplementedEncodings.includes(encoding)) {
    notImplemented(`"${encoding}" encoding`);
  }

  throw new Error(`The value "${encoding}" is invalid for option "encoding"`);
}

export function getOpenOptions(flag: string | undefined): Deno.OpenOptions {
  if (!flag) {
    return { create: true, append: true };
  }

  let openOptions: Deno.OpenOptions;
  switch (flag) {
    case "a": {
      // 'a': Open file for appending. The file is created if it does not exist.
      openOptions = { create: true, append: true };
      break;
    }
    case "ax": {
      // 'ax': Like 'a' but fails if the path exists.
      openOptions = { createNew: true, write: true, append: true };
      break;
    }
    case "a+": {
      // 'a+': Open file for reading and appending. The file is created if it does not exist.
      openOptions = { read: true, create: true, append: true };
      break;
    }
    case "ax+": {
      // 'ax+': Like 'a+' but fails if the path exists.
      openOptions = { read: true, createNew: true, append: true };
      break;
    }
    case "r": {
      // 'r': Open file for reading. An exception occurs if the file does not exist.
      openOptions = { read: true };
      break;
    }
    case "r+": {
      // 'r+': Open file for reading and writing. An exception occurs if the file does not exist.
      openOptions = { read: true, write: true };
      break;
    }
    case "w": {
      // 'w': Open file for writing. The file is created (if it does not exist) or truncated (if it exists).
      openOptions = { create: true, write: true, truncate: true };
      break;
    }
    case "wx": {
      // 'wx': Like 'w' but fails if the path exists.
      openOptions = { createNew: true, write: true };
      break;
    }
    case "w+": {
      // 'w+': Open file for reading and writing. The file is created (if it does not exist) or truncated (if it exists).
      openOptions = { create: true, write: true, truncate: true, read: true };
      break;
    }
    case "wx+": {
      // 'wx+': Like 'w+' but fails if the path exists.
      openOptions = { createNew: true, write: true, read: true };
      break;
    }
    case "as": {
      // 'as': Open file for appending in synchronous mode. The file is created if it does not exist.
      openOptions = { create: true, append: true };
    }
    case "as+": {
      // 'as+': Open file for reading and appending in synchronous mode. The file is created if it does not exist.
      openOptions = { create: true, read: true, append: true };
    }
    case "rs+": {
      // 'rs+': Open file for reading and writing in synchronous mode. Instructs the operating system to bypass the local file system cache.
      openOptions = { create: true, read: true, write: true };
    }
    default: {
      throw new Error(`Unrecognized file system flag: ${flag}`);
    }
  }

  return openOptions;
}
