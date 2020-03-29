// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-empty-interface, @typescript-eslint/no-explicit-any */

/// <reference no-default-lib="true" />
// TODO: we need to remove this, but Fetch::Response::Body implements Reader
// which requires Deno.EOF, and we shouldn't be leaking that, but https_proxy
// at the least requires the Reader interface on Body, which it shouldn't
/// <reference lib="deno.ns" />
/// <reference lib="esnext" />

// https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope

declare interface WindowOrWorkerGlobalScope {
  // methods
  atob: typeof __textEncoding.atob;
  btoa: typeof __textEncoding.btoa;
  clearInterval: typeof __timers.clearInterval;
  clearTimeout: typeof __timers.clearTimeout;
  fetch: typeof __fetch.fetch;
  setInterval: typeof __timers.setInterval;
  queueMicrotask: typeof __timers.queueMicrotask;
  setTimeout: typeof __timers.setTimeout;
  // properties
  console: __console.Console;
  Blob: typeof __blob.DenoBlob;
  File: __domTypes.DomFileConstructor;
  CustomEvent: typeof __customEvent.CustomEvent;
  Event: typeof __event.Event;
  EventTarget: typeof __eventTarget.EventTarget;
  URL: typeof __url.URL;
  URLSearchParams: typeof __urlSearchParams.URLSearchParams;
  Headers: __domTypes.HeadersConstructor;
  FormData: __domTypes.FormDataConstructor;
  TextEncoder: typeof __textEncoding.TextEncoder;
  TextDecoder: typeof __textEncoding.TextDecoder;
  Request: __domTypes.RequestConstructor;
  Response: typeof __fetch.Response;
  performance: __performanceUtil.Performance;
  Worker: typeof __workers.WorkerImpl;
  location: __domTypes.Location;

  addEventListener: (
    type: string,
    callback: __domTypes.EventListenerOrEventListenerObject | null,
    options?: boolean | __domTypes.AddEventListenerOptions | undefined
  ) => void;
  dispatchEvent: (event: __domTypes.Event) => boolean;
  removeEventListener: (
    type: string,
    callback: __domTypes.EventListenerOrEventListenerObject | null,
    options?: boolean | __domTypes.EventListenerOptions | undefined
  ) => void;
}

// This follows the WebIDL at: https://webassembly.github.io/spec/js-api/
// and: https://webassembly.github.io/spec/web-api/

declare namespace WebAssembly {
  interface WebAssemblyInstantiatedSource {
    module: Module;
    instance: Instance;
  }

  /** Compiles a `WebAssembly.Module` from WebAssembly binary code.  This
   * function is useful if it is necessary to a compile a module before it can
   * be instantiated (otherwise, the `WebAssembly.instantiate()` function
   * should be used). */
  function compile(bufferSource: __domTypes.BufferSource): Promise<Module>;

  /** Compiles a `WebAssembly.Module` directly from a streamed underlying
   * source. This function is useful if it is necessary to a compile a module
   * before it can be instantiated (otherwise, the
   * `WebAssembly.instantiateStreaming()` function should be used). */
  function compileStreaming(
    source: Promise<__domTypes.Response>
  ): Promise<Module>;

  /** Takes the WebAssembly binary code, in the form of a typed array or
   * `ArrayBuffer`, and performs both compilation and instantiation in one step.
   * The returned `Promise` resolves to both a compiled `WebAssembly.Module` and
   * its first `WebAssembly.Instance`. */
  function instantiate(
    bufferSource: __domTypes.BufferSource,
    importObject?: object
  ): Promise<WebAssemblyInstantiatedSource>;

  /** Takes an already-compiled `WebAssembly.Module` and returns a `Promise`
   * that resolves to an `Instance` of that `Module`. This overload is useful if
   * the `Module` has already been compiled. */
  function instantiate(
    module: Module,
    importObject?: object
  ): Promise<Instance>;

  /** Compiles and instantiates a WebAssembly module directly from a streamed
   * underlying source. This is the most efficient, optimized way to load wasm
   * code. */
  function instantiateStreaming(
    source: Promise<__domTypes.Response>,
    importObject?: object
  ): Promise<WebAssemblyInstantiatedSource>;

  /** Validates a given typed array of WebAssembly binary code, returning
   * whether the bytes form a valid wasm module (`true`) or not (`false`). */
  function validate(bufferSource: __domTypes.BufferSource): boolean;

  type ImportExportKind = "function" | "table" | "memory" | "global";

  interface ModuleExportDescriptor {
    name: string;
    kind: ImportExportKind;
  }
  interface ModuleImportDescriptor {
    module: string;
    name: string;
    kind: ImportExportKind;
  }

  class Module {
    constructor(bufferSource: __domTypes.BufferSource);

    /** Given a `Module` and string, returns a copy of the contents of all
     * custom sections in the module with the given string name. */
    static customSections(
      moduleObject: Module,
      sectionName: string
    ): ArrayBuffer;

    /** Given a `Module`, returns an array containing descriptions of all the
     * declared exports. */
    static exports(moduleObject: Module): ModuleExportDescriptor[];

    /** Given a `Module`, returns an array containing descriptions of all the
     * declared imports. */
    static imports(moduleObject: Module): ModuleImportDescriptor[];
  }

  class Instance<T extends object = { [key: string]: any }> {
    constructor(module: Module, importObject?: object);

    /** An object containing as its members all the functions exported from the
     * WebAssembly module instance, to allow them to be accessed and used by
     * JavaScript. */
    readonly exports: T;
  }

  interface MemoryDescriptor {
    initial: number;
    maximum?: number;
  }

  class Memory {
    constructor(descriptor: MemoryDescriptor);

    /** An accessor property that returns the buffer contained in the memory. */
    readonly buffer: ArrayBuffer;

    /** Increases the size of the memory instance by a specified number of
     * WebAssembly pages (each one is 64KB in size). */
    grow(delta: number): number;
  }

  type TableKind = "anyfunc";

  interface TableDescriptor {
    element: TableKind;
    initial: number;
    maximum?: number;
  }

  class Table {
    constructor(descriptor: TableDescriptor);

    /** Returns the length of the table, i.e. the number of elements. */
    readonly length: number;

    /** Accessor function — gets the element stored at a given index. */
    get(index: number): (...args: any[]) => any;

