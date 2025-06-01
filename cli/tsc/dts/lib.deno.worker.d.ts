// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />
/// <reference lib="deno.cache" />

/**
 * Event map for WorkerGlobalScope event handlers.
 *
 * @category Workers
 */
declare interface WorkerGlobalScopeEventMap {
  "error": ErrorEvent;
  "unhandledrejection": PromiseRejectionEvent;
}

/**
 * The WorkerGlobalScope interface represents the global execution context of a worker.
 * It serves as the base interface for specific worker contexts like DedicatedWorkerGlobalScope.
 *
 * @example
 * ```ts
 * // Inside a worker script
 * console.log(self.location.href);  // Logs the worker's location URL
 *
 * self.addEventListener("error", (event) => {
 *   console.error("Error in worker:", event.message);
 * });
 * ```
 *
 * @category Workers
 */
declare interface WorkerGlobalScope extends EventTarget {
  /** The location of the worker's script, represented by a WorkerLocation object. */
  readonly location: WorkerLocation;

  /** The navigator object for the worker context. */
  readonly navigator: WorkerNavigator;

  /** Event handler for error events that occur in the worker. */
  onerror: ((this: WorkerGlobalScope, ev: ErrorEvent) => any) | null;

  /**
   * Event handler for unhandled promise rejections in the worker.
   */
  onunhandledrejection:
    | ((this: WorkerGlobalScope, ev: PromiseRejectionEvent) => any)
    | null;

  /** Reference to the worker's global scope, which is the worker itself. */
  readonly self: WorkerGlobalScope & typeof globalThis;

  /** Adds an event listener for events of a specific type on the worker. */
  addEventListener<K extends keyof WorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: WorkerGlobalScope,
      ev: WorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /** Adds an event listener for events of a specific type on the worker. */
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /** Removes an event listener previously registered with addEventListener. */
  removeEventListener<K extends keyof WorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: WorkerGlobalScope,
      ev: WorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | EventListenerOptions,
  ): void;

  /** Removes an event listener previously registered with addEventListener. */
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;

  /** The Deno namespace containing runtime APIs. */
  Deno: typeof Deno;

  /** The cache storage object for the worker. */
  caches: CacheStorage;
}

/**
 * The WorkerGlobalScope interface constructor.
 *
 * @category Workers
 */
declare var WorkerGlobalScope: {
  readonly prototype: WorkerGlobalScope;
  new (): never;
};

/**
 * The WorkerNavigator interface represents the identity and state of the user agent
 * (browser) in a worker context.
 *
 * @example
 * ```ts
 * // Inside a worker
 * console.log(navigator.userAgent);  // Logs the user agent string
 * console.log(navigator.hardwareConcurrency);  // Logs the number of logical processors
 * ```
 *
 * @category Platform
 */
declare interface WorkerNavigator {
  /** Provides access to the WebGPU API. */
  readonly gpu: GPU;

  /** Returns the number of logical processors available to run threads on the user's computer. */
  readonly hardwareConcurrency: number;

  /** Returns the user agent string for the current browser. */
  readonly userAgent: string;

  /** Returns the preferred language of the user, as a string. */
  readonly language: string;

  /** Returns an array of strings representing the languages known to the user. */
  readonly languages: string[];
}

/**
 * The WorkerNavigator interface constructor.
 *
 * @category Platform
 */
declare var WorkerNavigator: {
  readonly prototype: WorkerNavigator;
  new (): never;
};

/**
 * The navigator object for the worker context.
 *
 * @category Platform
 */
declare var navigator: WorkerNavigator;

/**
 * Event map for DedicatedWorkerGlobalScope event handlers.
 * Extends WorkerGlobalScopeEventMap with worker-specific events.
 *
 * @category Workers
 */
declare interface DedicatedWorkerGlobalScopeEventMap
  extends WorkerGlobalScopeEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/**
 * The DedicatedWorkerGlobalScope interface represents the global execution context of a
 * dedicated worker, which is a worker that is utilized by a single script.
 *
 * @example
 * ```ts
 * // Inside a dedicated worker
 * self.addEventListener("message", (e) => {
 *   const receivedData = e.data;
 *   console.log("Worker received:", receivedData);
 *
 *   // Process the data and send a response back
 *   const result = processData(receivedData);
 *   self.postMessage(result);
 * });
 *
 * function processData(data) {
 *   return data.toUpperCase();
 * }
 * ```
 *
 * @category Platform
 */
declare interface DedicatedWorkerGlobalScope extends WorkerGlobalScope {
  /** The name given to the worker when it was created. */
  readonly name: string;

  /** Event handler for message events received from the parent context. */
  onmessage:
    | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
    | null;

  /** Event handler for messageerror events that occur when a message cannot be deserialized. */
  onmessageerror:
    | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
    | null;

  /**
   * Closes the worker thread.
   *
   * @example
   * ```ts
   * // Inside a worker
   * self.addEventListener("message", (e) => {
   *   if (e.data === "terminate") {
   *     self.close();  // Terminates the worker
   *   }
   * });
   * ```
   */
  close(): void;

