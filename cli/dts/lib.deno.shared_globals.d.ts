// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any, no-var */

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="deno.web" />
/// <reference lib="deno.fetch" />

declare namespace WebAssembly {
  export class CompileError {
    constructor();
  }

  export class Global {
    constructor(descriptor: GlobalDescriptor, v?: any);

    value: any;
    valueOf(): any;
  }

  export class Instance {
    constructor(module: Module, importObject?: Imports);
    readonly exports: Exports;
  }

  export class LinkError {
    constructor();
  }

  export class Memory {
    constructor(descriptor: MemoryDescriptor);
    readonly buffer: ArrayBuffer;
    grow(delta: number): number;
  }

  export class Module {
    constructor(bytes: BufferSource);
    static customSections(
      moduleObject: Module,
      sectionName: string,
    ): ArrayBuffer[];
    static exports(moduleObject: Module): ModuleExportDescriptor[];
    static imports(moduleObject: Module): ModuleImportDescriptor[];
  }

  export class RuntimeError {
    constructor();
  }

  export class Table {
    constructor(descriptor: TableDescriptor);
    readonly length: number;
    get(index: number): Function | null;
    grow(delta: number): number;
    set(index: number, value: Function | null): void;
  }

  export interface GlobalDescriptor {
    mutable?: boolean;
    value: ValueType;
  }

  export interface MemoryDescriptor {
    initial: number;
    maximum?: number;
  }

  export interface ModuleExportDescriptor {
    kind: ImportExportKind;
    name: string;
  }

  export interface ModuleImportDescriptor {
    kind: ImportExportKind;
    module: string;
    name: string;
  }

  export interface TableDescriptor {
    element: TableKind;
    initial: number;
    maximum?: number;
  }

  export interface WebAssemblyInstantiatedSource {
    instance: Instance;
    module: Module;
  }

  export type ImportExportKind = "function" | "global" | "memory" | "table";
  export type TableKind = "anyfunc";
  export type ValueType = "f32" | "f64" | "i32" | "i64";
  export type ExportValue = Function | Global | Memory | Table;
  export type Exports = Record<string, ExportValue>;
  export type ImportValue = ExportValue | number;
  export type ModuleImports = Record<string, ImportValue>;
  export type Imports = Record<string, ModuleImports>;
  export function compile(bytes: BufferSource): Promise<Module>;
  export function compileStreaming(
    source: Response | Promise<Response>,
  ): Promise<Module>;
  export function instantiate(
    bytes: BufferSource,
    importObject?: Imports,
  ): Promise<WebAssemblyInstantiatedSource>;
  export function instantiate(
    moduleObject: Module,
    importObject?: Imports,
  ): Promise<Instance>;
  export function instantiateStreaming(
    response: Response | PromiseLike<Response>,
    importObject?: Imports,
  ): Promise<WebAssemblyInstantiatedSource>;
  export function validate(bytes: BufferSource): boolean;
}

/** Sets a timer which executes a function once after the timer expires. Returns
 * an id which may be used to cancel the timeout.
 *
 *     setTimeout(() => { console.log('hello'); }, 500);
 */
declare function setTimeout(
  /** callback function to execute when timer expires */
  cb: (...args: any[]) => void,
  /** delay in ms */
  delay?: number,
  /** arguments passed to callback function */
  ...args: any[]
): number;

/** Repeatedly calls a function , with a fixed time delay between each call.
 *
 *     // Outputs 'hello' to the console every 500ms
 *     setInterval(() => { console.log('hello'); }, 500);
 */
declare function setInterval(
  /** callback function to execute when timer expires */
  cb: (...args: any[]) => void,
  /** delay in ms */
  delay?: number,
  /** arguments passed to callback function */
  ...args: any[]
): number;

/** Cancels a timed, repeating action which was previously started by a call
 * to `setInterval()`
 *
 *     const id = setInterval(()= > {console.log('hello');}, 500);
 *     ...
 *     clearInterval(id);
 */
declare function clearInterval(id?: number): void;

/** Cancels a scheduled action initiated by `setTimeout()`
 *
 *     const id = setTimeout(()= > {console.log('hello');}, 500);
 *     ...
 *     clearTimeout(id);
 */
