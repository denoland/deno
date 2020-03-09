// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function postMessage(data: Uint8Array): void {
  sendSync("op_worker_post_message", {}, data);
}

export function close(): void {
  sendSync("op_worker_close");
}
