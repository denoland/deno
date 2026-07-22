// Copyright 2018-2026 the Deno authors. MIT license.
// @deno-types="./internal/buffer.d.ts"
import { core } from "ext:core/mod.js";
const __buffer = core.loadExtScript("ext:deno_node/internal/buffer.mjs");
export const {
  atob,
  Blob,
  btoa,
  Buffer,
  constants,
  File,
  INSPECT_MAX_BYTES,
  isAscii,
  isUtf8,
  kMaxLength,
  kStringMaxLength,
  resolveObjectURL,
  SlowBuffer,
  transcode,
} = __buffer;
const _default = __buffer.default;
export { _default as default };