declare function clearTimeout(id?: number): void;

interface VoidFunction {
  (): void;
}

/** A microtask is a short function which is executed after the function or
 * module which created it exits and only if the JavaScript execution stack is
 * empty, but before returning control to the event loop being used to drive the
 * script's execution environment. This event loop may be either the main event
 * loop or the event loop driving a web worker.
 *
 *     queueMicrotask(() => { console.log('This event loop stack is complete'); });
 */
declare function queueMicrotask(func: VoidFunction): void;

declare var crypto: Crypto;

/** Registers an event listener in the global scope, which will be called
 * synchronously whenever the event `type` is dispatched.
 *
 *     addEventListener('unload', () => { console.log('All finished!'); });
 *     ...
 *     dispatchEvent(new Event('unload'));
 */
declare function addEventListener(
  type: string,
  callback: EventListenerOrEventListenerObject | null,
  options?: boolean | AddEventListenerOptions | undefined,
): void;

/** Dispatches an event in the global scope, synchronously invoking any
 * registered event listeners for this event in the appropriate order. Returns
 * false if event is cancelable and at least one of the event handlers which
 * handled this event called Event.preventDefault(). Otherwise it returns true.
 *
 *     dispatchEvent(new Event('unload'));
 */
declare function dispatchEvent(event: Event): boolean;

/** Remove a previously registered event listener from the global scope
 *
 *     const lstnr = () => { console.log('hello'); };
 *     addEventListener('load', lstnr);
 *     removeEventListener('load', lstnr);
 */
declare function removeEventListener(
  type: string,
  callback: EventListenerOrEventListenerObject | null,
  options?: boolean | EventListenerOptions | undefined,
): void;

interface DOMStringList {
  /** Returns the number of strings in strings. */
  readonly length: number;
  /** Returns true if strings contains string, and false otherwise. */
  contains(string: string): boolean;
  /** Returns the string with index index from strings. */
  item(index: number): string | null;
  [index: number]: string;
}

type BufferSource = ArrayBufferView | ArrayBuffer;

declare const isConsoleInstance: unique symbol;

declare interface Console {
  assert(condition?: boolean, ...data: any[]): void;
  clear(): void;
  count(label?: string): void;
  countReset(label?: string): void;
  debug(...data: any[]): void;
  dir(item?: any, options?: any): void;
  dirxml(...data: any[]): void;
  error(...data: any[]): void;
  group(...data: any[]): void;
  groupCollapsed(...data: any[]): void;
  groupEnd(): void;
  info(...data: any[]): void;
  log(...data: any[]): void;
  table(tabularData?: any, properties?: string[]): void;
  time(label?: string): void;
  timeEnd(label?: string): void;
  timeLog(label?: string, ...data: any[]): void;
  timeStamp(label?: string): void;
  trace(...data: any[]): void;
  warn(...data: any[]): void;
}

declare var console: Console;

declare interface Crypto {
  readonly subtle: null;
  getRandomValues<
    T extends
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | null,
  >(
    array: T,
  ): T;
}

declare class URLSearchParams {
  constructor(
    init?: string[][] | Record<string, string> | string | URLSearchParams,
  );
  static toString(): string;

  /** Appends a specified key/value pair as a new search parameter.
   *
   * ```ts
   * let searchParams = new URLSearchParams();
   * searchParams.append('name', 'first');
   * searchParams.append('name', 'second');
   * ```
   */
  append(name: string, value: string): void;

  /** Deletes the given search parameter and its associated value,
   * from the list of all search parameters.
   *
   * ```ts
   * let searchParams = new URLSearchParams([['name', 'value']]);
   * searchParams.delete('name');
   * ```
   */
  delete(name: string): void;

  /** Returns all the values associated with a given search parameter
   * as an array.
   *
   * ```ts
   * searchParams.getAll('name');
   * ```
   */
  getAll(name: string): string[];

  /** Returns the first value associated to the given search parameter.
   *
   * ```ts
   * searchParams.get('name');
   * ```
   */
  get(name: string): string | null;