    /** Increases the size of the Table instance by a specified number of
     * elements. */
    grow(delta: number): number;

    /** Sets an element stored at a given index to a given value. */
    set(index: number, value: (...args: any[]) => any): void;
  }

  type ValueType = "i32" | "i64" | "f32" | "f64";

  interface GlobalDescriptor {
    value: ValueType;
    mutable?: boolean;
  }

  /** Represents a global variable instance, accessible from both JavaScript and
   * importable/exportable across one or more `WebAssembly.Module` instances.
   * This allows dynamic linking of multiple modules. */
  class Global {
    constructor(descriptor: GlobalDescriptor, value?: any);

    /** Old-style method that returns the value contained inside the global
     * variable. */
    valueOf(): any;

    /** The value contained inside the global variable — this can be used to
     * directly set and get the global's value. */
    value: any;
  }

  /** Indicates an error during WebAssembly decoding or validation */
  class CompileError extends Error {
    constructor(message: string, fileName?: string, lineNumber?: string);
  }

  /** Indicates an error during module instantiation (besides traps from the
   * start function). */
  class LinkError extends Error {
    constructor(message: string, fileName?: string, lineNumber?: string);
  }

  /** Is thrown whenever WebAssembly specifies a trap. */
  class RuntimeError extends Error {
    constructor(message: string, fileName?: string, lineNumber?: string);
  }
}

declare const atob: typeof __textEncoding.atob;
declare const btoa: typeof __textEncoding.btoa;
declare const clearInterval: typeof __timers.clearInterval;
declare const clearTimeout: typeof __timers.clearTimeout;
declare const fetch: typeof __fetch.fetch;
declare const setInterval: typeof __timers.setInterval;
declare const setTimeout: typeof __timers.setTimeout;
declare const queueMicrotask: typeof __timers.queueMicrotask;

declare const console: __console.Console;
declare const Blob: typeof __blob.DenoBlob;
declare const File: __domTypes.DomFileConstructor;
declare const CustomEventInit: typeof __customEvent.CustomEventInit;
declare const CustomEvent: typeof __customEvent.CustomEvent;
declare const EventInit: typeof __event.EventInit;
declare const Event: typeof __event.Event;
declare const EventListener: __domTypes.EventListener;
declare const EventTarget: typeof __eventTarget.EventTarget;
declare const URL: typeof __url.URL;
declare const URLSearchParams: typeof __urlSearchParams.URLSearchParams;
declare const Headers: __domTypes.HeadersConstructor;
declare const location: __domTypes.Location;
declare const FormData: __domTypes.FormDataConstructor;
declare const TextEncoder: typeof __textEncoding.TextEncoder;
declare const TextDecoder: typeof __textEncoding.TextDecoder;
declare const Request: __domTypes.RequestConstructor;
declare const Response: typeof __fetch.Response;
declare const performance: __performanceUtil.Performance;
declare const Worker: typeof __workers.WorkerImpl;

declare const addEventListener: (
  type: string,
  callback: __domTypes.EventListenerOrEventListenerObject | null,
  options?: boolean | __domTypes.AddEventListenerOptions | undefined
) => void;
declare const dispatchEvent: (event: __domTypes.Event) => boolean;
declare const removeEventListener: (
  type: string,
  callback: __domTypes.EventListenerOrEventListenerObject | null,
  options?: boolean | __domTypes.EventListenerOptions | undefined
) => void;

declare type Blob = __domTypes.Blob;
declare type Body = __domTypes.Body;
declare type File = __domTypes.DomFile;
declare type CustomEventInit = __domTypes.CustomEventInit;
declare type CustomEvent = __domTypes.CustomEvent;
declare type EventInit = __domTypes.EventInit;
declare type Event = __domTypes.Event;
declare type EventListener = __domTypes.EventListener;
declare type EventTarget = __domTypes.EventTarget;
declare type URL = __url.URL;
declare type URLSearchParams = __domTypes.URLSearchParams;
declare type Headers = __domTypes.Headers;
declare type FormData = __domTypes.FormData;
declare type TextEncoder = __textEncoding.TextEncoder;
declare type TextDecoder = __textEncoding.TextDecoder;
declare type Request = __domTypes.Request;
declare type Response = __domTypes.Response;
declare type Worker = __workers.Worker;

declare interface ImportMeta {
  url: string;
  main: boolean;
}

