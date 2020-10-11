// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />

declare interface DedicatedWorkerGlobalScope {
  self: DedicatedWorkerGlobalScope & typeof globalThis;
  onmessage:
    | ((
      this: DedicatedWorkerGlobalScope & typeof globalThis,
      ev: MessageEvent,
    ) => any)
    | null;
  onmessageerror:
    | ((
      this: DedicatedWorkerGlobalScope & typeof globalThis,
      ev: MessageEvent,
    ) => any)
    | null;
  onerror:
    | ((
      this: DedicatedWorkerGlobalScope & typeof globalThis,
      ev: ErrorEvent,
    ) => any)
    | null;
  name: string;
  close: () => void;
  postMessage: (message: any) => void;
  Deno: typeof Deno;
}

declare var self: DedicatedWorkerGlobalScope & typeof globalThis;
declare var onmessage:
  | ((
    this: DedicatedWorkerGlobalScope & typeof globalThis,
    ev: MessageEvent,
  ) => any)
  | null;
declare var onmessageerror:
  | ((
    this: DedicatedWorkerGlobalScope & typeof globalThis,
    ev: MessageEvent,
  ) => any)
  | null;
declare var onerror:
  | ((
    this: DedicatedWorkerGlobalScope & typeof globalThis,
    ev: ErrorEvent,
  ) => any)
  | null;
declare var close: () => void;
declare var name: string;
declare var postMessage: (message: any) => void;

/* eslint-enable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */
