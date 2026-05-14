// Copyright the Browserify authors. MIT License.
// Ported mostly from https://github.com/browserify/path-browserify/
// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core } = globalThis.__bootstrap;
const { isWindows } = core.loadExtScript("ext:deno_node/_util/os.ts");
const _win32 = core.loadExtScript("ext:deno_node/path/_win32.ts").default;
const _posix = core.loadExtScript("ext:deno_node/path/_posix.ts").default;

const win32 = {
  ..._win32,
  win32: null,
  posix: null,
};

const posix = {
  ..._posix,
  win32: null,
  posix: null,
};

posix.win32 = win32.win32 = win32;
posix.posix = win32.posix = posix;

const path = isWindows ? win32 : posix;
const {
  basename,
  delimiter,
  dirname,
  extname,
  format,
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
  sep,
  toNamespacedPath,
  _makeLong,
  matchesGlob,
} = path;
const { common } = core.loadExtScript("ext:deno_node/path/common.ts");

return {
  win32,
  posix,
  basename,
  delimiter,
  dirname,
  extname,
  format,
  isAbsolute,
  join,
  normalize,
  parse,
  relative,
  resolve,
  sep,
  toNamespacedPath,
  _makeLong,
  matchesGlob,
  common,
  default: path,
};
})();
