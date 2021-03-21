// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="deno.webgpu" />
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
  WorkerNavigator: typeof WorkerNavigator;
  navigator: WorkerNavigator;
  WorkerLocation: typeof WorkerLocation;
  location: WorkerLocation;
}

declare class WorkerNavigator {
  constructor();
  readonly gpu: GPU;
}

declare var navigator: WorkerNavigator;

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

// TODO(nayeemrmn): Move this to `op_crates/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** The absolute location of the script executed by the Worker. Such an object
 * is initialized for each worker and is available via the
 * WorkerGlobalScope.location property obtained by calling self.location. */
declare class WorkerLocation {
  constructor();
  readonly hash: string;
  readonly host: string;
  readonly hostname: string;
  readonly href: string;
  toString(): string;
  readonly origin: string;
  readonly pathname: string;
  readonly port: string;
  readonly protocol: string;
  readonly search: string;
}

// TODO(nayeemrmn): Move this to `op_crates/web` where its implementation is.
// The types there must first be split into window, worker and global types.
declare var location: WorkerLocation;
