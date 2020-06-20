// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
/* eslint-disable @typescript-eslint/no-explicit-any */
import { core } from "../core.ts";

export function createWorker(
  specifier: string,
  hasSourceCode: boolean,
  sourceCode: string,
  useDenoNamespace: boolean,
  name?: string
): { id: number } {
  return core.dispatchJson.sendSync("op_create_worker", {
    specifier,
    hasSourceCode,
    sourceCode,
    name,
    useDenoNamespace,
  });
}

export function hostTerminateWorker(id: number): void {
  core.dispatchJson.sendSync("op_host_terminate_worker", { id });
}

export function hostPostMessage(id: number, data: Uint8Array): void {
  core.dispatchJson.sendSync("op_host_post_message", { id }, data);
}

export function hostGetMessage(id: number): Promise<any> {
  return core.dispatchJson.sendAsync("op_host_get_message", { id });
}
