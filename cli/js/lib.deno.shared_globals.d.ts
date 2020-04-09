// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */

/// <reference no-default-lib="true" />
// TODO: we need to remove this, but Fetch::Response::Body implements Reader
// which requires Deno.EOF, and we shouldn't be leaking that, but https_proxy
// at the least requires the Reader interface on Body, which it shouldn't
/// <reference lib="deno.ns" />
/// <reference lib="esnext" />

// https://developer.mozilla.org/en-US/docs/Web/API/WindowOrWorkerGlobalScope

declare interface WindowOrWorkerGlobalScope {
  // methods
  fetch: typeof __fetch.fetch;
  // properties
  File: __domTypes.DomFileConstructor;
  Headers: __domTypes.HeadersConstructor;
  FormData: __domTypes.FormDataConstructor;
  ReadableStream: __domTypes.ReadableStreamConstructor;
  Request: __domTypes.RequestConstructor;
  Response: typeof __fetch.Response;
  location: __domTypes.Location;
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
  function compile(bufferSource: BufferSource): Promise<Module>;

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
    bufferSource: BufferSource,
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
  function validate(bufferSource: BufferSource): boolean;

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
    constructor(bufferSource: BufferSource);

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

declare const fetch: typeof __fetch.fetch;

/** Sets a timer which executes a function once after the timer expires. */
declare function setTimeout(
  cb: (...args: unknown[]) => void,
  delay?: number,
  ...args: unknown[]
): number;
/** Repeatedly calls a function , with a fixed time delay between each call. */
declare function setInterval(
  cb: (...args: unknown[]) => void,
  delay?: number,
  ...args: unknown[]
): number;
declare function clearTimeout(id?: number): void;
declare function clearInterval(id?: number): void;
declare function queueMicrotask(func: Function): void;

declare const console: Console;
declare const File: __domTypes.DomFileConstructor;
declare const Headers: __domTypes.HeadersConstructor;
declare const location: __domTypes.Location;
declare const FormData: __domTypes.FormDataConstructor;
declare const ReadableStream: __domTypes.ReadableStreamConstructor;
declare const Request: __domTypes.RequestConstructor;
declare const Response: typeof __fetch.Response;

declare function addEventListener(
  type: string,
  callback: EventListenerOrEventListenerObject | null,
  options?: boolean | AddEventListenerOptions | undefined
): void;

declare function dispatchEvent(event: Event): boolean;

declare function removeEventListener(
  type: string,
  callback: EventListenerOrEventListenerObject | null,
  options?: boolean | EventListenerOptions | undefined
): void;

declare type Body = __domTypes.Body;
declare type File = __domTypes.DomFile;
declare type Headers = __domTypes.Headers;
declare type FormData = __domTypes.FormData;
declare type ReadableStream<R = any> = __domTypes.ReadableStream<R>;
declare type Request = __domTypes.Request;
declare type Response = __domTypes.Response;

declare interface ImportMeta {
  url: string;
  main: boolean;
}

declare namespace __domTypes {
  export type HeadersInit =
    | Headers
    | Array<[string, string]>
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

  interface AbortSignalEventMap {
    abort: ProgressEvent;
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
  export interface UnderlyingSource<R = any> {
    cancel?: ReadableStreamErrorCallback;
    pull?: ReadableStreamDefaultControllerCallback<R>;
    start?: ReadableStreamDefaultControllerCallback<R>;
    type?: undefined;
  }
  export interface ReadableStreamErrorCallback {
    (reason: any): void | PromiseLike<void>;
  }

  export interface ReadableStreamDefaultControllerCallback<R> {
    (controller: ReadableStreamDefaultController<R>): void | PromiseLike<void>;
  }

  export interface ReadableStreamDefaultController<R> {
    readonly desiredSize: number;
    enqueue(chunk?: R): void;
    close(): void;
    error(e?: any): void;
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

