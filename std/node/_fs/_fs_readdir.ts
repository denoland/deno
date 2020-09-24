import { asyncIterableToCallback } from "./_fs_watch.ts";
import Dirent from "./_fs_dirent.ts";
import { fromFileUrl } from "../../path/mod.ts";

function toDirent(val: Deno.DirEntry): Dirent {
  return new Dirent(val);
}

type readDirOptions = {
  encoding?: string;
  withFileTypes?: boolean;
};

type readDirCallback = (err: Error | undefined, files: string[]) => void;

type readDirCallbackDirent = (err: Error | undefined, files: Dirent[]) => void;

type readDirBoth = (
  err: Error | undefined,
  files: string[] | Dirent[] | Array<string | Dirent>,
) => void;

export function readdir(
  path: string | URL,
  options: { withFileTypes?: false; encoding?: string },
  callback: readDirCallback,
): void;
export function readdir(
  path: string | URL,
  options: { withFileTypes: true; encoding?: string },
  callback: readDirCallbackDirent,
): void;
export function readdir(path: string | URL, callback: readDirCallback): void;
export function readdir(
  path: string | URL,
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
  path = path instanceof URL ? fromFileUrl(path) : path;

  if (!callback) throw new Error("No callback function supplied");

  if (options?.encoding) {
    try {
      new TextDecoder(options.encoding);
    } catch (error) {
      throw new Error(
        `TypeError [ERR_INVALID_OPT_VALUE_ENCODING]: The value "${options.encoding}" is invalid for option "encoding"`,
      );
    }
  }

  try {
    asyncIterableToCallback(Deno.readDir(path), (val, done) => {
      if (typeof path !== "string") return;
      if (done) {
        callback(undefined, result);
        return;
      }
      if (options?.withFileTypes) {
        result.push(toDirent(val));
      } else result.push(decode(val.name));
    });
  } catch (error) {
    callback(error, result);
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

export function readdirSync(
  path: string | URL,
  options: { withFileTypes: true; encoding?: string },
): Dirent[];
export function readdirSync(
  path: string | URL,
  options?: { withFileTypes?: false; encoding?: string },
): string[];
export function readdirSync(
  path: string | URL,
  options?: readDirOptions,
): Array<string | Dirent> {
  const result = [];
  path = path instanceof URL ? fromFileUrl(path) : path;

  if (options?.encoding) {
    try {
      new TextDecoder(options.encoding);
    } catch (error) {
      throw new Error(
        `TypeError [ERR_INVALID_OPT_VALUE_ENCODING]: The value "${options.encoding}" is invalid for option "encoding"`,
      );
    }
  }

  for (const file of Deno.readDirSync(path)) {
    if (options?.withFileTypes) {
      result.push(toDirent(file));
    } else result.push(decode(file.name));
  }
  return result;
}