  /** Returns a Boolean that indicates whether a parameter with the
   * specified name exists.
   *
   * ```ts
   * searchParams.has('name');
   * ```
   */
  has(name: string): boolean;

  /** Sets the value associated with a given search parameter to the
   * given value. If there were several matching values, this method
   * deletes the others. If the search parameter doesn't exist, this
   * method creates it.
   *
   * ```ts
   * searchParams.set('name', 'value');
   * ```
   */
  set(name: string, value: string): void;

  /** Sort all key/value pairs contained in this object in place and
   * return undefined. The sort order is according to Unicode code
   * points of the keys.
   *
   * ```ts
   * searchParams.sort();
   * ```
   */
  sort(): void;

  /** Calls a function for each element contained in this object in
   * place and return undefined. Optionally accepts an object to use
   * as this when executing callback as second argument.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * params.forEach((value, key, parent) => {
   *   console.log(value, key, parent);
   * });
   * ```
   *
   */
  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    thisArg?: any,
  ): void;

  /** Returns an iterator allowing to go through all keys contained
   * in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const key of params.keys()) {
   *   console.log(key);
   * }
   * ```
   */
  keys(): IterableIterator<string>;

  /** Returns an iterator allowing to go through all values contained
   * in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const value of params.values()) {
   *   console.log(value);
   * }
   * ```
   */
  values(): IterableIterator<string>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const [key, value] of params.entries()) {
   *   console.log(key, value);
   * }
   * ```
   */
  entries(): IterableIterator<[string, string]>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   * ```ts
   * const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   * for (const [key, value] of params) {
   *   console.log(key, value);
   * }
   * ```
   */
  [Symbol.iterator](): IterableIterator<[string, string]>;

  /** Returns a query string suitable for use in a URL.
   *
   * ```ts
   * searchParams.toString();
   * ```
   */
  toString(): string;
}

/** The URL interface represents an object providing static methods used for creating object URLs. */
declare class URL {
  constructor(url: string, base?: string | URL);
  createObjectURL(object: any): string;
  revokeObjectURL(url: string): void;

  hash: string;
  host: string;
  hostname: string;
  href: string;
  toString(): string;
  readonly origin: string;
  password: string;
  pathname: string;
  port: string;
  protocol: string;
  search: string;
  readonly searchParams: URLSearchParams;
  username: string;
  toJSON(): string;
}

interface MessageEventInit extends EventInit {
  data?: any;
  origin?: string;
  lastEventId?: string;
}

declare class MessageEvent extends Event {
  readonly data: any;
  readonly origin: string;
  readonly lastEventId: string;
  constructor(type: string, eventInitDict?: MessageEventInit);
}

interface ErrorEventInit extends EventInit {
  message?: string;
  filename?: string;
  lineno?: number;
  colno?: number;
  error?: any;
}

declare class ErrorEvent extends Event {
  readonly message: string;
  readonly filename: string;
  readonly lineno: number;
  readonly colno: number;
  readonly error: any;
  constructor(type: string, eventInitDict?: ErrorEventInit);
}

interface PostMessageOptions {
  transfer?: any[];
}

interface ProgressEventInit extends EventInit {
  lengthComputable?: boolean;
  loaded?: number;
  total?: number;
}

declare class Worker extends EventTarget {
  onerror?: (e: ErrorEvent) => void;
  onmessage?: (e: MessageEvent) => void;
  onmessageerror?: (e: MessageEvent) => void;
  constructor(
    specifier: string,
    options?: {
      type?: "classic" | "module";
      name?: string;
      /** UNSTABLE: New API. Expect many changes; most likely this
       * field will be made into an object for more granular
       * configuration of worker thread (permissions, import map, etc.).
       *
       * Set to `true` to make `Deno` namespace and all of its methods
       * available to worker thread.
       *
       * Currently worker inherits permissions from main thread (permissions
       * given using `--allow-*` flags).
       * Configurable permissions are on the roadmap to be implemented.
       *
       * Example:
       *
       * ```ts
       * // mod.ts
       * const worker = new Worker(
       *   new URL("deno_worker.ts", import.meta.url).href,
       *   { type: "module", deno: true }
       * );
       * worker.postMessage({ cmd: "readFile", fileName: "./log.txt" });
       *
       * // deno_worker.ts
       *
       *
       * self.onmessage = async function (e) {
       *     const { cmd, fileName } = e.data;
       *     if (cmd !== "readFile") {
       *         throw new Error("Invalid command");
       *     }
       *     const buf = await Deno.readFile(fileName);
       *     const fileContents = new TextDecoder().decode(buf);
       *     console.log(fileContents);
       * }
       * ```
       *
       * // log.txt
       * hello world
       * hello world 2
       *
       * // run program
       * $ deno run --allow-read mod.ts
       * hello world
       * hello world2
       *
       */
      deno?: boolean;
    },
  );
  postMessage(message: any, transfer: ArrayBuffer[]): void;
  postMessage(message: any, options?: PostMessageOptions): void;
  terminate(): void;
}

declare type PerformanceEntryList = PerformanceEntry[];

declare class Performance {
  constructor();

  /** Removes the stored timestamp with the associated name. */
  clearMarks(markName?: string): void;

  /** Removes stored timestamp with the associated name. */
  clearMeasures(measureName?: string): void;

  getEntries(): PerformanceEntryList;
  getEntriesByName(name: string, type?: string): PerformanceEntryList;
  getEntriesByType(type: string): PerformanceEntryList;

  /** Stores a timestamp with the associated name (a "mark"). */
  mark(markName: string, options?: PerformanceMarkOptions): PerformanceMark;

  /** Stores the `DOMHighResTimeStamp` duration between two marks along with the
   * associated name (a "measure"). */
  measure(
    measureName: string,
    options?: PerformanceMeasureOptions,
  ): PerformanceMeasure;
  /** Stores the `DOMHighResTimeStamp` duration between two marks along with the
   * associated name (a "measure"). */
  measure(
    measureName: string,
    startMark?: string,
    endMark?: string,
  ): PerformanceMeasure;

  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the permission flag `--allow-hrtime` return a precise value.
   *
   * ```ts
   * const t = performance.now();
   * console.log(`${t} ms since start!`);
   * ```
   */
  now(): number;
}

declare const performance: Performance;

declare interface PerformanceMarkOptions {
  /** Metadata to be included in the mark. */
  detail?: any;

  /** Timestamp to be used as the mark time. */
  startTime?: number;
}

declare interface PerformanceMeasureOptions {
  /** Metadata to be included in the measure. */
  detail?: any;

  /** Timestamp to be used as the start time or string to be used as start
   * mark.*/
  start?: string | number;

  /** Duration between the start and end times. */
  duration?: number;

  /** Timestamp to be used as the end time or string to be used as end mark. */
  end?: string | number;
}

/** Encapsulates a single performance metric that is part of the performance
 * timeline. A performance entry can be directly created by making a performance
 * mark or measure (for example by calling the `.mark()` method) at an explicit
 * point in an application. */
declare class PerformanceEntry {
  readonly duration: number;
  readonly entryType: string;
  readonly name: string;
  readonly startTime: number;
  toJSON(): any;
}

/** `PerformanceMark` is an abstract interface for `PerformanceEntry` objects
 * with an entryType of `"mark"`. Entries of this type are created by calling
 * `performance.mark()` to add a named `DOMHighResTimeStamp` (the mark) to the
 * performance timeline. */
declare class PerformanceMark extends PerformanceEntry {
  readonly detail: any;
  readonly entryType: "mark";
  constructor(name: string, options?: PerformanceMarkOptions);
}

/** `PerformanceMeasure` is an abstract interface for `PerformanceEntry` objects
 * with an entryType of `"measure"`. Entries of this type are created by calling
 * `performance.measure()` to add a named `DOMHighResTimeStamp` (the measure)
 * between two marks to the performance timeline. */
declare class PerformanceMeasure extends PerformanceEntry {
  readonly detail: any;
  readonly entryType: "measure";
}

/** Events measuring progress of an underlying process, like an HTTP request
 * (for an XMLHttpRequest, or the loading of the underlying resource of an
 * <img>, <audio>, <video>, <style> or <link>). */
declare class ProgressEvent<T extends EventTarget = EventTarget> extends Event {
  constructor(type: string, eventInitDict?: ProgressEventInit);
  readonly lengthComputable: boolean;
  readonly loaded: number;
  readonly target: T | null;
  readonly total: number;
}

declare interface CustomEventInit<T = any> extends EventInit {
  detail?: T;
}

declare class CustomEvent<T = any> extends Event {
  constructor(typeArg: string, eventInitDict?: CustomEventInit<T>);
  /** Returns any custom data event was created with. Typically used for
   * synthetic events. */
  readonly detail: T;
}

interface ErrorConstructor {
  /** See https://v8.dev/docs/stack-trace-api#stack-trace-collection-for-custom-exceptions. */
  // eslint-disable-next-line @typescript-eslint/ban-types
  captureStackTrace(error: Object, constructor?: Function): void;
  // TODO(nayeemrmn): Support `Error.prepareStackTrace()`. We currently use this
  // internally in a way that makes it unavailable for users.
}

interface CloseEventInit extends EventInit {
  code?: number;
  reason?: string;
  wasClean?: boolean;
}

declare class CloseEvent extends Event {
  constructor(type: string, eventInitDict?: CloseEventInit);
  /**
   * Returns the WebSocket connection close code provided by the server.
   */
  readonly code: number;
  /**
   * Returns the WebSocket connection close reason provided by the server.
   */
  readonly reason: string;
  /**
   * Returns true if the connection closed cleanly; false otherwise.
   */
  readonly wasClean: boolean;
}

interface WebSocketEventMap {
  close: CloseEvent;
  error: Event;
  message: MessageEvent;
  open: Event;
}

/** Provides the API for creating and managing a WebSocket connection to a server, as well as for sending and receiving data on the connection. */
declare class WebSocket extends EventTarget {
  constructor(url: string, protocols?: string | string[]);

  static readonly CLOSED: number;
  static readonly CLOSING: number;
  static readonly CONNECTING: number;
  static readonly OPEN: number;

  /**
   * Returns a string that indicates how binary data from the WebSocket object is exposed to scripts:
   *
   * Can be set, to change how binary data is returned. The default is "blob".
   */
  binaryType: BinaryType;
  /**
   * Returns the number of bytes of application data (UTF-8 text and binary data) that have been queued using send() but not yet been transmitted to the network.
   *
   * If the WebSocket connection is closed, this attribute's value will only increase with each call to the send() method. (The number does not reset to zero once the connection closes.)
   */
  readonly bufferedAmount: number;
  /**
   * Returns the extensions selected by the server, if any.
   */
  readonly extensions: string;
  onclose: ((this: WebSocket, ev: CloseEvent) => any) | null;
  onerror: ((this: WebSocket, ev: Event | ErrorEvent) => any) | null;
  onmessage: ((this: WebSocket, ev: MessageEvent) => any) | null;
  onopen: ((this: WebSocket, ev: Event) => any) | null;
  /**
   * Returns the subprotocol selected by the server, if any. It can be used in conjunction with the array form of the constructor's second argument to perform subprotocol negotiation.
   */
  readonly protocol: string;
  /**
   * Returns the state of the WebSocket object's connection. It can have the values described below.
   */
  readonly readyState: number;
  /**
   * Returns the URL that was used to establish the WebSocket connection.
   */
  readonly url: string;
  /**
   * Closes the WebSocket connection, optionally using code as the the WebSocket connection close code and reason as the the WebSocket connection close reason.
   */
  close(code?: number, reason?: string): void;
  /**
   * Transmits data using the WebSocket connection. data can be a string, a Blob, an ArrayBuffer, or an ArrayBufferView.
   */
  send(data: string | ArrayBufferLike | Blob | ArrayBufferView): void;
  readonly CLOSED: number;
  readonly CLOSING: number;
  readonly CONNECTING: number;
  readonly OPEN: number;
  addEventListener<K extends keyof WebSocketEventMap>(
    type: K,
    listener: (this: WebSocket, ev: WebSocketEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;
  removeEventListener<K extends keyof WebSocketEventMap>(
    type: K,
    listener: (this: WebSocket, ev: WebSocketEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;
}

type BinaryType = "arraybuffer" | "blob";