  export interface ReadableStreamConstructor<R = any> {
    new (src?: UnderlyingSource<R>): ReadableStream<R>;
    prototype: ReadableStream<R>;
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

type BufferSource = ArrayBufferView | ArrayBuffer;
type BlobPart = BufferSource | Blob | string;

interface BlobPropertyBag {
  type?: string;
  ending?: "transparent" | "native";
}

/** A file-like object of immutable, raw data. Blobs represent data that isn't necessarily in a JavaScript-native format. The File interface is based on Blob, inheriting blob functionality and expanding it to support files on the user's system. */
interface Blob {
  readonly size: number;
  readonly type: string;
  arrayBuffer(): Promise<ArrayBuffer>;
  slice(start?: number, end?: number, contentType?: string): Blob;
  stream(): ReadableStream;
  text(): Promise<string>;
}

declare const Blob: {
  prototype: Blob;
  new (blobParts?: BlobPart[], options?: BlobPropertyBag): Blob;
};

declare const isConsoleInstance: unique symbol;

declare class Console {
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

declare namespace __fetch {
  class Body
    implements
      __domTypes.Body,
      __domTypes.ReadableStream<Uint8Array>,
      Deno.ReadCloser {
    readonly contentType: string;
    bodyUsed: boolean;
    readonly locked: boolean;
    readonly body: __domTypes.ReadableStream<Uint8Array>;
    constructor(rid: number, contentType: string);
    arrayBuffer(): Promise<ArrayBuffer>;
    blob(): Promise<Blob>;
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
    blob(): Promise<Blob>;
    formData(): Promise<__domTypes.FormData>;
    json(): Promise<any>;
    text(): Promise<string>;
    readonly ok: boolean;
    clone(): __domTypes.Response;
    redirect(url: URL | string, status: number): __domTypes.Response;
  }
  /** Fetch a resource from the network. */
  export function fetch(
    input: __domTypes.Request | URL | string,
    init?: __domTypes.RequestInit
  ): Promise<Response>;
}

declare function atob(s: string): string;

/** Creates a base-64 ASCII string from the input string. */
declare function btoa(s: string): string;

declare class TextDecoder {
  /** Returns encoding's name, lowercased. */
  readonly encoding: string;
  /** Returns `true` if error mode is "fatal", and `false` otherwise. */
  readonly fatal: boolean;
  /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
  readonly ignoreBOM = false;
  constructor(
    label?: string,
    options?: { fatal?: boolean; ignoreBOM?: boolean }
  );
  /** Returns the result of running encoding's decoder. */
  decode(input?: BufferSource, options?: { stream?: false }): string;
  readonly [Symbol.toStringTag]: string;
}

declare class TextEncoder {
  /** Returns "utf-8". */
  readonly encoding = "utf-8";
  /** Returns the result of running UTF-8's encoder. */
  encode(input?: string): Uint8Array;
  encodeInto(
    input: string,
    dest: Uint8Array
  ): { read: number; written: number };
  readonly [Symbol.toStringTag]: string;
}

interface URLSearchParams {
  /** Appends a specified key/value pair as a new search parameter.
   *
   *       let searchParams = new URLSearchParams();
   *       searchParams.append('name', 'first');
   *       searchParams.append('name', 'second');
   */
  append(name: string, value: string): void;

  /** Deletes the given search parameter and its associated value,
   * from the list of all search parameters.
   *
   *       let searchParams = new URLSearchParams([['name', 'value']]);
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
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       params.forEach((value, key, parent) => {
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
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const key of params.keys()) {
   *         console.log(key);
   *       }
   */
  keys(): IterableIterator<string>;