declare namespace __domTypes {
  export type BufferSource = ArrayBufferView | ArrayBuffer;
  export type HeadersInit =
    | Headers
    | Array<[string, string]>
    | Record<string, string>;
  export type URLSearchParamsInit =
    | string
    | string[][]
    | Record<string, string>;
  type BodyInit =
    | Blob
    | BufferSource
    | FormData
    | URLSearchParams
    | ReadableStream
    | string;
  export type RequestInfo = Request | string;
  type ReferrerPolicy =
    | ""
    | "no-referrer"
    | "no-referrer-when-downgrade"
    | "origin-only"
    | "origin-when-cross-origin"
    | "unsafe-url";
  export type BlobPart = BufferSource | Blob | string;
  export type FormDataEntryValue = DomFile | string;
  export interface DomIterable<K, V> {
    keys(): IterableIterator<K>;
    values(): IterableIterator<V>;
    entries(): IterableIterator<[K, V]>;
    [Symbol.iterator](): IterableIterator<[K, V]>;
    forEach(
      callback: (value: V, key: K, parent: this) => void,
      thisArg?: any
    ): void;
  }
  type EndingType = "transparent" | "native";
  export interface BlobPropertyBag {
    type?: string;
    ending?: EndingType;
  }
  interface AbortSignalEventMap {
    abort: ProgressEvent;
  }
  export enum NodeType {
    ELEMENT_NODE = 1,
    TEXT_NODE = 3,
    DOCUMENT_FRAGMENT_NODE = 11,
  }
  export interface EventListener {
    (evt: Event): void | Promise<void>;
  }
  export interface EventListenerObject {
    handleEvent(evt: Event): void | Promise<void>;
  }
  export type EventListenerOrEventListenerObject =
    | EventListener
    | EventListenerObject;
  export interface EventTargetListener {
    callback: EventListenerOrEventListenerObject;
    options: AddEventListenerOptions;
  }
  export const eventTargetHost: unique symbol;
  export const eventTargetListeners: unique symbol;
  export const eventTargetMode: unique symbol;
  export const eventTargetNodeType: unique symbol;
  export interface EventTarget {
    addEventListener(
      type: string,
      callback: EventListenerOrEventListenerObject | null,
      options?: boolean | AddEventListenerOptions
    ): void;
    dispatchEvent(event: Event): boolean;
    removeEventListener(
      type: string,
      callback?: EventListenerOrEventListenerObject | null,
      options?: EventListenerOptions | boolean
    ): void;
  }
  export interface ProgressEventInit extends EventInit {
    lengthComputable?: boolean;
    loaded?: number;
    total?: number;
  }
  export interface URLSearchParams extends DomIterable<string, string> {
    /**
     * Appends a specified key/value pair as a new search parameter.
     */
    append(name: string, value: string): void;
    /**
     * Deletes the given search parameter, and its associated value,
     * from the list of all search parameters.
     */
    delete(name: string): void;
    /**
     * Returns the first value associated to the given search parameter.
     */
    get(name: string): string | null;
    /**
     * Returns all the values association with a given search parameter.
     */
    getAll(name: string): string[];
    /**
     * Returns a Boolean indicating if such a search parameter exists.
     */
    has(name: string): boolean;
    /**
     * Sets the value associated to a given search parameter to the given value.
     * If there were several values, delete the others.
     */
    set(name: string, value: string): void;
    /**
     * Sort all key/value pairs contained in this object in place
     * and return undefined. The sort order is according to Unicode
     * code points of the keys.
     */
    sort(): void;
    /**
     * Returns a query string suitable for use in a URL.
     */
    toString(): string;
    /**
     * Iterates over each name-value pair in the query
     * and invokes the given function.
     */
    forEach(
      callbackfn: (value: string, key: string, parent: this) => void,
      thisArg?: any
    ): void;
  }
  export interface EventInit {
    bubbles?: boolean;
    cancelable?: boolean;
    composed?: boolean;
  }
  export interface CustomEventInit extends EventInit {
    detail?: any;
  }
  export enum EventPhase {
    NONE = 0,
    CAPTURING_PHASE = 1,
    AT_TARGET = 2,
    BUBBLING_PHASE = 3,
  }
  export interface EventPath {
    item: EventTarget;
    itemInShadowTree: boolean;
    relatedTarget: EventTarget | null;
    rootOfClosedTree: boolean;
    slotInClosedTree: boolean;
    target: EventTarget | null;
    touchTargetList: EventTarget[];
  }
  export interface Event {
    readonly type: string;
    target: EventTarget | null;
    currentTarget: EventTarget | null;
    composedPath(): EventPath[];
    eventPhase: number;
    stopPropagation(): void;
    stopImmediatePropagation(): void;
    readonly bubbles: boolean;
    readonly cancelable: boolean;
    preventDefault(): void;
    readonly defaultPrevented: boolean;
    readonly composed: boolean;
    isTrusted: boolean;
    readonly timeStamp: Date;
    dispatched: boolean;
    readonly initialized: boolean;
    inPassiveListener: boolean;
    cancelBubble: boolean;
    cancelBubbleImmediately: boolean;
    path: EventPath[];
    relatedTarget: EventTarget | null;
  }
  export interface CustomEvent extends Event {
    readonly detail: any;
    initCustomEvent(
      type: string,
      bubbles?: boolean,
      cancelable?: boolean,
      detail?: any | null
    ): void;
  }
  export interface DomFile extends Blob {
    readonly lastModified: number;
    readonly name: string;
  }
  export interface DomFileConstructor {
    new (
      bits: BlobPart[],
      filename: string,
      options?: FilePropertyBag
    ): DomFile;
    prototype: DomFile;
  }
  export interface FilePropertyBag extends BlobPropertyBag {
    lastModified?: number;
  }
  interface ProgressEvent extends Event {
    readonly lengthComputable: boolean;
    readonly loaded: number;
    readonly total: number;
  }
  export interface EventListenerOptions {
    capture?: boolean;
  }
  export interface AddEventListenerOptions extends EventListenerOptions {
    once?: boolean;
    passive?: boolean;
  }
  interface AbortSignal extends EventTarget {
    readonly aborted: boolean;
    onabort: ((this: AbortSignal, ev: ProgressEvent) => any) | null;
    addEventListener<K extends keyof AbortSignalEventMap>(
      type: K,
      listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
      options?: boolean | AddEventListenerOptions
    ): void;
    addEventListener(
      type: string,
      listener: EventListener,
      options?: boolean | AddEventListenerOptions
    ): void;
    removeEventListener<K extends keyof AbortSignalEventMap>(
      type: K,
      listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
      options?: boolean | EventListenerOptions
    ): void;
    removeEventListener(
      type: string,
      listener: EventListener,
      options?: boolean | EventListenerOptions
    ): void;
  }
  export interface ReadableStreamReadDoneResult<T> {
    done: true;
    value?: T;
  }
  export interface ReadableStreamReadValueResult<T> {
    done: false;
    value: T;
  }
  export type ReadableStreamReadResult<T> =
    | ReadableStreamReadValueResult<T>
    | ReadableStreamReadDoneResult<T>;
  export interface ReadableStreamDefaultReader<R = any> {
    readonly closed: Promise<void>;
    cancel(reason?: any): Promise<void>;
    read(): Promise<ReadableStreamReadResult<R>>;
    releaseLock(): void;
  }
  export interface PipeOptions {
    preventAbort?: boolean;
    preventCancel?: boolean;
    preventClose?: boolean;
    signal?: AbortSignal;
  }
  /** This Streams API interface represents a readable stream of byte data. The
   * Fetch API offers a concrete instance of a ReadableStream through the body
   * property of a Response object. */
  export interface ReadableStream<R = any> {
    readonly locked: boolean;
    cancel(reason?: any): Promise<void>;
    getReader(options: { mode: "byob" }): ReadableStreamBYOBReader;
    getReader(): ReadableStreamDefaultReader<R>;
    /* disabled for now
    pipeThrough<T>(
      {
        writable,
        readable
      }: {
        writable: WritableStream<R>;
        readable: ReadableStream<T>;
      },
      options?: PipeOptions
    ): ReadableStream<T>;
    pipeTo(dest: WritableStream<R>, options?: PipeOptions): Promise<void>;
    */
    tee(): [ReadableStream<R>, ReadableStream<R>];
  }
  export interface ReadableStreamReader<R = any> {
    cancel(reason: any): Promise<void>;
    read(): Promise<ReadableStreamReadResult<R>>;
    releaseLock(): void;
  }
  export interface ReadableStreamBYOBReader {
    readonly closed: Promise<void>;
    cancel(reason?: any): Promise<void>;
    read<T extends ArrayBufferView>(
      view: T
    ): Promise<ReadableStreamReadResult<T>>;
    releaseLock(): void;
  }
  export interface WritableStream<W = any> {
    readonly locked: boolean;
    abort(reason?: any): Promise<void>;
    getWriter(): WritableStreamDefaultWriter<W>;
  }
  export interface WritableStreamDefaultWriter<W = any> {
    readonly closed: Promise<void>;
    readonly desiredSize: number | null;
    readonly ready: Promise<void>;
    abort(reason?: any): Promise<void>;
    close(): Promise<void>;
    releaseLock(): void;
    write(chunk: W): Promise<void>;
  }
  export interface FormData extends DomIterable<string, FormDataEntryValue> {
    append(name: string, value: string | Blob, fileName?: string): void;
    delete(name: string): void;
    get(name: string): FormDataEntryValue | null;
    getAll(name: string): FormDataEntryValue[];
    has(name: string): boolean;
    set(name: string, value: string | Blob, fileName?: string): void;
  }
  export interface FormDataConstructor {
    new (): FormData;
    prototype: FormData;
  }
  /** A blob object represents a file-like object of immutable, raw data. */
  export interface Blob {
    /** The size, in bytes, of the data contained in the `Blob` object. */
    readonly size: number;
    /** A string indicating the media type of the data contained in the `Blob`.
     * If the type is unknown, this string is empty.
     */
    readonly type: string;
    /** Returns a new `Blob` object containing the data in the specified range of
     * bytes of the source `Blob`.
     */
    slice(start?: number, end?: number, contentType?: string): Blob;
  }
  export interface Body {
    /** A simple getter used to expose a `ReadableStream` of the body contents. */
    readonly body: ReadableStream<Uint8Array> | null;
    /** Stores a `Boolean` that declares whether the body has been used in a
     * response yet.
     */
    readonly bodyUsed: boolean;
    /** Takes a `Response` stream and reads it to completion. It returns a promise
     * that resolves with an `ArrayBuffer`.
     */
    arrayBuffer(): Promise<ArrayBuffer>;
    /** Takes a `Response` stream and reads it to completion. It returns a promise
     * that resolves with a `Blob`.
     */
    blob(): Promise<Blob>;
    /** Takes a `Response` stream and reads it to completion. It returns a promise
     * that resolves with a `FormData` object.
     */
    formData(): Promise<FormData>;
    /** Takes a `Response` stream and reads it to completion. It returns a promise
     * that resolves with the result of parsing the body text as JSON.
     */
    json(): Promise<any>;
    /** Takes a `Response` stream and reads it to completion. It returns a promise
     * that resolves with a `USVString` (text).
     */
    text(): Promise<string>;
  }
  export interface Headers extends DomIterable<string, string> {
    /** Appends a new value onto an existing header inside a `Headers` object, or
     * adds the header if it does not already exist.
     */
    append(name: string, value: string): void;
    /** Deletes a header from a `Headers` object. */
    delete(name: string): void;
    /** Returns an iterator allowing to go through all key/value pairs
     * contained in this Headers object. The both the key and value of each pairs
     * are ByteString objects.
     */
    entries(): IterableIterator<[string, string]>;
    /** Returns a `ByteString` sequence of all the values of a header within a
     * `Headers` object with a given name.
     */
    get(name: string): string | null;
    /** Returns a boolean stating whether a `Headers` object contains a certain
     * header.
     */
    has(name: string): boolean;
    /** Returns an iterator allowing to go through all keys contained in
     * this Headers object. The keys are ByteString objects.
     */
    keys(): IterableIterator<string>;
    /** Sets a new value for an existing header inside a Headers object, or adds
     * the header if it does not already exist.
     */
    set(name: string, value: string): void;
    /** Returns an iterator allowing to go through all values contained in
     * this Headers object. The values are ByteString objects.
     */
    values(): IterableIterator<string>;
    forEach(
      callbackfn: (value: string, key: string, parent: this) => void,
      thisArg?: any
    ): void;
    /** The Symbol.iterator well-known symbol specifies the default
     * iterator for this Headers object
     */
    [Symbol.iterator](): IterableIterator<[string, string]>;
  }
  export interface HeadersConstructor {
    new (init?: HeadersInit): Headers;
    prototype: Headers;
  }
  type RequestCache =
    | "default"
    | "no-store"
    | "reload"
    | "no-cache"
    | "force-cache"
    | "only-if-cached";
  type RequestCredentials = "omit" | "same-origin" | "include";
  type RequestDestination =
    | ""
    | "audio"
    | "audioworklet"
    | "document"
    | "embed"
    | "font"
    | "image"
    | "manifest"
    | "object"
    | "paintworklet"
    | "report"
    | "script"
    | "sharedworker"
    | "style"
    | "track"
    | "video"
    | "worker"
    | "xslt";
  type RequestMode = "navigate" | "same-origin" | "no-cors" | "cors";
  type RequestRedirect = "follow" | "error" | "manual";
  type ResponseType =
    | "basic"
    | "cors"
    | "default"
    | "error"
    | "opaque"
    | "opaqueredirect";
  export interface RequestInit {
    body?: BodyInit | null;
    cache?: RequestCache;
    credentials?: RequestCredentials;
    headers?: HeadersInit;
    integrity?: string;
    keepalive?: boolean;
    method?: string;
    mode?: RequestMode;
    redirect?: RequestRedirect;
    referrer?: string;
    referrerPolicy?: ReferrerPolicy;
    signal?: AbortSignal | null;
    window?: any;
  }
  export interface ResponseInit {
    headers?: HeadersInit;
    status?: number;
    statusText?: string;
  }
  export interface RequestConstructor {
    new (input: RequestInfo, init?: RequestInit): Request;
    prototype: Request;
  }
  export interface Request extends Body {
    /** Returns the cache mode associated with request, which is a string
     * indicating how the the request will interact with the browser's cache when
     * fetching.
     */
    readonly cache?: RequestCache;
    /** Returns the credentials mode associated with request, which is a string
     * indicating whether credentials will be sent with the request always, never,
     * or only when sent to a same-origin URL.
     */
    readonly credentials?: RequestCredentials;
    /** Returns the kind of resource requested by request, (e.g., `document` or
     * `script`).
     */
    readonly destination?: RequestDestination;
    /** Returns a Headers object consisting of the headers associated with
     * request.
     *
     * Note that headers added in the network layer by the user agent
     * will not be accounted for in this object, (e.g., the `Host` header).
     */
    readonly headers: Headers;
    /** Returns request's subresource integrity metadata, which is a cryptographic
     * hash of the resource being fetched. Its value consists of multiple hashes
     * separated by whitespace. [SRI]
     */
    readonly integrity?: string;
    /** Returns a boolean indicating whether or not request is for a history
     * navigation (a.k.a. back-forward navigation).
     */
    readonly isHistoryNavigation?: boolean;
    /** Returns a boolean indicating whether or not request is for a reload
     * navigation.
     */
    readonly isReloadNavigation?: boolean;
    /** Returns a boolean indicating whether or not request can outlive the global
     * in which it was created.
     */
    readonly keepalive?: boolean;
    /** Returns request's HTTP method, which is `GET` by default. */
    readonly method: string;
    /** Returns the mode associated with request, which is a string indicating
     * whether the request will use CORS, or will be restricted to same-origin
     * URLs.
     */
    readonly mode?: RequestMode;
    /** Returns the redirect mode associated with request, which is a string
     * indicating how redirects for the request will be handled during fetching.
     *
     * A request will follow redirects by default.
     */
    readonly redirect?: RequestRedirect;
    /** Returns the referrer of request. Its value can be a same-origin URL if
     * explicitly set in init, the empty string to indicate no referrer, and
     * `about:client` when defaulting to the global's default.
     *
     * This is used during fetching to determine the value of the `Referer`
     * header of the request being made.
     */
    readonly referrer?: string;
    /** Returns the referrer policy associated with request. This is used during
     * fetching to compute the value of the request's referrer.
     */
    readonly referrerPolicy?: ReferrerPolicy;
    /** Returns the signal associated with request, which is an AbortSignal object
     * indicating whether or not request has been aborted, and its abort event
     * handler.
     */
    readonly signal?: AbortSignal;
    /** Returns the URL of request as a string. */
    readonly url: string;
    clone(): Request;
  }
  export interface Response extends Body {
    /** Contains the `Headers` object associated with the response. */
    readonly headers: Headers;
    /** Contains a boolean stating whether the response was successful (status in
     * the range 200-299) or not.
     */
    readonly ok: boolean;
    /** Indicates whether or not the response is the result of a redirect; that
     * is, its URL list has more than one entry.
     */
    readonly redirected: boolean;
    /** Contains the status code of the response (e.g., `200` for a success). */
    readonly status: number;
    /** Contains the status message corresponding to the status code (e.g., `OK`
     * for `200`).
     */
    readonly statusText: string;
    readonly trailer: Promise<Headers>;
    /** Contains the type of the response (e.g., `basic`, `cors`). */
    readonly type: ResponseType;
    /** Contains the URL of the response. */
    readonly url: string;
    /** Creates a clone of a `Response` object. */
    clone(): Response;
  }
  export interface DOMStringList {
    /** Returns the number of strings in strings. */
    readonly length: number;
    /** Returns true if strings contains string, and false otherwise. */
    contains(string: string): boolean;
    /** Returns the string with index index from strings. */
    item(index: number): string | null;
    [index: number]: string;
  }
  /** The location (URL) of the object it is linked to. Changes done on it are
   * reflected on the object it relates to. Both the Document and Window
   * interface have such a linked Location, accessible via Document.location and
   * Window.location respectively. */
  export interface Location {
    /** Returns a DOMStringList object listing the origins of the ancestor
     * browsing contexts, from the parent browsing context to the top-level
     * browsing context. */
    readonly ancestorOrigins: DOMStringList;
    /** Returns the Location object's URL's fragment (includes leading "#" if
     * non-empty).
     *
     * Can be set, to navigate to the same URL with a changed fragment (ignores
     * leading "#"). */
    hash: string;
    /** Returns the Location object's URL's host and port (if different from the
     * default port for the scheme).
     *
     * Can be set, to navigate to the same URL with a changed host and port. */
    host: string;
    /** Returns the Location object's URL's host.
     *
     * Can be set, to navigate to the same URL with a changed host. */
    hostname: string;
    /** Returns the Location object's URL.
     *
     * Can be set, to navigate to the given URL. */
    href: string;
    toString(): string;
    /** Returns the Location object's URL's origin. */
    readonly origin: string;
    /** Returns the Location object's URL's path.
     *
     * Can be set, to navigate to the same URL with a changed path. */
    pathname: string;
    /** Returns the Location object's URL's port.
     *
     * Can be set, to navigate to the same URL with a changed port. */
    port: string;
    /** Returns the Location object's URL's scheme.
     *
     * Can be set, to navigate to the same URL with a changed scheme. */
    protocol: string;
    /** Returns the Location object's URL's query (includes leading "?" if
     * non-empty).
     *
     * Can be set, to navigate to the same URL with a changed query (ignores
     * leading "?"). */
    search: string;
    /**
     * Navigates to the given URL.
     */
    assign(url: string): void;
    /**
     * Reloads the current page.
     */
    reload(): void;
    /** Removes the current page from the session history and navigates to the
     * given URL. */
    replace(url: string): void;
  }
}

