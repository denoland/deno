// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const mod = core.loadExtScript("ext:deno_node/domain.ts");

export const { _stack, create, active, createDomain, Domain } = mod;

export default mod.default;
