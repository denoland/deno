// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-explicit-any */
import { sendAsync, sendSync } from "./dispatch_json.ts";

interface CreateWorkerResponse {
  id: number;
}

export function createWorker(args: {
  specifier: string;
  hasSourceCode: boolean;
  sourceCode: string;
  useDenoNamespace: boolean;
  name?: string;
  importMap?: string;
}): CreateWorkerResponse {
  return sendSync("op_create_worker", args);
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
