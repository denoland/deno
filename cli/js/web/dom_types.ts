// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

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

/* eslint-disable @typescript-eslint/no-explicit-any */

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

// https://dom.spec.whatwg.org/#node
export enum NodeType {
  ELEMENT_NODE = 1,
  TEXT_NODE = 3,
  DOCUMENT_FRAGMENT_NODE = 11,
}

export const eventTargetHost: unique symbol = Symbol();
export const eventTargetListeners: unique symbol = Symbol();
export const eventTargetMode: unique symbol = Symbol();
export const eventTargetNodeType: unique symbol = Symbol();

export interface EventListener {
  // Different from lib.dom.d.ts. Added Promise<void>
  (evt: Event): void | Promise<void>;
}

export interface EventListenerObject {
  // Different from lib.dom.d.ts. Added Promise<void>
  handleEvent(evt: Event): void | Promise<void>;
}

export type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

// This is actually not part of actual DOM types,
// but an implementation specific thing on our custom EventTarget
// (due to the presence of our custom symbols)
export interface EventTargetListener {
  callback: EventListenerOrEventListenerObject;
  options: AddEventListenerOptions;
}

export interface EventTarget {
  // TODO: below 4 symbol props should not present on EventTarget WebIDL.
  // They should be implementation specific details.
  [eventTargetHost]: EventTarget | null;
  [eventTargetListeners]: { [type in string]: EventTargetListener[] };
  [eventTargetMode]: string;
  [eventTargetNodeType]: NodeType;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions
  ): void;
  dispatchEvent(event: Event): boolean;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean
  ): void;
}

export interface ProgressEventInit extends EventInit {
  lengthComputable?: boolean;
  loaded?: number;
  total?: number;
}

