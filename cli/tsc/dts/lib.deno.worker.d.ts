// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />
/// <reference lib="deno.cache" />

/** @category Web Workers */
declare interface WorkerGlobalScopeEventMap {
  "error": ErrorEvent;
  "unhandledrejection": PromiseRejectionEvent;
}

/** @category Web Workers */
declare interface WorkerGlobalScope extends EventTarget {
  readonly location: WorkerLocation;
  readonly navigator: WorkerNavigator;
  onerror: ((this: WorkerGlobalScope, ev: ErrorEvent) => any) | null;
  onunhandledrejection:
    | ((this: WorkerGlobalScope, ev: PromiseRejectionEvent) => any)
    | null;

  readonly self: WorkerGlobalScope & typeof globalThis;

  addEventListener<K extends keyof WorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: WorkerGlobalScope,
      ev: WorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof WorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: WorkerGlobalScope,
      ev: WorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;

  Deno: typeof Deno;
  caches: CacheStorage;
}

/** @category Web Workers */
declare var WorkerGlobalScope: {
  readonly prototype: WorkerGlobalScope;
  new (): never;
};

/** @category Web APIs */
declare interface WorkerNavigator {
  readonly gpu: GPU;
  readonly hardwareConcurrency: number;
  readonly userAgent: string;
  readonly language: string;
  readonly languages: string[];
}

/** @category Web APIs */
declare var WorkerNavigator: {
  readonly prototype: WorkerNavigator;
  new (): never;
};

/** @category Web APIs */
declare var navigator: WorkerNavigator;

/** @category Web Workers */
declare interface DedicatedWorkerGlobalScopeEventMap
  extends WorkerGlobalScopeEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/** @category Web APIs */
declare interface DedicatedWorkerGlobalScope extends WorkerGlobalScope {
  readonly name: string;
  onmessage:
    | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
    | null;
  onmessageerror:
    | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
    | null;
  close(): void;
  postMessage(message: any, transfer: Transferable[]): void;
  postMessage(message: any, options?: StructuredSerializeOptions): void;
  addEventListener<K extends keyof DedicatedWorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: DedicatedWorkerGlobalScope,
      ev: DedicatedWorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof DedicatedWorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: DedicatedWorkerGlobalScope,
      ev: DedicatedWorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/** @category Web APIs */
declare var DedicatedWorkerGlobalScope: {
  readonly prototype: DedicatedWorkerGlobalScope;
  new (): never;
};

/** @category Web Workers */
declare var name: string;
/** @category Web Workers */
declare var onmessage:
  | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
  | null;
/** @category Web Workers */
declare var onmessageerror:
  | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
  | null;
/** @category Web Workers */
declare function close(): void;
/** @category Web Workers */
declare function postMessage(message: any, transfer: Transferable[]): void;
/** @category Web Workers */
declare function postMessage(
  message: any,
  options?: StructuredSerializeOptions,
): void;
/** @category Web APIs */
declare var navigator: WorkerNavigator;
/** @category Web APIs */
declare var onerror:
  | ((this: DedicatedWorkerGlobalScope, ev: ErrorEvent) => any)
  | null;
/** @category Observability */
declare var onunhandledrejection:
  | ((this: DedicatedWorkerGlobalScope, ev: PromiseRejectionEvent) => any)
  | null;
/** @category Web Workers */
declare var self: WorkerGlobalScope & typeof globalThis;
/** @category DOM Events */
declare function addEventListener<
  K extends keyof DedicatedWorkerGlobalScopeEventMap,
>(
  type: K,
  listener: (
    this: DedicatedWorkerGlobalScope,
    ev: DedicatedWorkerGlobalScopeEventMap[K],
  ) => any,
  options?: boolean | AddEventListenerOptions,
): void;
/** @category DOM Events */
declare function addEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions,
): void;
/** @category DOM Events */
declare function removeEventListener<
  K extends keyof DedicatedWorkerGlobalScopeEventMap,
>(
  type: K,
  listener: (
    this: DedicatedWorkerGlobalScope,
    ev: DedicatedWorkerGlobalScopeEventMap[K],
  ) => any,
  options?: boolean | EventListenerOptions,
): void;
/** @category DOM Events */
declare function removeEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | EventListenerOptions,
): void;

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** The absolute location of the script executed by the Worker. Such an object
 * is initialized for each worker and is available via the
 * WorkerGlobalScope.location property obtained by calling self.location.
 *
 * @category Web APIs
 */
declare interface WorkerLocation {
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

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** The absolute location of the script executed by the Worker. Such an object
 * is initialized for each worker and is available via the
 * WorkerGlobalScope.location property obtained by calling self.location.
 *
 * @category Web APIs
 */
declare var WorkerLocation: {
  readonly prototype: WorkerLocation;
  new (): never;
};

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** @category Web APIs */
declare var location: WorkerLocation;
