// Copyright 2018-2025 the Deno authors. MIT license.

// Documentation partially adapted from [MDN](https://developer.mozilla.org/),
// by Mozilla Contributors, which is licensed under CC-BY-SA 2.5.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="deno.console" />
/// <reference lib="deno.url" />
/// <reference lib="deno.web" />
/// <reference lib="deno.webgpu" />
/// <reference lib="deno.canvas" />
/// <reference lib="deno.fetch" />
/// <reference lib="deno.websocket" />
/// <reference lib="deno.crypto" />
/// <reference lib="deno.ns" />

/** @category Wasm */
declare namespace WebAssembly {
  /**
   * The `WebAssembly.CompileError` object indicates an error during WebAssembly decoding or validation.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/CompileError)
   *
   * @category Wasm
   */
  export class CompileError extends Error {
    /** Creates a new `WebAssembly.CompileError` object. */
    constructor(message?: string, options?: ErrorOptions);
  }

  /**
   * A `WebAssembly.Global` object represents a global variable instance, accessible from
   * both JavaScript and importable/exportable across one or more `WebAssembly.Module`
   * instances. This allows dynamic linking of multiple modules.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/Global)
   *
   * @category Wasm
   */
  export class Global {
    /** Creates a new `Global` object. */
    constructor(descriptor: GlobalDescriptor, v?: any);

    /**
     * The value contained inside the global variable — this can be used to directly set
     * and get the global's value.
     */
    value: any;

    /** Old-style method that returns the value contained inside the global variable. */
    valueOf(): any;
  }

  /**
   * A `WebAssembly.Instance` object is a stateful, executable instance of a `WebAssembly.Module`.
   * Instance objects contain all the Exported WebAssembly functions that allow calling into
   * WebAssembly code from JavaScript.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/Instance)
   *
   * @category Wasm
   */
  export class Instance {
    /** Creates a new Instance object. */
    constructor(module: Module, importObject?: Imports);

    /**
     * Returns an object containing as its members all the functions exported from the
     * WebAssembly module instance, to allow them to be accessed and used by JavaScript.
     * Read-only.
     */
    readonly exports: Exports;
  }

  /**
   * The `WebAssembly.LinkError` object indicates an error during module instantiation
   * (besides traps from the start function).
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/LinkError)
   *
   * @category Wasm
   */
  export class LinkError extends Error {
    /** Creates a new WebAssembly.LinkError object. */
    constructor(message?: string, options?: ErrorOptions);
  }

  /**
   * The `WebAssembly.Memory` object is a resizable `ArrayBuffer` or `SharedArrayBuffer` that
   * holds the raw bytes of memory accessed by a WebAssembly Instance.
   *
   * A memory created by JavaScript or in WebAssembly code will be accessible and mutable
   * from both JavaScript and WebAssembly.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/Memory)
   *
   * @category Wasm
   */
  export class Memory {
    /** Creates a new `Memory` object. */
    constructor(descriptor: MemoryDescriptor);

    /** An accessor property that returns the buffer contained in the memory. */
    readonly buffer: ArrayBuffer | SharedArrayBuffer;

    /**
     * Increases the size of the memory instance by a specified number of WebAssembly
     * pages (each one is 64KB in size).
     */
    grow(delta: number): number;
  }

  /**
   * A `WebAssembly.Module` object contains stateless WebAssembly code that has already been compiled
   * by the browser — this can be efficiently shared with Workers, and instantiated multiple times.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/Module)
   *
   * @category Wasm
   */
  export class Module {
    /** Creates a new `Module` object. */
    constructor(bytes: BufferSource);

    /**
     * Given a `Module` and string, returns a copy of the contents of all custom sections in the
     * module with the given string name.
     */
    static customSections(
      moduleObject: Module,
      sectionName: string,
    ): ArrayBuffer[];

    /** Given a `Module`, returns an array containing descriptions of all the declared exports. */
    static exports(moduleObject: Module): ModuleExportDescriptor[];

    /** Given a `Module`, returns an array containing descriptions of all the declared imports. */
    static imports(moduleObject: Module): ModuleImportDescriptor[];
  }

