// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { sendAsync } from "./dispatch_json.ts";

interface FetchRequest {
  url: string;
  method: string | null;
  headers: Array<[string, string]>;
}

export interface FetchResponse {
  bodyRid: number;
  status: number;
  statusText: string;
  headers: Array<[string, string]>;
}

export function fetch(
  args: FetchRequest,
  body: ArrayBufferView | undefined
): Promise<FetchResponse> {
  let zeroCopy = undefined;
  if (body) {
    zeroCopy = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
  }

  return sendAsync("op_fetch", args, ...(zeroCopy ? [zeroCopy] : []));
}