declare namespace __blob {
  export class DenoBlob implements __domTypes.Blob {
    readonly size: number;
    readonly type: string;
    /** A blob object represents a file-like object of immutable, raw data. */
    constructor(
      blobParts?: __domTypes.BlobPart[],
      options?: __domTypes.BlobPropertyBag
    );
    slice(start?: number, end?: number, contentType?: string): DenoBlob;
  }
}

declare namespace __console {
  type InspectOptions = Partial<{
    showHidden: boolean;
    depth: number;
    colors: boolean;
    indentLevel: number;
  }>;
  export class CSI {
    static kClear: string;
    static kClearScreenDown: string;
  }
  const isConsoleInstance: unique symbol;
  export class Console {
    indentLevel: number;
    [isConsoleInstance]: boolean;
    /** Writes the arguments to stdout */
    log: (...args: unknown[]) => void;
    /** Writes the arguments to stdout */
    debug: (...args: unknown[]) => void;
    /** Writes the arguments to stdout */
    info: (...args: unknown[]) => void;
    /** Writes the properties of the supplied `obj` to stdout */
    dir: (
      obj: unknown,
      options?: Partial<{
        showHidden: boolean;
        depth: number;
        colors: boolean;
        indentLevel: number;
      }>
    ) => void;

    /** From MDN:
     * Displays an interactive tree of the descendant elements of
     * the specified XML/HTML element. If it is not possible to display
     * as an element the JavaScript Object view is shown instead.
     * The output is presented as a hierarchical listing of expandable
     * nodes that let you see the contents of child nodes.
     *
     * Since we write to stdout, we can't display anything interactive
     * we just fall back to `console.dir`.
     */
    dirxml: (
      obj: unknown,
      options?: Partial<{
        showHidden: boolean;
        depth: number;
        colors: boolean;
        indentLevel: number;
      }>
    ) => void;