  /**
   * The `WebAssembly.RuntimeError` object is the error type that is thrown whenever WebAssembly
   * specifies a trap.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/RuntimeError)
   *
   * @category Wasm
   */
  export class RuntimeError extends Error {
    /** Creates a new `WebAssembly.RuntimeError` object. */
    constructor(message?: string, options?: ErrorOptions);
  }

  /**
   * The `WebAssembly.Table()` object is a JavaScript wrapper object — an array-like structure
   * representing a WebAssembly Table, which stores function references. A table created by
   * JavaScript or in WebAssembly code will be accessible and mutable from both JavaScript
   * and WebAssembly.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/Table)
   *
   * @category Wasm
   */
  export class Table {
    /** Creates a new `Table` object. */
    constructor(descriptor: TableDescriptor);

    /** Returns the length of the table, i.e. the number of elements. */
    readonly length: number;

    /** Accessor function — gets the element stored at a given index. */
    get(index: number): Function | null;

    /** Increases the size of the `Table` instance by a specified number of elements. */
    grow(delta: number): number;

    /** Sets an element stored at a given index to a given value. */
    set(index: number, value: Function | null): void;
  }

  /** The `GlobalDescriptor` describes the options you can pass to
   * `new WebAssembly.Global()`.
   *
   * @category Wasm
   */
  export interface GlobalDescriptor {
    mutable?: boolean;
    value: ValueType;
  }

  /** The `MemoryDescriptor` describes the options you can pass to
   * `new WebAssembly.Memory()`.
   *
   * @category Wasm
   */
  export interface MemoryDescriptor {
    initial: number;
    maximum?: number;
    shared?: boolean;
  }

  /** A `ModuleExportDescriptor` is the description of a declared export in a
   * `WebAssembly.Module`.
   *
   * @category Wasm
   */
  export interface ModuleExportDescriptor {
    kind: ImportExportKind;
    name: string;
  }

  /** A `ModuleImportDescriptor` is the description of a declared import in a
   * `WebAssembly.Module`.
   *
   * @category Wasm
   */
  export interface ModuleImportDescriptor {
    kind: ImportExportKind;
    module: string;
    name: string;
  }

  /** The `TableDescriptor` describes the options you can pass to
   * `new WebAssembly.Table()`.
   *
   * @category Wasm
   */
  export interface TableDescriptor {
    element: TableKind;
    initial: number;
    maximum?: number;
  }

  /** The value returned from `WebAssembly.instantiate`.
   *
   * @category Wasm
   */
  export interface WebAssemblyInstantiatedSource {
    /* A `WebAssembly.Instance` object that contains all the exported WebAssembly functions. */
    instance: Instance;

    /**
     * A `WebAssembly.Module` object representing the compiled WebAssembly module.
     * This `Module` can be instantiated again, or shared via postMessage().
     */
    module: Module;
  }

  /** @category Wasm */
  export type ImportExportKind = "function" | "global" | "memory" | "table";
  /** @category Wasm */
  export type TableKind = "anyfunc";
  /** @category Wasm */
  export type ValueType = "f32" | "f64" | "i32" | "i64";
  /** @category Wasm */
  export type ExportValue = Function | Global | Memory | Table;
  /** @category Wasm */
  export type Exports = Record<string, ExportValue>;
  /** @category Wasm */
  export type ImportValue = ExportValue | number;
  /** @category Wasm */
  export type ModuleImports = Record<string, ImportValue>;
  /** @category Wasm */
  export type Imports = Record<string, ModuleImports>;

  /**
   * The `WebAssembly.compile()` function compiles WebAssembly binary code into a
   * `WebAssembly.Module` object. This function is useful if it is necessary to compile
   * a module before it can be instantiated (otherwise, the `WebAssembly.instantiate()`
   * function should be used).
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/compile)
   *
   * @category Wasm
   */
  export function compile(bytes: BufferSource): Promise<Module>;

  /**
   * The `WebAssembly.compileStreaming()` function compiles a `WebAssembly.Module`
   * directly from a streamed underlying source. This function is useful if it is
   * necessary to a compile a module before it can be instantiated (otherwise, the
   * `WebAssembly.instantiateStreaming()` function should be used).
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/compileStreaming)
   *
   * @category Wasm
   */
  export function compileStreaming(
    source: Response | Promise<Response>,
  ): Promise<Module>;

