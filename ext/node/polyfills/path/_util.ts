// Copyright the Browserify authors. MIT License.
// Ported from https://github.com/browserify/path-browserify/
// Copyright 2018-2026 the Deno authors. MIT license.
// deno-fmt-ignore-file
(function () {
  const { core, primordials } = globalThis.__bootstrap;
  const { CHAR_BACKWARD_SLASH, CHAR_DOT, CHAR_FORWARD_SLASH, CHAR_LOWERCASE_A, CHAR_LOWERCASE_Z, CHAR_UPPERCASE_A, CHAR_UPPERCASE_Z } = core.loadExtScript("ext:deno_node/path/_constants.ts");
  const { ERR_INVALID_ARG_TYPE } = core.loadExtScript(
    "ext:deno_node/internal/errors.ts",
  );
  const {
    StringPrototypeCharCodeAt,
    StringPrototypeLastIndexOf,
    StringPrototypeSlice,
  } = primordials;

  function assertPath(path: string) {
    if (typeof path !== "string") {
      throw new ERR_INVALID_ARG_TYPE("path", ["string"], path);
    }
  }

  function isPosixPathSeparator(code: number): boolean {
    return code === CHAR_FORWARD_SLASH;
  }

  function isPathSeparator(code: number): boolean {
    return isPosixPathSeparator(code) || code === CHAR_BACKWARD_SLASH;
  }

  function isWindowsDeviceRoot(code: number): boolean {
    return (
      (code >= CHAR_LOWERCASE_A && code <= CHAR_LOWERCASE_Z) ||
      (code >= CHAR_UPPERCASE_A && code <= CHAR_UPPERCASE_Z)
    );
  }

  // Resolves . and .. elements in a path with directory names
  function normalizeString(
    path: string,
    allowAboveRoot: boolean,
    separator: string,
    isPathSeparator: (code: number) => boolean,
  ): string {
    let res = "";
    let lastSegmentLength = 0;
    let lastSlash = -1;
    let dots = 0;
    let code: number | undefined;
    for (let i = 0, len = path.length; i <= len; ++i) {
      if (i < len) code = StringPrototypeCharCodeAt(path, i);
      else if (isPathSeparator(code!)) break;
      else code = CHAR_FORWARD_SLASH;

      if (isPathSeparator(code!)) {
        if (lastSlash === i - 1 || dots === 1) {
          // NOOP
        } else if (dots === 2) {
          if (
            res.length < 2 || lastSegmentLength !== 2 ||
            StringPrototypeCharCodeAt(res, res.length - 1) !== CHAR_DOT ||
            StringPrototypeCharCodeAt(res, res.length - 2) !== CHAR_DOT
          ) {
            if (res.length > 2) {
              const lastSlashIndex = res.length - lastSegmentLength - 1;
              if (lastSlashIndex === -1) {
                res = "";
                lastSegmentLength = 0;
              } else {
                res = StringPrototypeSlice(res, 0, lastSlashIndex);
                lastSegmentLength = res.length - 1 -
                  StringPrototypeLastIndexOf(res, separator);
              }
              lastSlash = i;
              dots = 0;
              continue;
            } else if (res.length !== 0) {
              res = "";
              lastSegmentLength = 0;
              lastSlash = i;
              dots = 0;
              continue;
            }
          }
          if (allowAboveRoot) {
            res += res.length > 0 ? `${separator}..` : "..";
            lastSegmentLength = 2;
          }
        } else {
          if (res.length > 0) {
            res += separator + StringPrototypeSlice(path, lastSlash + 1, i);
          } else {
            res = StringPrototypeSlice(path, lastSlash + 1, i);
          }
          lastSegmentLength = i - lastSlash - 1;
        }
        lastSlash = i;
        dots = 0;
      } else if (code === CHAR_DOT && dots !== -1) {
        ++dots;
      } else {
        dots = -1;
      }
    }
    return res;
  }

  function formatExt(ext: string | undefined): string {
    return ext ? `${ext[0] === "." ? "" : "."}${ext}` : "";
  }

  function _format(
    sep: string,
    pathObject: FormatInputPathObject,
  ): string {
    const dir: string | undefined = pathObject.dir || pathObject.root;
    const base: string = pathObject.base ||
      (pathObject.name || "") + formatExt(pathObject.ext);
    if (!dir) return base;
    if (dir === pathObject.root) return dir + base;
    return dir + sep + base;
  }

  return {
    assertPath,
    isPosixPathSeparator,
    isPathSeparator,
    isWindowsDeviceRoot,
    normalizeString,
    _format,
  };
})()
