// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendSync } from "./dispatch_json.ts";

export function createHash(algorithm: string): number {
  return sendSync("op_create_hash", { algorithm });
}

export function updateHash(rid: number, buffer: Uint8Array): void {
  sendSync("op_update_hash", { rid }, buffer);
}

export function digestHash(rid: number): Uint8Array {
  return sendSync("op_digest_hash", { rid });
}