  /**
   * The WebAssembly.instantiate() function allows you to compile and instantiate
   * WebAssembly code.
   *
   * This overload takes the WebAssembly binary code, in the form of a typed
   * array or ArrayBuffer, and performs both compilation and instantiation in one step.
   * The returned Promise resolves to both a compiled WebAssembly.Module and its first
   * WebAssembly.Instance.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/instantiate)
   *
   * @category Wasm
   */
  export function instantiate(
    bytes: BufferSource,
    importObject?: Imports,
  ): Promise<WebAssemblyInstantiatedSource>;

  /**
   * The WebAssembly.instantiate() function allows you to compile and instantiate
   * WebAssembly code.
   *
   * This overload takes an already-compiled WebAssembly.Module and returns
   * a Promise that resolves to an Instance of that Module. This overload is useful
   * if the Module has already been compiled.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/instantiate)
   *
   * @category Wasm
   */
  export function instantiate(
    moduleObject: Module,
    importObject?: Imports,
  ): Promise<Instance>;

  /**
   * The `WebAssembly.instantiateStreaming()` function compiles and instantiates a
   * WebAssembly module directly from a streamed underlying source. This is the most
   * efficient, optimized way to load wasm code.
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/instantiateStreaming)
   *
   * @category Wasm
   */
  export function instantiateStreaming(
    response: Response | PromiseLike<Response>,
    importObject?: Imports,
  ): Promise<WebAssemblyInstantiatedSource>;

  /**
   * The `WebAssembly.validate()` function validates a given typed array of
   * WebAssembly binary code, returning whether the bytes form a valid wasm
   * module (`true`) or not (`false`).
   *
   * [MDN](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/WebAssembly/validate)
   *
   * @category Wasm
   */
  export function validate(bytes: BufferSource): boolean;
}

/** Sets a timer which executes a function once after the delay (in milliseconds) elapses. Returns
 * an id which may be used to cancel the timeout.
 *
 * ```ts
 * setTimeout(() => { console.log('hello'); }, 500);
 * ```
 *
 * @category Platform
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
 * ```ts
 * // Outputs 'hello' to the console every 500ms
 * setInterval(() => { console.log('hello'); }, 500);
 * ```
 *
 * @category Platform
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
 * ```ts
 * const id = setInterval(() => {console.log('hello');}, 500);
 * // ...
 * clearInterval(id);
 * ```
 *
 * @category Platform
 */
declare function clearInterval(id?: number): void;

/** Cancels a scheduled action initiated by `setTimeout()`
 *
 * ```ts
 * const id = setTimeout(() => {console.log('hello');}, 500);
 * // ...
 * clearTimeout(id);
 * ```
 *
 * @category Platform
 */
declare function clearTimeout(id?: number): void;

/** @category Platform */
interface VoidFunction {
  (): void;
}

/** A microtask is a short function which is executed after the function or
 * module which created it exits and only if the JavaScript execution stack is
 * empty, but before returning control to the event loop being used to drive the
 * script's execution environment. This event loop may be either the main event
 * loop or the event loop driving a web worker.
 *
 * ```ts
 * queueMicrotask(() => { console.log('This event loop stack is complete'); });
 * ```
 *
 * @category Platform
 */
declare function queueMicrotask(func: VoidFunction): void;

/** Dispatches an event in the global scope, synchronously invoking any
 * registered event listeners for this event in the appropriate order. Returns
 * false if event is cancelable and at least one of the event handlers which
 * handled this event called Event.preventDefault(). Otherwise it returns true.
 *
 * ```ts
 * dispatchEvent(new Event('unload'));
 * ```
 *
 * @category Events
 */
declare function dispatchEvent(event: Event): boolean;

/** @category Platform */
interface DOMStringList {
  /** Returns the number of strings in strings. */
  readonly length: number;
  /** Returns true if strings contains string, and false otherwise. */
  contains(string: string): boolean;
  /** Returns the string with index index from strings. */
  item(index: number): string | null;
  [index: number]: string;
}

