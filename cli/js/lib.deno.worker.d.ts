// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />

declare interface DedicatedWorkerGlobalScope {
  self: DedicatedWorkerGlobalScope & typeof globalThis;
  onmessage: (e: MessageEvent) => void;
  onmessageerror: (e: MessageEvent) => void;
  onerror: undefined | typeof onerror;
  name: typeof __workerMain.name;
  close: typeof __workerMain.close;
  postMessage: typeof __workerMain.postMessage;
  Deno: typeof Deno;
}

declare const self: DedicatedWorkerGlobalScope & typeof globalThis;
declare let onmessage: ((e: { data: any }) => Promise<void> | void) | undefined;
declare let onerror:
  | ((
    msg: string,
    source: string,
    lineno: number,
    colno: number,
    e: Event,
  ) => boolean | void)
  | undefined;
declare const close: typeof __workerMain.close;
declare const name: typeof __workerMain.name;
declare const postMessage: typeof __workerMain.postMessage;

declare namespace __workerMain {
  export let onmessage: (e: { data: any }) => void;
  export function postMessage(data: any): void;
  export function close(): void;
  export const name: string;
}

/* eslint-enable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */
