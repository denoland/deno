// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript(
  "ext:deno_node/internal/streams/readable.js",
);

export const Readable = mod.Readable;
export default mod.default;
