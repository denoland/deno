// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

const core = globalThis.Deno.core;
const internals = globalThis.__bootstrap.internals;

import { denoNsUnstable } from "ext:runtime/90_deno_ns.js";

function enableJupyter() {
  denoNsUnstable.jupyter = {
    async broadcast(msgType, content) {
      await core.opAsync("op_jupyter_broadcast", msgType, content);
    },
  };
}

internals.enableJupyter = enableJupyter;