    /** Writes the arguments to stdout */
    warn: (...args: unknown[]) => void;
    /** Writes the arguments to stdout */
    error: (...args: unknown[]) => void;
    /** Writes an error message to stdout if the assertion is `false`. If the
     * assertion is `true`, nothing happens.
     *
     * ref: https://console.spec.whatwg.org/#assert
     */
    assert: (condition?: boolean, ...args: unknown[]) => void;
    count: (label?: string) => void;
    countReset: (label?: string) => void;
    table: (data: unknown, properties?: string[] | undefined) => void;
    time: (label?: string) => void;
    timeLog: (label?: string, ...args: unknown[]) => void;
    timeEnd: (label?: string) => void;
    group: (...label: unknown[]) => void;
    groupCollapsed: (...label: unknown[]) => void;
    groupEnd: () => void;
    clear: () => void;
    trace: (...args: unknown[]) => void;
    static [Symbol.hasInstance](instance: Console): boolean;
  }
  /** A symbol which can be used as a key for a custom method which will be called
   * when `Deno.inspect()` is called, or when the object is logged to the console.
   */
  export const customInspect: unique symbol;
  /**
   * `inspect()` converts input into string that has the same format
   * as printed by `console.log(...)`;
   */
  export function inspect(value: unknown, options?: InspectOptions): string;
}