export interface URLSearchParams extends DomIterable<string, string> {
  append(name: string, value: string): void;
  delete(name: string): void;
  get(name: string): string | null;
  getAll(name: string): string[];
  has(name: string): boolean;
  set(name: string, value: string): void;
  sort(): void;
  toString(): string;
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
  new (bits: BlobPart[], filename: string, options?: FilePropertyBag): DomFile;
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

export interface AbortSignal extends EventTarget {
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

export interface Blob {
  readonly size: number;
  readonly type: string;
  slice(start?: number, end?: number, contentType?: string): Blob;
}

export interface Body {
  readonly body: ReadableStream<Uint8Array> | null;
  readonly bodyUsed: boolean;
  arrayBuffer(): Promise<ArrayBuffer>;
  blob(): Promise<Blob>;
  formData(): Promise<FormData>;
  json(): Promise<any>;
  text(): Promise<string>;
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

export interface UnderlyingSource<R = any> {
  cancel?: ReadableStreamErrorCallback;
  pull?: ReadableStreamDefaultControllerCallback<R>;
  start?: ReadableStreamDefaultControllerCallback<R>;
  type?: undefined;
}

export interface UnderlyingByteSource {
  autoAllocateChunkSize?: number;
  cancel?: ReadableStreamErrorCallback;
  pull?: ReadableByteStreamControllerCallback;
  start?: ReadableByteStreamControllerCallback;
  type: "bytes";
}

export interface ReadableStreamReader<R = any> {
  cancel(reason: any): Promise<void>;
  read(): Promise<ReadableStreamReadResult<R>>;
  releaseLock(): void;
}

export interface ReadableStreamErrorCallback {
  (reason: any): void | PromiseLike<void>;
}

export interface ReadableByteStreamControllerCallback {
  (controller: ReadableByteStreamController): void | PromiseLike<void>;
}

export interface ReadableStreamDefaultControllerCallback<R> {
  (controller: ReadableStreamDefaultController<R>): void | PromiseLike<void>;
}

export interface ReadableStreamDefaultController<R = any> {
  readonly desiredSize: number | null;
  close(): void;
  enqueue(chunk: R): void;
  error(error?: any): void;
}

export interface ReadableByteStreamController {
  readonly byobRequest: ReadableStreamBYOBRequest | undefined;
  readonly desiredSize: number | null;
  close(): void;
  enqueue(chunk: ArrayBufferView): void;
  error(error?: any): void;
}

export interface ReadableStreamBYOBRequest {
  readonly view: ArrayBufferView;
  respond(bytesWritten: number): void;
  respondWithNewView(view: ArrayBufferView): void;
}
/* TODO reenable these interfaces.  These are needed to enable WritableStreams in js/streams/
export interface WritableStream<W = any> {
  readonly locked: boolean;
  abort(reason?: any): Promise<void>;
  getWriter(): WritableStreamDefaultWriter<W>;
}

TODO reenable these interfaces.  These are needed to enable WritableStreams in js/streams/
export interface UnderlyingSink<W = any> {
  abort?: WritableStreamErrorCallback;
  close?: WritableStreamDefaultControllerCloseCallback;
  start?: WritableStreamDefaultControllerStartCallback;
  type?: undefined;
  write?: WritableStreamDefaultControllerWriteCallback<W>;
}

export interface PipeOptions {
  preventAbort?: boolean;
  preventCancel?: boolean;
  preventClose?: boolean;
  signal?: AbortSignal;
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

export interface WritableStreamErrorCallback {
  (reason: any): void | PromiseLike<void>;
}

export interface WritableStreamDefaultControllerCloseCallback {
  (): void | PromiseLike<void>;
}

export interface WritableStreamDefaultControllerStartCallback {
  (controller: WritableStreamDefaultController): void | PromiseLike<void>;
}

export interface WritableStreamDefaultControllerWriteCallback<W> {
  (chunk: W, controller: WritableStreamDefaultController): void | PromiseLike<
    void
  >;
}

export interface WritableStreamDefaultController {
  error(error?: any): void;
}
*/
export interface QueuingStrategy<T = any> {
  highWaterMark?: number;
  size?: QueuingStrategySizeCallback<T>;
}

export interface QueuingStrategySizeCallback<T = any> {
  (chunk: T): number;
}

export interface Headers extends DomIterable<string, string> {
  append(name: string, value: string): void;
  delete(name: string): void;
  entries(): IterableIterator<[string, string]>;
  get(name: string): string | null;
  has(name: string): boolean;
  keys(): IterableIterator<string>;
  set(name: string, value: string): void;
  values(): IterableIterator<string>;
  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    thisArg?: any
  ): void;
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
type RequestRedirect = "follow" | "nofollow" | "error" | "manual";
export type ResponseType =
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
  readonly cache?: RequestCache;
  readonly credentials?: RequestCredentials;
  readonly destination?: RequestDestination;
  readonly headers: Headers;
  readonly integrity?: string;
  readonly isHistoryNavigation?: boolean;
  readonly isReloadNavigation?: boolean;
  readonly keepalive?: boolean;
  readonly method: string;
  readonly mode?: RequestMode;
  readonly redirect?: RequestRedirect;
  readonly referrer?: string;
  readonly referrerPolicy?: ReferrerPolicy;
  readonly signal?: AbortSignal;
  readonly url: string;
  clone(): Request;
}

export interface Response extends Body {
  readonly headers: Headers;
  readonly ok: boolean;
  readonly redirected: boolean;
  readonly status: number;
  readonly statusText: string;
  readonly trailer: Promise<Headers>;
  readonly type: ResponseType;
  readonly url: string;
  clone(): Response;
}

export interface DOMStringList {
  readonly length: number;
  contains(string: string): boolean;
  item(index: number): string | null;
  [index: number]: string;
}

export interface Location {
  readonly ancestorOrigins: DOMStringList;
  hash: string;
  host: string;
  hostname: string;
  href: string;
  toString(): string;
  readonly origin: string;
  pathname: string;
  port: string;
  protocol: string;
  search: string;
  assign(url: string): void;
  reload(): void;
  replace(url: string): void;
}

export interface URL {
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

export interface URLSearchParams {
  /**
   * Appends a specified key/value pair as a new search parameter.
   */
  append(name: string, value: string): void;
  /**
   * Deletes the given search parameter, and its associated value, from the list of all search parameters.
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
   * Sets the value associated to a given search parameter to the given value. If there were several values, delete the others.
   */
  set(name: string, value: string): void;
  sort(): void;
  /**
   * Returns a string containing a query string suitable for use in a URL. Does not include the question mark.
   */
  toString(): string;
  forEach(
    callbackfn: (value: string, key: string, parent: URLSearchParams) => void,
    thisArg?: any
  ): void;

  [Symbol.iterator](): IterableIterator<[string, string]>;
  /**
   * Returns an array of key, value pairs for every entry in the search params.
   */
  entries(): IterableIterator<[string, string]>;
  /**
   * Returns a list of keys in the search params.
   */
  keys(): IterableIterator<string>;
  /**
   * Returns a list of values in the search params.
   */
  values(): IterableIterator<string>;
}
