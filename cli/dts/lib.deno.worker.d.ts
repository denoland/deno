// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />

declare class WorkerGlobalScope {
  new(): WorkerGlobalScope;
  self: WorkerGlobalScope & typeof globalThis;
  onmessage:
    | ((
      this: WorkerGlobalScope & typeof globalThis,
      ev: MessageEvent,
    ) => any)
    | null;
  onmessageerror:
    | ((
      this: WorkerGlobalScope & typeof globalThis,
      ev: MessageEvent,
    ) => any)
    | null;
  onerror:
    | ((
      this: WorkerGlobalScope & typeof globalThis,
      ev: ErrorEvent,
    ) => any)
    | null;
  close: () => void;
  postMessage: (message: any) => void;
  Deno: typeof Deno;
}

declare class DedicatedWorkerGlobalScope extends WorkerGlobalScope {
  new(): DedicatedWorkerGlobalScope;
  name: string;
}

declare var self: WorkerGlobalScope & typeof globalThis;
declare var onmessage:
  | ((
    this: WorkerGlobalScope & typeof globalThis,
    ev: MessageEvent,
  ) => any)
  | null;
declare var onmessageerror:
  | ((
    this: WorkerGlobalScope & typeof globalThis,
    ev: MessageEvent,
  ) => any)
  | null;
declare var onerror:
  | ((
    this: WorkerGlobalScope & typeof globalThis,
    ev: ErrorEvent,
  ) => any)
  | null;
declare var close: () => void;
declare var name: string;
declare var postMessage: (message: any) => void;
