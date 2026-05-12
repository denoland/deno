// Copyright 2018-2026 the Deno authors. MIT license.
// @deno-types="./internal/buffer.d.ts"
import { core } from "ext:core/mod.js";
const __buffer = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
export const {
  atob,
  btoa,
  Buffer,
  constants,
  INSPECT_MAX_BYTES,
  isAscii,
  isUtf8,
  kMaxLength,
  kStringMaxLength,
  resolveObjectURL,
  SlowBuffer,
  transcode,
} = __buffer;

// Blob/File: deferred via Proxy so 09_file.js (which pulls 06_streams.js) does
// NOT load at module-eval time. The proxy forwards on first construct/get,
// and after the underlying class is resolved the lazy slot on `__buffer`
// is replaced with the real value (zero indirection on subsequent reads).
function makeLazyClassProxy(getKey: "Blob" | "File"): any {
  // deno-lint-ignore no-explicit-any
  const target: any = function () {};
  return new Proxy(target, {
    construct(_t, args, newTarget) {
      return Reflect.construct(__buffer[getKey], args, newTarget);
    },
    get(_t, prop) {
      return __buffer[getKey][prop];
    },
    getPrototypeOf() {
      return Reflect.getPrototypeOf(__buffer[getKey]);
    },
    has(_t, prop) {
      return prop in __buffer[getKey];
    },
  });
}

export const Blob: any = makeLazyClassProxy("Blob");
export const File: any = makeLazyClassProxy("File");
const _default = __buffer.default;
export { _default as default };