  /** Returns an iterator allowing to go through all values contained
   * in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const value of params.values()) {
   *         console.log(value);
   *       }
   */
  values(): IterableIterator<string>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const [key, value] of params.entries()) {
   *         console.log(key, value);
   *       }
   */
  entries(): IterableIterator<[string, string]>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const [key, value] of params) {
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

declare const URLSearchParams: {
  prototype: URLSearchParams;
  new (
    init?: string[][] | Record<string, string> | string | URLSearchParams
  ): URLSearchParams;
  toString(): string;
};

/** The URL interface represents an object providing static methods used for creating object URLs. */
interface URL {
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

declare const URL: {
  prototype: URL;
  new (url: string, base?: string | URL): URL;
  createObjectURL(object: any): string;
  revokeObjectURL(url: string): void;
};

declare class Worker {
  onerror?: (e: Event) => void;
  onmessage?: (data: any) => void;
  onmessageerror?: () => void;
  constructor(
    specifier: string,
    options?: {
      type?: "classic" | "module";
      name?: string;
    }
  );
  postMessage(data: any): void;
  terminate(): void;
}

declare namespace performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  export function now(): number;
}

/** An event which takes place in the DOM. */
interface Event {
  /**
   * Returns true or false depending on how event was initialized. True if
   * event goes through its target's ancestors in reverse tree order, and
   * false otherwise.
   */
  readonly bubbles: boolean;

  // TODO(ry) Remove cancelBubbleImmediately - non-standard extension.
  cancelBubbleImmediately: boolean;

  cancelBubble: boolean;
  /**
   * Returns true or false depending on how event was initialized. Its return
   * value does not always carry meaning, but true can indicate that part of
   * the operation during which event was dispatched, can be canceled by
   * invoking the preventDefault() method.
   */
  readonly cancelable: boolean;
  /**
   * Returns true or false depending on how event was initialized. True if
   * event invokes listeners past a ShadowRoot node that is the root of its
   * target, and false otherwise.
   */
  readonly composed: boolean;
  /**
   * Returns the object whose event listener's callback is currently being
   * invoked.
   */
  readonly currentTarget: EventTarget | null;
  /**
   * Returns true if preventDefault() was invoked successfully to indicate
   * cancelation, and false otherwise.
   */
  readonly defaultPrevented: boolean;
  /**
   * Returns the event's phase, which is one of NONE, CAPTURING_PHASE,
   * AT_TARGET, and BUBBLING_PHASE.
   */
  readonly eventPhase: number;
  /**
   * Returns true if event was dispatched by the user agent, and false
   * otherwise.
   */
  readonly isTrusted: boolean;
  returnValue: boolean;
  /** @deprecated */
  readonly srcElement: EventTarget | null;
  /**
   * Returns the object to which event is dispatched (its target).
   */
  readonly target: EventTarget | null;
  /**
   * Returns the event's timestamp as the number of milliseconds measured
   * relative to the time origin.
   */
  readonly timeStamp: number;
  /**
   * Returns the type of event, e.g. "click", "hashchange", or "submit".
   */
  readonly type: string;
  /**
   * Returns the invocation target objects of event's path (objects on which
   * listeners will be invoked), except for any nodes in shadow trees of which
   * the shadow root's mode is "closed" that are not reachable from event's
   * currentTarget.
   */
  composedPath(): EventTarget[];
  initEvent(type: string, bubbles?: boolean, cancelable?: boolean): void;
  /**
   * If invoked when the cancelable attribute value is true, and while
   * executing a listener for the event with passive set to false, signals to
   * the operation that caused event to be dispatched that it needs to be
   * canceled.
   */
  preventDefault(): void;
  /**
   * Invoking this method prevents event from reaching any registered event
   * listeners after the current one finishes running and, when dispatched in
   * a tree, also prevents event from reaching any other objects.
   */
  stopImmediatePropagation(): void;
  /**
   * When dispatched in a tree, invoking this method prevents event from
   * reaching any objects other than the current object.
   */
  stopPropagation(): void;
  readonly AT_TARGET: number;
  readonly BUBBLING_PHASE: number;
  readonly CAPTURING_PHASE: number;
  readonly NONE: number;
}

interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

declare const Event: {
  prototype: Event;
  new (type: string, eventInitDict?: EventInit): Event;
  readonly AT_TARGET: number;
  readonly BUBBLING_PHASE: number;
  readonly CAPTURING_PHASE: number;
  readonly NONE: number;
};

/**
 * EventTarget is a DOM interface implemented by objects that can receive events
 * and may have listeners for them.
 */
interface EventTarget {
  /**
   * Appends an event listener for events whose type attribute value is type.
   * The callback argument sets the callback that will be invoked when the event
   * is dispatched.
   *
   * The options argument sets listener-specific options. For compatibility this
   * can be a boolean, in which case the method behaves exactly as if the value
   * was specified as options's capture.
   *
   * When set to true, options's capture prevents callback from being invoked
   * when the event's eventPhase attribute value is BUBBLING_PHASE. When false
   * (or not present), callback will not be invoked when event's eventPhase
   * attribute value is CAPTURING_PHASE. Either way, callback will be invoked if
   * event's eventPhase attribute value is AT_TARGET.
   *
   * When set to true, options's passive indicates that the callback will not
   * cancel the event by invoking preventDefault(). This is used to enable
   * performance optimizations described in § 2.8 Observing event listeners.
   *
   * When set to true, options's once indicates that the callback will only be
   * invoked once after which the event listener will be removed.
   *
   * The event listener is appended to target's event listener list and is not
   * appended if it has the same type, callback, and capture.
   */
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions
  ): void;
  /**
   * Dispatches a synthetic event event to target and returns true if either
   * event's cancelable attribute value is false or its preventDefault() method
   * was not invoked, and false otherwise.
   */
  dispatchEvent(event: Event): boolean;
  /**
   * Removes the event listener in target's event listener list with the same
   * type, callback, and options.
   */
  removeEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean
  ): void;
}

declare const EventTarget: {
  prototype: EventTarget;
  new (): EventTarget;
};

interface EventListener {
  (evt: Event): void;
}

interface EventListenerObject {
  handleEvent(evt: Event): void;
}

declare type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

interface AddEventListenerOptions extends EventListenerOptions {
  once?: boolean;
  passive?: boolean;
}

interface EventListenerOptions {
  capture?: boolean;
}

/** Events measuring progress of an underlying process, like an HTTP request
 * (for an XMLHttpRequest, or the loading of the underlying resource of an
 * <img>, <audio>, <video>, <style> or <link>). */
interface ProgressEvent<T extends EventTarget = EventTarget> extends Event {
  readonly lengthComputable: boolean;
  readonly loaded: number;
  readonly target: T | null;
  readonly total: number;
}

interface CustomEvent<T = any> extends Event {
  /**
   * Returns any custom data event was created with. Typically used for synthetic events.
   */
  readonly detail: T;
  initCustomEvent(
    typeArg: string,
    canBubbleArg: boolean,
    cancelableArg: boolean,
    detailArg: T
  ): void;
}

interface CustomEventInit<T = any> extends EventInit {
  detail?: T;
}

declare const CustomEvent: {
  prototype: CustomEvent;
  new <T>(typeArg: string, eventInitDict?: CustomEventInit<T>): CustomEvent<T>;
};
