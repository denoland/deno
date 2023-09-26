// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;

import { denoNs } from "ext:runtime/90_deno_ns.js";

denoNs.jupyter = {
  async broadcast(msgType, content) {
    await core.opAsync("op_jupyter_broadcast", msgType, content);
  },
};
