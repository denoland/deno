// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendAsync, sendSync } from "./dispatch_json.ts";

export function createWorker(
  specifier: string,
  hasSourceCode: boolean,
  sourceCode: string,
  name?: string
): { id: number } {
  return sendSync("op_create_worker", {
    specifier,
    hasSourceCode,
    sourceCode,
    name,
  });
}

export function hostTerminateWorker(id: number): void {
  sendSync("op_host_terminate_worker", { id });
}

export function hostPostMessage(id: number, data: Uint8Array): void {
  sendSync("op_host_post_message", { id }, data);
}

export function hostGetMessage(id: number): Promise<any> {
  return sendAsync("op_host_get_message", { id });
}
