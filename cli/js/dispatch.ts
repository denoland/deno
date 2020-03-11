// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as minimal from "./ops/dispatch_minimal.ts";
import * as json from "./ops/dispatch_json.ts";
import { AsyncHandler } from "./plugins.ts";

const PLUGIN_ASYNC_HANDLER_MAP: Map<number, AsyncHandler> = new Map();

export function setPluginAsyncHandler(
  opId: number,
  handler: AsyncHandler
): void {
  PLUGIN_ASYNC_HANDLER_MAP.set(opId, handler);
}

export function getAsyncHandler(opName: string): (msg: Uint8Array) => void {
  switch (opName) {
    case "op_write":
    case "op_read":
      return minimal.asyncMsgFromRust;
    default:
      return json.asyncMsgFromRust;
  }
}
