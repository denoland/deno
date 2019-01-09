/*! ****************************************************************************
Copyright (c) Microsoft Corporation. All rights reserved.
Licensed under the Apache License, Version 2.0 (the "License"); you may not use
this file except in compliance with the License. You may obtain a copy of the
License at http://www.apache.org/licenses/LICENSE-2.0

THIS CODE IS PROVIDED ON AN *AS IS* BASIS, WITHOUT WARRANTIES OR CONDITIONS OF
ANY KIND, EITHER EXPRESS OR IMPLIED, INCLUDING WITHOUT LIMITATION ANY IMPLIED
WARRANTIES OR CONDITIONS OF TITLE, FITNESS FOR A PARTICULAR PURPOSE,
MERCHANTABLITY OR NON-INFRINGEMENT.

See the Apache Version 2.0 License for specific language governing permissions
and limitations under the License.
*******************************************************************************/

export type BufferSource = ArrayBufferView | ArrayBuffer;

export type HeadersInit =
  | Headers
  | Array<[string, string]>
  | Record<string, string>;
export type URLSearchParamsInit = string | string[][] | Record<string, string>;
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
export type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

export interface DomIterable<K, V> {
  keys(): IterableIterator<K>;
  values(): IterableIterator<V>;
  entries(): IterableIterator<[K, V]>;
  [Symbol.iterator](): IterableIterator<[K, V]>;
  forEach(
    callback: (value: V, key: K, parent: this) => void,
    // tslint:disable-next-line:no-any
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

export interface EventTarget {
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions
  ): void;
  dispatchEvent(evt: Event): boolean;
  removeEventListener(
    type: string,
    listener?: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean
  ): void;
}

export interface ProgressEventInit extends EventInit {
  lengthComputable?: boolean;
  loaded?: number;
  total?: number;
}

export interface URLSearchParams {
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
    callbackfn: (value: string, key: string, parent: URLSearchParams) => void,
    // tslint:disable-next-line:no-any
    thisArg?: any
  ): void;
}

export interface EventListener {
  (evt: Event): void;
}

export interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

export enum EventPhase {
  NONE = 0,
  CAPTURING_PHASE = 1,
  AT_TARGET = 2,
  BUBBLING_PHASE = 3
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
  readonly target: EventTarget | null;
  readonly currentTarget: EventTarget | null;
  composedPath(): EventPath[];

  readonly eventPhase: number;

  stopPropagation(): void;
  stopImmediatePropagation(): void;

  readonly bubbles: boolean;
  readonly cancelable: boolean;
  preventDefault(): void;
  readonly defaultPrevented: boolean;
  readonly composed: boolean;

  readonly isTrusted: boolean;
  readonly timeStamp: Date;
}

/* TODO(ry) Re-expose this interface. There is currently some interference
 * between deno's File and this one.
 */
export interface DomFile extends Blob {
  readonly lastModified: number;
  readonly name: string;
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
  // tslint:disable-next-line:no-any
  onabort: ((this: AbortSignal, ev: ProgressEvent) => any) | null;
  addEventListener<K extends keyof AbortSignalEventMap>(
    type: K,
    // tslint:disable-next-line:no-any
    listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions
  ): void;
  removeEventListener<K extends keyof AbortSignalEventMap>(
    type: K,
    // tslint:disable-next-line:no-any
    listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
    options?: boolean | EventListenerOptions
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions
  ): void;
}

export interface ReadableStream {
  readonly locked: boolean;
  cancel(): Promise<void>;
  getReader(): ReadableStreamReader;
}

export interface EventListenerObject {
  handleEvent(evt: Event): void;
}

export interface ReadableStreamReader {
  cancel(): Promise<void>;
  // tslint:disable-next-line:no-any
  read(): Promise<any>;
  releaseLock(): void;
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
  readonly body: ReadableStream | null;
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
  // tslint:disable-next-line:no-any
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
    // tslint:disable-next-line:no-any
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
  // tslint:disable-next-line:no-any
  window?: any;
}

export interface ResponseInit {
  headers?: HeadersInit;
  status?: number;
  statusText?: string;
}

export interface Request extends Body {
  /** Returns the cache mode associated with request, which is a string
   * indicating how the the request will interact with the browser's cache when
   * fetching.
   */
  readonly cache: RequestCache;
  /** Returns the credentials mode associated with request, which is a string
   * indicating whether credentials will be sent with the request always, never,
   * or only when sent to a same-origin URL.
   */
  readonly credentials: RequestCredentials;
  /** Returns the kind of resource requested by request, (e.g., `document` or
   * `script`).
   */
  readonly destination: RequestDestination;
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
  readonly integrity: string;
  /** Returns a boolean indicating whether or not request is for a history
   * navigation (a.k.a. back-forward navigation).
   */
  readonly isHistoryNavigation: boolean;
  /** Returns a boolean indicating whether or not request is for a reload
   * navigation.
   */
  readonly isReloadNavigation: boolean;
  /** Returns a boolean indicating whether or not request can outlive the global
   * in which it was created.
   */
  readonly keepalive: boolean;
  /** Returns request's HTTP method, which is `GET` by default. */
  readonly method: string;
  /** Returns the mode associated with request, which is a string indicating
   * whether the request will use CORS, or will be restricted to same-origin
   * URLs.
   */
  readonly mode: RequestMode;
  /** Returns the redirect mode associated with request, which is a string
   * indicating how redirects for the request will be handled during fetching.
   *
   * A request will follow redirects by default.
   */
  readonly redirect: RequestRedirect;
  /** Returns the referrer of request. Its value can be a same-origin URL if
   * explicitly set in init, the empty string to indicate no referrer, and
   * `about:client` when defaulting to the global's default.
   *
   * This is used during fetching to determine the value of the `Referer`
   * header of the request being made.
   */
  readonly referrer: string;
  /** Returns the referrer policy associated with request. This is used during
   * fetching to compute the value of the request's referrer.
   */
  readonly referrerPolicy: ReferrerPolicy;
  /** Returns the signal associated with request, which is an AbortSignal object
   * indicating whether or not request has been aborted, and its abort event
   * handler.
   */
  readonly signal: AbortSignal;
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
