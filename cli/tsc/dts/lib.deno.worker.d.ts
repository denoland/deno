// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />
/// <reference lib="deno.cache" />

/** @category Workers */
declare interface WorkerGlobalScopeEventMap {
  "error": ErrorEvent;
  "unhandledrejection": PromiseRejectionEvent;
}

/** @category Workers */
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

/** @category Workers */
declare var WorkerGlobalScope: {
  readonly prototype: WorkerGlobalScope;
  new (): never;
};

/** @category Platform */
declare interface WorkerNavigator {
  readonly gpu: GPU;
  readonly hardwareConcurrency: number;
  readonly userAgent: string;
  readonly language: string;
  readonly languages: string[];
}

/** @category Platform */
declare var WorkerNavigator: {
  readonly prototype: WorkerNavigator;
  new (): never;
};

/** @category Platform */
declare var navigator: WorkerNavigator;

/** @category Workers */
declare interface DedicatedWorkerGlobalScopeEventMap
  extends WorkerGlobalScopeEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/** @category Platform */
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

/** @category Platform */
declare var DedicatedWorkerGlobalScope: {
  readonly prototype: DedicatedWorkerGlobalScope;
  new (): never;
};

/** @category Workers */
declare var name: string;
/** @category Workers */
declare var onmessage:
  | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
  | null;
/** @category Workers */
declare var onmessageerror:
  | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
  | null;
/** @category Workers */
declare function close(): void;
/** @category Workers */
declare function postMessage(message: any, transfer: Transferable[]): void;
/** @category Workers */
declare function postMessage(
  message: any,
  options?: StructuredSerializeOptions,
): void;
/** @category Platform */
declare var navigator: WorkerNavigator;
/** @category Platform */
declare var onerror:
  | ((this: DedicatedWorkerGlobalScope, ev: ErrorEvent) => any)
  | null;
/** @category Observability */
declare var onunhandledrejection:
  | ((this: DedicatedWorkerGlobalScope, ev: PromiseRejectionEvent) => any)
  | null;
/** @category Workers */
declare var self: WorkerGlobalScope & typeof globalThis;
/** @category Events */
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
/** @category Events */
declare function addEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions,
): void;
/** @category Events */
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
/** @category Events */
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
 * @category Platform
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
 * @category Platform
 */
declare var WorkerLocation: {
  readonly prototype: WorkerLocation;
  new (): never;
};

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/** @category Platform */
declare var location: WorkerLocation;
