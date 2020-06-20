// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { core } from "../core.ts";

export function postMessage(data: Uint8Array): void {
  core.dispatchJson.sendSync("op_worker_post_message", {}, data);
}

export function close(): void {
  core.dispatchJson.sendSync("op_worker_close");
}