declare namespace __event {
  export const eventAttributes: WeakMap<object, any>;
  export class EventInit implements __domTypes.EventInit {
    bubbles: boolean;
    cancelable: boolean;
    composed: boolean;
    constructor({
      bubbles,
      cancelable,
      composed,
    }?: {
      bubbles?: boolean | undefined;
      cancelable?: boolean | undefined;
      composed?: boolean | undefined;
    });
  }
  export class Event implements __domTypes.Event {
    isTrusted: boolean;
    private _canceledFlag;
    private _dispatchedFlag;
    private _initializedFlag;
    private _inPassiveListenerFlag;
    private _stopImmediatePropagationFlag;
    private _stopPropagationFlag;
    private _path;
    constructor(type: string, eventInitDict?: __domTypes.EventInit);
    readonly bubbles: boolean;
    cancelBubble: boolean;
    cancelBubbleImmediately: boolean;
    readonly cancelable: boolean;
    readonly composed: boolean;
    currentTarget: __domTypes.EventTarget;
    readonly defaultPrevented: boolean;
    dispatched: boolean;
    eventPhase: number;
    readonly initialized: boolean;
    inPassiveListener: boolean;
    path: __domTypes.EventPath[];
    relatedTarget: __domTypes.EventTarget;
    target: __domTypes.EventTarget;
    readonly timeStamp: Date;
    readonly type: string;
    /** Returns the event’s path (objects on which listeners will be
     * invoked). This does not include nodes in shadow trees if the
     * shadow root was created with its ShadowRoot.mode closed.
     *
     *      event.composedPath();
     */
    composedPath(): __domTypes.EventPath[];
    /** Cancels the event (if it is cancelable).
     * See https://dom.spec.whatwg.org/#set-the-canceled-flag
     *
     *      event.preventDefault();
     */
    preventDefault(): void;
    /** Stops the propagation of events further along in the DOM.
     *
     *      event.stopPropagation();
     */
    stopPropagation(): void;
    /** For this particular event, no other listener will be called.
     * Neither those attached on the same element, nor those attached
     * on elements which will be traversed later (in capture phase,
     * for instance).
     *
     *      event.stopImmediatePropagation();
     */
    stopImmediatePropagation(): void;
  }
}

declare namespace __customEvent {
  export const customEventAttributes: WeakMap<object, any>;
  export class CustomEventInit extends __event.EventInit
    implements __domTypes.CustomEventInit {
    detail: any;
    constructor({
      bubbles,
      cancelable,
      composed,
      detail,
    }: __domTypes.CustomEventInit);
  }
  export class CustomEvent extends __event.Event
    implements __domTypes.CustomEvent {
    constructor(type: string, customEventInitDict?: __domTypes.CustomEventInit);
    readonly detail: any;
    initCustomEvent(
      type: string,
      bubbles?: boolean,
      cancelable?: boolean,
      detail?: any
    ): void;
    readonly [Symbol.toStringTag]: string;
  }
}

declare namespace __eventTarget {
  export class EventListenerOptions implements __domTypes.EventListenerOptions {
    _capture: boolean;
    constructor({ capture }?: { capture?: boolean | undefined });
    readonly capture: boolean;
  }
  export class AddEventListenerOptions extends EventListenerOptions
    implements __domTypes.AddEventListenerOptions {
    _passive: boolean;
    _once: boolean;
    constructor({
      capture,
      passive,
      once,
    }?: {
      capture?: boolean | undefined;
      passive?: boolean | undefined;
      once?: boolean | undefined;
    });
    readonly passive: boolean;
    readonly once: boolean;
  }
  export const eventTargetAssignedSlot: unique symbol;
  export const eventTargetHasActivationBehavior: unique symbol;
  export class EventTarget implements __domTypes.EventTarget {
    [__domTypes.eventTargetHost]: __domTypes.EventTarget | null;
    [__domTypes.eventTargetListeners]: {
      [type in string]: __domTypes.EventListener[];
    };
    [__domTypes.eventTargetMode]: string;
    [__domTypes.eventTargetNodeType]: __domTypes.NodeType;
    private [eventTargetAssignedSlot];
    private [eventTargetHasActivationBehavior];
    addEventListener(
      type: string,
      callback: __domTypes.EventListenerOrEventListenerObject | null,
      options?: __domTypes.AddEventListenerOptions | boolean
    ): void;
    removeEventListener(
      type: string,
      callback: __domTypes.EventListenerOrEventListenerObject | null,
      options?: __domTypes.EventListenerOptions | boolean
    ): void;
    dispatchEvent(event: __domTypes.Event): boolean;
    readonly [Symbol.toStringTag]: string;
  }
}