/** @category Platform */
type BufferSource = ArrayBufferView | ArrayBuffer;

/**
 * A global console object that provides methods for logging, debugging, and error reporting.
 * The console object provides access to the browser's or runtime's debugging console functionality.
 * It allows developers to output text and data for debugging purposes.
 *
 * @example
 * ```typescript
 * console.log("Hello, world!");
 * console.error("An error occurred");
 * console.warn("Warning message");
 * console.debug("Debug information");
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/console
 *
 * @category I/O
 */
declare var console: Console;

/** @category Events */
interface ErrorEventInit extends EventInit {
  message?: string;
  filename?: string;
  lineno?: number;
  colno?: number;
  error?: any;
}

/** @category Events */
interface ErrorEvent extends Event {
  readonly message: string;
  readonly filename: string;
  readonly lineno: number;
  readonly colno: number;
  readonly error: any;
}

/** @category Events */
declare var ErrorEvent: {
  readonly prototype: ErrorEvent;
  new (type: string, eventInitDict?: ErrorEventInit): ErrorEvent;
};

/** @category Events */
interface PromiseRejectionEventInit extends EventInit {
  promise: Promise<any>;
  reason?: any;
}

/** @category Events */
interface PromiseRejectionEvent extends Event {
  readonly promise: Promise<any>;
  readonly reason: any;
}

/** @category Events */
declare var PromiseRejectionEvent: {
  readonly prototype: PromiseRejectionEvent;
  new (
    type: string,
    eventInitDict?: PromiseRejectionEventInit,
  ): PromiseRejectionEvent;
};

/** @category Workers */
interface AbstractWorkerEventMap {
  "error": ErrorEvent;
}

/** @category Workers */
interface WorkerEventMap extends AbstractWorkerEventMap {
  "message": MessageEvent;
  "messageerror": MessageEvent;
}

/** @category Workers */
interface WorkerOptions {
  type?: "classic" | "module";
  name?: string;
}

/**
 * The Worker interface represents a background task that can be created via the
 * `new Worker()` constructor. Workers run in a separate thread, allowing for parallel execution
 * without blocking the main thread.
 *
 * Workers can be used to:
 * - Perform CPU-intensive calculations
 * - Process large datasets
 * - Handle tasks in parallel with the main execution thread
 * - Run code in isolation with its own event loop
 *
 * @example
 * ```ts
 * // Creating a basic worker (main.ts)
 * const worker = new Worker(new URL("./worker.ts", import.meta.url).href, {
 *   type: "module"
 * });
 *
 * // Send data to the worker
 * worker.postMessage({ command: "start", data: [1, 2, 3, 4] });
 *
 * // Receive messages from the worker
 * worker.onmessage = (e) => {
 *   console.log("Result from worker:", e.data);
 *   worker.terminate(); // Stop the worker when done
 * };
 *
 * // Handle worker errors
 * worker.onerror = (e) => {
 *   console.error("Worker error:", e.message);
 * };
 * ```
 *
 * @example
 * ```ts
 * // Worker file (worker.ts)
 * // Worker context: self refers to the worker's global scope
 * self.onmessage = (e) => {
 *   if (e.data.command === "start") {
 *     // Perform calculation with the data
 *     const result = e.data.data.reduce((sum, num) => sum + num, 0);
 *     // Send result back to main thread
 *     self.postMessage(result);
 *   }
 * };
 * ```
 *
 * @category Workers
 */
interface Worker extends EventTarget {
  /** Event handler for error events. Fired when an error occurs in the worker's execution context. */
  onerror: (this: Worker, e: ErrorEvent) => any | null;

  /** Event handler for message events. Fired when the worker sends data back to the main thread. */
  onmessage: (this: Worker, e: MessageEvent) => any | null;

  /** Event handler for message error events. Fired when a message cannot be deserialized. */
  onmessageerror: (this: Worker, e: MessageEvent) => any | null;

