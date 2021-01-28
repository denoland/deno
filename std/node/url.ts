// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

import {
  CHAR_BACKWARD_SLASH,
  CHAR_FORWARD_SLASH,
  CHAR_LOWERCASE_A,
  CHAR_LOWERCASE_Z,
} from "../path/_constants.ts";
import * as path from "./path.ts";
import { isWindows } from "../_util/os.ts";

const forwardSlashRegEx = /\//g;
const percentRegEx = /%/g;
const backslashRegEx = /\\/g;
const newlineRegEx = /\n/g;
const carriageReturnRegEx = /\r/g;
const tabRegEx = /\t/g;

const _url = URL;
export { _url as URL };

/**
 * Get fully resolved platform-specific file path from the given URL string/ object
 * @param path The file URL string or URL object to convert to a path
 */
export function fileURLToPath(path: string | URL): string {
  if (typeof path === "string") path = new URL(path);
  else if (!(path instanceof URL)) {
    throw new Deno.errors.InvalidData(
      "invalid argument path , must be a string or URL",
    );
  }
  if (path.protocol !== "file:") {
    throw new Deno.errors.InvalidData("invalid url scheme");
  }
  return isWindows ? getPathFromURLWin(path) : getPathFromURLPosix(path);
}

function getPathFromURLWin(url: URL): string {
  const hostname = url.hostname;
  let pathname = url.pathname;
  for (let n = 0; n < pathname.length; n++) {
    if (pathname[n] === "%") {
      const third = pathname.codePointAt(n + 2) || 0x20;
      if (
        (pathname[n + 1] === "2" && third === 102) || // 2f 2F /
        (pathname[n + 1] === "5" && third === 99)
      ) {
        // 5c 5C \
        throw new Deno.errors.InvalidData(
          "must not include encoded \\ or / characters",
        );
      }
    }
  }

  pathname = pathname.replace(forwardSlashRegEx, "\\");
  pathname = decodeURIComponent(pathname);
  if (hostname !== "") {
    // TODO(bartlomieju): add support for punycode encodings
    return `\\\\${hostname}${pathname}`;
  } else {
    // Otherwise, it's a local path that requires a drive letter
    const letter = pathname.codePointAt(1)! | 0x20;
    const sep = pathname[2];
    if (
      letter < CHAR_LOWERCASE_A ||
      letter > CHAR_LOWERCASE_Z || // a..z A..Z
      sep !== ":"
    ) {
      throw new Deno.errors.InvalidData("file url path must be absolute");
    }
    return pathname.slice(1);
  }
}

function getPathFromURLPosix(url: URL): string {
  if (url.hostname !== "") {
    throw new Deno.errors.InvalidData("invalid file url hostname");
  }
  const pathname = url.pathname;
  for (let n = 0; n < pathname.length; n++) {
    if (pathname[n] === "%") {
      const third = pathname.codePointAt(n + 2) || 0x20;
      if (pathname[n + 1] === "2" && third === 102) {
        throw new Deno.errors.InvalidData(
          "must not include encoded / characters",
        );
      }
    }
  }
  return decodeURIComponent(pathname);
}

/** Get fully resolved platform-specific File URL from the given file path */
export function pathToFileURL(filepath: string): URL {
  let resolved = path.resolve(filepath);
  // path.resolve strips trailing slashes so we must add them back
  const filePathLast = filepath.charCodeAt(filepath.length - 1);
  if (
    (filePathLast === CHAR_FORWARD_SLASH ||
      (isWindows && filePathLast === CHAR_BACKWARD_SLASH)) &&
    resolved[resolved.length - 1] !== path.sep
  ) {
    resolved += "/";
  }
  const outURL = new URL("file://");
  if (resolved.includes("%")) resolved = resolved.replace(percentRegEx, "%25");
  // In posix, "/" is a valid character in paths
  if (!isWindows && resolved.includes("\\")) {
    resolved = resolved.replace(backslashRegEx, "%5C");
  }
  if (resolved.includes("\n")) resolved = resolved.replace(newlineRegEx, "%0A");
  if (resolved.includes("\r")) {
    resolved = resolved.replace(carriageReturnRegEx, "%0D");
  }
  if (resolved.includes("\t")) resolved = resolved.replace(tabRegEx, "%09");
  outURL.pathname = resolved;
  return outURL;
}

export default {
  fileURLToPath,
  pathToFileURL,
  URL,
};