declare namespace __io {
  /** UNSTABLE: maybe remove "SEEK_" prefix. Maybe capitalization wrong. */
  export enum SeekMode {
    SEEK_START = 0,
    SEEK_CURRENT = 1,
    SEEK_END = 2,
  }
  export interface Reader {
    /** Reads up to p.byteLength bytes into `p`. It resolves to the number
     * of bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error encountered.
     * Even if `read()` returns `n` < `p.byteLength`, it may use all of `p` as
     * scratch space during the call. If some data is available but not
     * `p.byteLength` bytes, `read()` conventionally returns what is available
     * instead of waiting for more.
     *
     * When `read()` encounters end-of-file condition, it returns EOF symbol.
     *
     * When `read()` encounters an error, it rejects with an error.
     *
     * Callers should always process the `n` > `0` bytes returned before
     * considering the EOF. Doing so correctly handles I/O errors that happen
     * after reading some bytes and also both of the allowed EOF behaviors.
     *
     * Implementations must not retain `p`.
     */
    read(p: Uint8Array): Promise<number | Deno.EOF>;
  }
  export interface SyncReader {
    readSync(p: Uint8Array): number | Deno.EOF;
  }
  export interface Writer {
    /** Writes `p.byteLength` bytes from `p` to the underlying data
     * stream. It resolves to the number of bytes written from `p` (`0` <= `n` <=
     * `p.byteLength`) and any error encountered that caused the write to stop
     * early. `write()` must return a non-null error if it returns `n` <
     * `p.byteLength`. write() must not modify the slice data, even temporarily.
     *
     * Implementations must not retain `p`.
     */
    write(p: Uint8Array): Promise<number>;
  }
  export interface SyncWriter {
    writeSync(p: Uint8Array): number;
  }
  export interface Closer {
    close(): void;
  }
  export interface Seeker {
    /** Seek sets the offset for the next `read()` or `write()` to offset,
     * interpreted according to `whence`: `SEEK_START` means relative to the
     * start of the file, `SEEK_CURRENT` means relative to the current offset,
     * and `SEEK_END` means relative to the end. Seek returns the new offset
     * relative to the start of the file and an error, if any.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O operations
     * on the underlying object is implementation-dependent.
     * It returns the cursor position.
     */
    seek(offset: number, whence: SeekMode): Promise<number>;
  }
  export interface SyncSeeker {
    seekSync(offset: number, whence: SeekMode): number;
  }
  export interface ReadCloser extends Reader, Closer {}
  export interface WriteCloser extends Writer, Closer {}
  export interface ReadSeeker extends Reader, Seeker {}
  export interface WriteSeeker extends Writer, Seeker {}
  export interface ReadWriteCloser extends Reader, Writer, Closer {}
  export interface ReadWriteSeeker extends Reader, Writer, Seeker {}

  /** UNSTABLE: controversial.
   *
   * Copies from `src` to `dst` until either `EOF` is reached on `src`
   * or an error occurs. It returns the number of bytes copied and the first
   * error encountered while copying, if any.
   *
   * Because `copy()` is defined to read from `src` until `EOF`, it does not
   * treat an `EOF` from `read()` as an error to be reported.
   */
  export function copy(dst: Writer, src: Reader): Promise<number>;

  /** UNSTABLE: Make Reader into AsyncIterable? Remove this?
   *
   * Turns `r` into async iterator.
   *
   *      for await (const chunk of toAsyncIterator(reader)) {
   *          console.log(chunk)
   *      }
   */
  export function toAsyncIterator(r: Reader): AsyncIterableIterator<Uint8Array>;
}

declare namespace __fetch {
  class Body
    implements
      __domTypes.Body,
      __domTypes.ReadableStream<Uint8Array>,
      __io.ReadCloser {
    readonly contentType: string;
    bodyUsed: boolean;
    readonly locked: boolean;
    readonly body: __domTypes.ReadableStream<Uint8Array>;
    constructor(rid: number, contentType: string);
    arrayBuffer(): Promise<ArrayBuffer>;
    blob(): Promise<__domTypes.Blob>;
    formData(): Promise<__domTypes.FormData>;
    json(): Promise<any>;
    text(): Promise<string>;
    read(p: Uint8Array): Promise<number | Deno.EOF>;
    close(): void;
    cancel(): Promise<void>;
    getReader(options: { mode: "byob" }): __domTypes.ReadableStreamBYOBReader;
    getReader(): __domTypes.ReadableStreamDefaultReader<Uint8Array>;
    getReader(): __domTypes.ReadableStreamBYOBReader;
    tee(): [__domTypes.ReadableStream, __domTypes.ReadableStream];
    [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array>;
  }
  export class Response implements __domTypes.Response {
    readonly url: string;
    readonly status: number;
    statusText: string;
    readonly type: __domTypes.ResponseType;
    readonly redirected: boolean;
    headers: __domTypes.Headers;
    readonly trailer: Promise<__domTypes.Headers>;
    bodyUsed: boolean;
    readonly body: Body;
    constructor(
      url: string,
      status: number,
      statusText: string,
      headersList: Array<[string, string]>,
      rid: number,
      redirected_: boolean,
      type_?: null | __domTypes.ResponseType,
      body_?: null | Body
    );
    arrayBuffer(): Promise<ArrayBuffer>;
    blob(): Promise<__domTypes.Blob>;
    formData(): Promise<__domTypes.FormData>;
    json(): Promise<any>;
    text(): Promise<string>;
    readonly ok: boolean;
    clone(): __domTypes.Response;
    redirect(url: URL | string, status: number): __domTypes.Response;
  }
  /** Fetch a resource from the network. */
  export function fetch(
    input: __domTypes.Request | __url.URL | string,
    init?: __domTypes.RequestInit
  ): Promise<Response>;
}

declare namespace __textEncoding {
  export function atob(s: string): string;
  /** Creates a base-64 ASCII string from the input string. */
  export function btoa(s: string): string;
  export interface TextDecodeOptions {
    stream?: false;
  }
  export interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: boolean;
  }
  export class TextDecoder {
    /** Returns encoding's name, lowercased. */
    readonly encoding: string;
    /** Returns `true` if error mode is "fatal", and `false` otherwise. */
    readonly fatal: boolean;
    /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
    readonly ignoreBOM = false;
    constructor(label?: string, options?: TextDecoderOptions);
    /** Returns the result of running encoding's decoder. */
    decode(
      input?: __domTypes.BufferSource,
      options?: TextDecodeOptions
    ): string;
    readonly [Symbol.toStringTag]: string;
  }
  interface TextEncoderEncodeIntoResult {
    read: number;
    written: number;
  }
  export class TextEncoder {
    /** Returns "utf-8". */
    readonly encoding = "utf-8";
    /** Returns the result of running UTF-8's encoder. */
    encode(input?: string): Uint8Array;
    encodeInto(input: string, dest: Uint8Array): TextEncoderEncodeIntoResult;
    readonly [Symbol.toStringTag]: string;
  }
}