  /**
   * Sends a message to the main thread that spawned the worker.
   *
   * @example
   * ```ts
   * // Inside a worker
   * const result = { status: "complete", data: [1, 2, 3] };
   * self.postMessage(result);
   * ```
   */
  postMessage(message: any, transfer: Transferable[]): void;

  /**
   * Sends a message to the main thread that spawned the worker.
   *
   * @example
   * ```ts
   * // Inside a worker, transferring an ArrayBuffer
   * const buffer = new ArrayBuffer(1024);
   * self.postMessage({ result: "data processed", buffer }, { transfer: [buffer] });
   * ```
   */
  postMessage(message: any, options?: StructuredSerializeOptions): void;

  /** Adds an event listener for events of a specific type on the dedicated worker. */
  addEventListener<K extends keyof DedicatedWorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: DedicatedWorkerGlobalScope,
      ev: DedicatedWorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /** Adds an event listener for events of a specific type on the dedicated worker. */
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /** Removes an event listener previously registered with addEventListener. */
  removeEventListener<K extends keyof DedicatedWorkerGlobalScopeEventMap>(
    type: K,
    listener: (
      this: DedicatedWorkerGlobalScope,
      ev: DedicatedWorkerGlobalScopeEventMap[K],
    ) => any,
    options?: boolean | EventListenerOptions,
  ): void;

  /** Removes an event listener previously registered with addEventListener. */
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

/**
 * The DedicatedWorkerGlobalScope interface constructor.
 *
 * @category Platform
 */
declare var DedicatedWorkerGlobalScope: {
  readonly prototype: DedicatedWorkerGlobalScope;
  new (): never;
};

/**
 * The name given to the worker when it was created.
 *
 * @category Workers
 */
declare var name: string;

/**
 * Event handler for message events received from the parent context.
 *
 * @category Workers
 */
declare var onmessage:
  | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
  | null;

/**
 * Event handler for messageerror events that occur when a message cannot be deserialized.
 *
 * @category Workers
 */
declare var onmessageerror:
  | ((this: DedicatedWorkerGlobalScope, ev: MessageEvent) => any)
  | null;

/**
 * Closes the worker thread.
 *
 * @category Workers
 */
declare function close(): void;

/**
 * Sends a message to the main thread that spawned the worker.
 *
 * @category Workers
 */
declare function postMessage(message: any, transfer: Transferable[]): void;

/**
 * Sends a message to the main thread that spawned the worker.
 *
 * @category Workers
 */
declare function postMessage(
  message: any,
  options?: StructuredSerializeOptions,
): void;

/**
 * The navigator object for the worker context.
 *
 * @category Platform
 */
declare var navigator: WorkerNavigator;

/**
 * Event handler for error events that occur in the worker.
 *
 * @category Platform
 */
declare var onerror:
  | ((this: DedicatedWorkerGlobalScope, ev: ErrorEvent) => any)
  | null;

/**
 * Event handler for unhandled promise rejections in the worker.
 *
 * @category Observability
 */
declare var onunhandledrejection:
  | ((this: DedicatedWorkerGlobalScope, ev: PromiseRejectionEvent) => any)
  | null;

/**
 * Reference to the worker's global scope, which is the worker itself.
 *
 * @category Workers
 */
declare var self: WorkerGlobalScope & typeof globalThis;

/**
 * Adds an event listener for events of a specific type on the worker.
 *
 * @category Events
 */
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

/**
 * Adds an event listener for events of a specific type on the worker.
 *
 * @category Events
 */
declare function addEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | AddEventListenerOptions,
): void;

/**
 * Removes an event listener previously registered with addEventListener.
 *
 * @category Events
 */
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

/**
 * Removes an event listener previously registered with addEventListener.
 *
 * @category Events
 */
declare function removeEventListener(
  type: string,
  listener: EventListenerOrEventListenerObject,
  options?: boolean | EventListenerOptions,
): void;

// TODO(nayeemrmn): Move this to `extensions/web` where its implementation is.
// The types there must first be split into window, worker and global types.
/**
 * The absolute location of the script executed by the Worker. Such an object
 * is initialized for each worker and is available via the
 * WorkerGlobalScope.location property obtained by calling self.location.
 *
 * @category Platform
 */
declare interface WorkerLocation {
  /** The fragment identifier of the worker's URL, including the leading '#' character or an empty string if there is no fragment identifier. */
  readonly hash: string;

  /** The host and port of the worker's URL. */
  readonly host: string;

  /** The domain of the worker's URL. */
  readonly hostname: string;

  /** The complete URL of the worker script. */
  readonly href: string;

  /** Returns a string containing the serialized URL of the worker script. */
  toString(): string;

  /** The origin of the worker's URL, which is the scheme, domain, and port. */
  readonly origin: string;

  /** The path component of the worker's URL, including the leading slash. */
  readonly pathname: string;

  /** The port component of the worker's URL, or an empty string if the port is not specified. */
  readonly port: string;

  /** The protocol scheme of the worker's URL, including the trailing colon. */
  readonly protocol: string;

  /** The search (query) component of the worker's URL, including the leading '?' character or an empty string if there is no search component. */
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
/**
 * The location object for the worker script.
 *
 * @category Platform
 */
declare var location: WorkerLocation;
