// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript(
  "ext:deno_node/internal/streams/transform.js",
);

export const Transform = mod.Transform;
export default mod.default;