declare namespace __timers {
  export type Args = unknown[];
  /** Sets a timer which executes a function once after the timer expires. */
  export function setTimeout(
    cb: (...args: Args) => void,
    delay?: number,
    ...args: Args
  ): number;
  /** Repeatedly calls a function , with a fixed time delay between each call. */
  export function setInterval(
    cb: (...args: Args) => void,
    delay?: number,
    ...args: Args
  ): number;
  export function clearTimeout(id?: number): void;
  export function clearInterval(id?: number): void;
  export function queueMicrotask(func: Function): void;
}

declare namespace __urlSearchParams {
  export class URLSearchParams {
    constructor(init?: string | string[][] | Record<string, string>);
    /** Appends a specified key/value pair as a new search parameter.
     *
     *       searchParams.append('name', 'first');
     *       searchParams.append('name', 'second');
     */
    append(name: string, value: string): void;
    /** Deletes the given search parameter and its associated value,
     * from the list of all search parameters.
     *
     *       searchParams.delete('name');
     */
    delete(name: string): void;
    /** Returns all the values associated with a given search parameter
     * as an array.
     *
     *       searchParams.getAll('name');
     */
    getAll(name: string): string[];
    /** Returns the first value associated to the given search parameter.
     *
     *       searchParams.get('name');
     */
    get(name: string): string | null;
    /** Returns a Boolean that indicates whether a parameter with the
     * specified name exists.
     *
     *       searchParams.has('name');
     */
    has(name: string): boolean;
    /** Sets the value associated with a given search parameter to the
     * given value. If there were several matching values, this method
     * deletes the others. If the search parameter doesn't exist, this
     * method creates it.
     *
     *       searchParams.set('name', 'value');
     */
    set(name: string, value: string): void;
    /** Sort all key/value pairs contained in this object in place and
     * return undefined. The sort order is according to Unicode code
     * points of the keys.
     *
     *       searchParams.sort();
     */
    sort(): void;
    /** Calls a function for each element contained in this object in
     * place and return undefined. Optionally accepts an object to use
     * as this when executing callback as second argument.
     *
     *       searchParams.forEach((value, key, parent) => {
     *         console.log(value, key, parent);
     *       });
     *
     */
    forEach(
      callbackfn: (value: string, key: string, parent: this) => void,
      thisArg?: any
    ): void;
    /** Returns an iterator allowing to go through all keys contained
     * in this object.
     *
     *       for (const key of searchParams.keys()) {
     *         console.log(key);
     *       }
     */
    keys(): IterableIterator<string>;
    /** Returns an iterator allowing to go through all values contained
     * in this object.
     *
     *       for (const value of searchParams.values()) {
     *         console.log(value);
     *       }
     */
    values(): IterableIterator<string>;
    /** Returns an iterator allowing to go through all key/value
     * pairs contained in this object.
     *
     *       for (const [key, value] of searchParams.entries()) {
     *         console.log(key, value);
     *       }
     */
    entries(): IterableIterator<[string, string]>;
    /** Returns an iterator allowing to go through all key/value
     * pairs contained in this object.
     *
     *       for (const [key, value] of searchParams[Symbol.iterator]()) {
     *         console.log(key, value);
     *       }
     */
    [Symbol.iterator](): IterableIterator<[string, string]>;
    /** Returns a query string suitable for use in a URL.
     *
     *        searchParams.toString();
     */
    toString(): string;
  }
}

declare namespace __url {
  export interface URL {
    hash: string;
    host: string;
    hostname: string;
    href: string;
    readonly origin: string;
    password: string;
    pathname: string;
    port: string;
    protocol: string;
    search: string;
    readonly searchParams: __urlSearchParams.URLSearchParams;
    username: string;
    toString(): string;
    toJSON(): string;
  }

  export const URL: {
    prototype: URL;
    new (url: string, base?: string | URL): URL;
    createObjectURL(object: __domTypes.Blob): string;
    revokeObjectURL(url: string): void;
  };
}

declare namespace __workers {
  export interface Worker {
    onerror?: (e: Event) => void;
    onmessage?: (e: { data: any }) => void;
    onmessageerror?: () => void;
    postMessage(data: any): void;
    terminate(): void;
  }
  export interface WorkerOptions {
    type?: "classic" | "module";
    name?: string;
  }
  export class WorkerImpl implements Worker {
    onerror?: (e: Event) => void;
    onmessage?: (data: any) => void;
    onmessageerror?: () => void;
    constructor(specifier: string, options?: WorkerOptions);
    postMessage(data: any): void;
    terminate(): void;
  }
}

declare namespace __performanceUtil {
  export class Performance {
    /** Returns a current time from Deno's start in milliseconds.
     *
     * Use the flag --allow-hrtime return a precise value.
     *
     *       const t = performance.now();
     *       console.log(`${t} ms since start!`);
     */
    now(): number;
  }
}

/* eslint-enable @typescript-eslint/no-unused-vars, @typescript-eslint/no-empty-interface, @typescript-eslint/no-explicit-any */