  /**
   * Sends a message to the worker, transferring ownership of the specified transferable objects.
   *
   * @example
   * ```ts
   * // Create a buffer to transfer (not copy) to the worker
   * const buffer = new ArrayBuffer(1024);
   * worker.postMessage({ data: buffer }, [buffer]);
   * // After transfer, buffer is no longer usable in the main thread
   * ```
   */
  postMessage(message: any, transfer: Transferable[]): void;

  /**
   * Sends a message to the worker.
   *
   * @example
   * ```ts
   * // Send a simple message with data
   * worker.postMessage({
   *   command: "process",
   *   data: [1, 2, 3, 4],
   *   settings: { optimize: true }
   * });
   * ```
   */
  postMessage(message: any, options?: StructuredSerializeOptions): void;

  /** Adds an event listener to the worker. */
  addEventListener<K extends keyof WorkerEventMap>(
    type: K,
    listener: (this: Worker, ev: WorkerEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /** Adds an event listener for events whose type attribute value is type. */
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ): void;

  /** Removes an event listener from the worker. */
  removeEventListener<K extends keyof WorkerEventMap>(
    type: K,
    listener: (this: Worker, ev: WorkerEventMap[K]) => any,
    options?: boolean | EventListenerOptions,
  ): void;

  /** Removes an event listener from the worker. */
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ): void;

  /**
   * Immediately terminates the worker.
   * This does not offer the worker an opportunity to finish its operations;
   * it is stopped at once.
   *
   * @example
   * ```ts
   * // Create a worker
   * const worker = new Worker(new URL("./worker.ts", import.meta.url).href, {
   *   type: "module"
   * });
   *
   * // Some time later, when you're done with the worker
   * worker.terminate();
   * // The worker is now terminated and its resources are freed
   * ```
   */
  terminate(): void;
}

/**
 * The Worker constructor creates a new Worker object that executes code in a separate thread.
 *
 * Workers can import ES modules when created with the `type: "module"` option.
 *
 * @category Workers
 */
declare var Worker: {
  readonly prototype: Worker;

  /**
   * Creates a new Worker object.
   *
   * @param specifier - URL or file path for the worker's script.
   *                    When using a relative path, use `new URL("./worker.ts", import.meta.url)`
   *                    to ensure the path is correctly resolved relative to the current module.
   * @param options - Worker options including type and name
   *
   * @example Module worker with URL resolution
   * ```ts
   * // Create a worker that can use ES modules
   * const worker = new Worker(
   *   new URL("./workers/heavy_computation.ts", import.meta.url).href,
   *   { type: "module", name: "computation-worker" }
   * );
   * ```
   *
   * @example Worker communication pattern
   * ```ts
   * // Main thread
   * const worker = new Worker(new URL("./worker.ts", import.meta.url).href, { type: "module" });
   *
   * // Set up communication
   * worker.postMessage({ action: "start", data: [1, 2, 3, 4, 5] });
   *
   * worker.onmessage = (e) => {
   *   console.log("Worker result:", e.data);
   *   if (e.data.status === "complete") {
   *     worker.terminate();
   *   }
   * };
   *
   * // Worker file (worker.ts)
   * // self.onmessage = (e) => {
   * //   if (e.data.action === "start") {
   * //     const result = e.data.data.reduce((a, b) => a + b, 0);
   * //     self.postMessage({ status: "complete", result });
   * //   }
   * // };
   * ```
   */
  new (specifier: string | URL, options?: WorkerOptions): Worker;
};

/** @category Performance */
type PerformanceEntryList = PerformanceEntry[];

/** @category Performance */
interface Performance extends EventTarget {
  /** Returns a timestamp representing the start of the performance measurement. */
  readonly timeOrigin: number;

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

  /** Returns a current time from Deno's start in fractional milliseconds.
   *
   * ```ts
   * const t = performance.now();
   * console.log(`${t} ms since start!`);
   * ```
   */
  now(): number;

  /** Returns a JSON representation of the performance object. */
  toJSON(): any;
}

/** @category Performance */
declare var Performance: {
  readonly prototype: Performance;
  new (): never;
};

/** @category Performance */
declare var performance: Performance;

/** @category Performance */
interface PerformanceMarkOptions {
  /** Metadata to be included in the mark. */
  detail?: any;

  /** Timestamp to be used as the mark time. */
  startTime?: number;
}

