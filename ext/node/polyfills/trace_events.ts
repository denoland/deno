// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/_trace_events.ts");
export const { createTracing, getEnabledCategories } = _mod;
export default _mod.default;
