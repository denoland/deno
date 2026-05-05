// Copyright 2018-2026 the Deno authors. MIT license.
import { core } from "ext:core/mod.js";
const _mod = core.loadExtScript("ext:deno_node/_diagnostics_channel.js");
export const {
  channel,
  subscribe,
  unsubscribe,
  hasSubscribers,
  tracingChannel,
  Channel,
} = _mod;
export default _mod.default;