/** @category Performance */
interface PerformanceMeasureOptions {
  /** Metadata to be included in the measure. */
  detail?: any;

  /** Timestamp to be used as the start time or string to be used as start
   * mark. */
  start?: string | number;

  /** Duration between the start and end times. */
  duration?: number;

  /** Timestamp to be used as the end time or string to be used as end mark. */
  end?: string | number;
}

/** Encapsulates a single performance metric that is part of the performance
 * timeline. A performance entry can be directly created by making a performance
 * mark or measure (for example by calling the `.mark()` method) at an explicit
 * point in an application.
 *
 * @category Performance
 */
interface PerformanceEntry {
  readonly duration: number;
  readonly entryType: string;
  readonly name: string;
  readonly startTime: number;
  toJSON(): any;
}

/** Encapsulates a single performance metric that is part of the performance
 * timeline. A performance entry can be directly created by making a performance
 * mark or measure (for example by calling the `.mark()` method) at an explicit
 * point in an application.
 *
 * @category Performance
 */
declare var PerformanceEntry: {
  readonly prototype: PerformanceEntry;
  new (): never;
};

/** `PerformanceMark` is an abstract interface for `PerformanceEntry` objects
 * with an entryType of `"mark"`. Entries of this type are created by calling
 * `performance.mark()` to add a named `DOMHighResTimeStamp` (the mark) to the
 * performance timeline.
 *
 * @category Performance
 */
interface PerformanceMark extends PerformanceEntry {
  readonly detail: any;
  readonly entryType: "mark";
}

/** `PerformanceMark` is an abstract interface for `PerformanceEntry` objects
 * with an entryType of `"mark"`. Entries of this type are created by calling
 * `performance.mark()` to add a named `DOMHighResTimeStamp` (the mark) to the
 * performance timeline.
 *
 * @category Performance
 */
declare var PerformanceMark: {
  readonly prototype: PerformanceMark;
  new (name: string, options?: PerformanceMarkOptions): PerformanceMark;
};

/** `PerformanceMeasure` is an abstract interface for `PerformanceEntry` objects
 * with an entryType of `"measure"`. Entries of this type are created by calling
 * `performance.measure()` to add a named `DOMHighResTimeStamp` (the measure)
 * between two marks to the performance timeline.
 *
 * @category Performance
 */
interface PerformanceMeasure extends PerformanceEntry {
  readonly detail: any;
  readonly entryType: "measure";
}

/** `PerformanceMeasure` is an abstract interface for `PerformanceEntry` objects
 * with an entryType of `"measure"`. Entries of this type are created by calling
 * `performance.measure()` to add a named `DOMHighResTimeStamp` (the measure)
 * between two marks to the performance timeline.
 *
 * @category Performance
 */
declare var PerformanceMeasure: {
  readonly prototype: PerformanceMeasure;
  new (): never;
};

/** @category Events */
interface CustomEventInit<T = any> extends EventInit {
  detail?: T;
}

/** @category Events */
interface CustomEvent<T = any> extends Event {
  /** Returns any custom data event was created with. Typically used for
   * synthetic events. */
  readonly detail: T;
}

/** @category Events */
declare var CustomEvent: {
  readonly prototype: CustomEvent;
  new <T>(typeArg: string, eventInitDict?: CustomEventInit<T>): CustomEvent<T>;
};

/** @category Platform */
interface ErrorConstructor {
  /** See https://v8.dev/docs/stack-trace-api#stack-trace-collection-for-custom-exceptions. */
  captureStackTrace(error: Object, constructor?: Function): void;
  stackTraceLimit: number;
  // TODO(nayeemrmn): Support `Error.prepareStackTrace()`. We currently use this
  // internally in a way that makes it unavailable for users.
}

/** The [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API)
 * which also supports setting a {@linkcode Deno.HttpClient} which provides a
 * way to connect via proxies and use custom TLS certificates.
 *
 * @tags allow-net, allow-read
 * @category Fetch
 */
declare function fetch(
  input: RequestInfo | URL,
  init?: RequestInit & { client?: Deno.HttpClient },
): Promise<Response>;
