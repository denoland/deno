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

export type FormDataEntryValue = DomFile | string;

export type EndingType = "transparent" | "native";

export interface BlobPropertyBag {
  type?: string;
  ending?: EndingType;
}

export interface ProgressEventInit extends EventInit {
  lengthComputable?: boolean;
  loaded?: number;
  total?: number;
}

export interface UIEventInit extends EventInit {
  detail?: number;
  // adjust Window -> Node
  view?: Node | null;
}

export class UIEvent extends Event {
  constructor(type: string, eventInitDict?: UIEventInit);
  readonly detail: number;
  // adjust Window -> Node
  readonly view: Node | null;
}

export interface FocusEventInit extends UIEventInit {
  relatedTarget?: EventTarget | null;
}

export class FocusEvent extends UIEvent {
  constructor(type: string, eventInitDict?: FocusEventInit);
  readonly relatedTarget: EventTarget | null;
}

export interface EventModifierInit extends UIEventInit {
  altKey?: boolean;
  ctrlKey?: boolean;
  metaKey?: boolean;
  modifierAltGraph?: boolean;
  modifierCapsLock?: boolean;
  modifierFn?: boolean;
  modifierFnLock?: boolean;
  modifierHyper?: boolean;
  modifierNumLock?: boolean;
  modifierScrollLock?: boolean;
  modifierSuper?: boolean;
  modifierSymbol?: boolean;
  modifierSymbolLock?: boolean;
  shiftKey?: boolean;
}

export interface MouseEventInit extends EventModifierInit {
  button?: number;
  buttons?: number;
  clientX?: number;
  clientY?: number;
  movementX?: number;
  movementY?: number;
  relatedTarget?: EventTarget | null;
  screenX?: number;
  screenY?: number;
}

export class MouseEvent extends UIEvent {
  constructor(type: string, eventInitDict?: MouseEventInit);
  readonly altKey: boolean;
  readonly button: number;
  readonly buttons: number;
  readonly clientX: number;
  readonly clientY: number;
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly movementX: number;
  readonly movementY: number;
  readonly offsetX: number;
  readonly offsetY: number;
  readonly pageX: number;
  readonly pageY: number;
  readonly relatedTarget: EventTarget | null;
  readonly screenX: number;
  readonly screenY: number;
  readonly shiftKey: boolean;
  readonly x: number;
  readonly y: number;
  getModifierState(keyArg: string): boolean;
}

interface GetRootNodeOptions {
  composed?: boolean;
}

export class Node extends EventTarget {
  readonly baseURI: string;
  readonly childNodes: NodeListOf<ChildNode>;
  readonly firstChild: ChildNode | null;
  readonly isConnected: boolean;
  readonly lastChild: ChildNode | null;
  readonly nextSibling: ChildNode | null;
  readonly nodeName: string;
  readonly nodeType: number;
  nodeValue: string | null;
  // adjusted: Document -> Node
  readonly ownerDocument: Node | null;
  // adjusted: HTMLElement -> Node
  readonly parentElement: Node | null;
  readonly parentNode: (Node & ParentNode) | null;
  readonly previousSibling: ChildNode | null;
  textContent: string | null;
  appendChild<T extends Node>(newChild: T): T;
  cloneNode(deep?: boolean): Node;
  compareDocumentPosition(other: Node): number;
  contains(other: Node | null): boolean;
  getRootNode(options?: GetRootNodeOptions): Node;
  hasChildNodes(): boolean;
  insertBefore<T extends Node>(newChild: T, refChild: Node | null): T;
  isDefaultNamespace(namespace: string | null): boolean;
  isEqualNode(otherNode: Node | null): boolean;
  isSameNode(otherNode: Node | null): boolean;
  lookupNamespaceURI(prefix: string | null): string | null;
  lookupPrefix(namespace: string | null): string | null;
  normalize(): void;
  removeChild<T extends Node>(oldChild: T): T;
  replaceChild<T extends Node>(newChild: Node, oldChild: T): T;
  readonly ATTRIBUTE_NODE: number;
  readonly CDATA_SECTION_NODE: number;
  readonly COMMENT_NODE: number;
  readonly DOCUMENT_FRAGMENT_NODE: number;
  readonly DOCUMENT_NODE: number;
  readonly DOCUMENT_POSITION_CONTAINED_BY: number;
  readonly DOCUMENT_POSITION_CONTAINS: number;
  readonly DOCUMENT_POSITION_DISCONNECTED: number;
  readonly DOCUMENT_POSITION_FOLLOWING: number;
  readonly DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: number;
  readonly DOCUMENT_POSITION_PRECEDING: number;
  readonly DOCUMENT_TYPE_NODE: number;
  readonly ELEMENT_NODE: number;
  readonly ENTITY_NODE: number;
  readonly ENTITY_REFERENCE_NODE: number;
  readonly NOTATION_NODE: number;
  readonly PROCESSING_INSTRUCTION_NODE: number;
  readonly TEXT_NODE: number;
  static readonly ATTRIBUTE_NODE: number;
  static readonly CDATA_SECTION_NODE: number;
  static readonly COMMENT_NODE: number;
  static readonly DOCUMENT_FRAGMENT_NODE: number;
  static readonly DOCUMENT_NODE: number;
  static readonly DOCUMENT_POSITION_CONTAINED_BY: number;
  static readonly DOCUMENT_POSITION_CONTAINS: number;
  static readonly DOCUMENT_POSITION_DISCONNECTED: number;
  static readonly DOCUMENT_POSITION_FOLLOWING: number;
  static readonly DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC: number;
  static readonly DOCUMENT_POSITION_PRECEDING: number;
  static readonly DOCUMENT_TYPE_NODE: number;
  static readonly ELEMENT_NODE: number;
  static readonly ENTITY_NODE: number;
  static readonly ENTITY_REFERENCE_NODE: number;
  static readonly NOTATION_NODE: number;
  static readonly PROCESSING_INSTRUCTION_NODE: number;
  static readonly TEXT_NODE: number;
}

interface Slotable {
  // adjusted: HTMLSlotElement -> Node
  readonly assignedSlot: Node | null;
}

interface ChildNode extends Node {
  after(...nodes: Array<Node | string>): void;
  before(...nodes: Array<Node | string>): void;
  remove(): void;
  replaceWith(...nodes: Array<Node | string>): void;
}

interface ParentNode {
  readonly childElementCount: number;
  // not currently supported
  // readonly children: HTMLCollection;
  // adjusted: Element -> Node
  readonly firstElementChild: Node | null;
  // adjusted: Element -> Node
  readonly lastElementChild: Node | null;
  append(...nodes: Array<Node | string>): void;
  prepend(...nodes: Array<Node | string>): void;
  // not currently supported
  // querySelector<K extends keyof HTMLElementTagNameMap>(
  //   selectors: K,
  // ): HTMLElementTagNameMap[K] | null;
  // querySelector<K extends keyof SVGElementTagNameMap>(
  //   selectors: K,
  // ): SVGElementTagNameMap[K] | null;
  // querySelector<E extends Element = Element>(selectors: string): E | null;
  // querySelectorAll<K extends keyof HTMLElementTagNameMap>(
  //   selectors: K,
  // ): NodeListOf<HTMLElementTagNameMap[K]>;
  // querySelectorAll<K extends keyof SVGElementTagNameMap>(
  //   selectors: K,
  // ): NodeListOf<SVGElementTagNameMap[K]>;
  // querySelectorAll<E extends Element = Element>(
  //   selectors: string,
  // ): NodeListOf<E>;
}

interface NodeList {
  readonly length: number;
  item(index: number): Node | null;
  forEach(
    callbackfn: (value: Node, key: number, parent: NodeList) => void,
    thisArg?: any
  ): void;
  [index: number]: Node;
  [Symbol.iterator](): IterableIterator<Node>;
  entries(): IterableIterator<[number, Node]>;
  keys(): IterableIterator<number>;
  values(): IterableIterator<Node>;
}

interface NodeListOf<TNode extends Node> extends NodeList {
  length: number;
  item(index: number): TNode;
  forEach(
    callbackfn: (value: TNode, key: number, parent: NodeListOf<TNode>) => void,
    thisArg?: any
  ): void;
  [index: number]: TNode;
  [Symbol.iterator](): IterableIterator<TNode>;
  entries(): IterableIterator<[number, TNode]>;
  keys(): IterableIterator<number>;
  values(): IterableIterator<TNode>;
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

export class FormData {
  append(name: string, value: string | Blob, fileName?: string): void;
  delete(name: string): void;
  get(name: string): FormDataEntryValue | null;
  getAll(name: string): FormDataEntryValue[];
  has(name: string): boolean;
  set(name: string, value: string | Blob, fileName?: string): void;
  [Symbol.iterator](): IterableIterator<[string, FormDataEntryValue]>;
  entries(): IterableIterator<[string, FormDataEntryValue]>;
  keys(): IterableIterator<string>;
  values(): IterableIterator<FormDataEntryValue>;
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

export interface ReadableStreamConstructor {
  new <R = any>(source?: UnderlyingSource<R>): ReadableStream<R>;
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

export class Headers {
  constructor(init?: HeadersInit);
  append(name: string, value: string): void;
  delete(name: string): void;
  get(name: string): string | null;
  has(name: string): boolean;
  set(name: string, value: string): void;
  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    thisArg?: any
  ): void;
  [Symbol.iterator](): IterableIterator<[string, string]>;
  entries(): IterableIterator<[string, string]>;
  keys(): IterableIterator<string>;
  values(): IterableIterator<string>;
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

export interface RequestConstructor {
  new (input: RequestInfo, init?: RequestInit): Request;
  prototype: Request;
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

export interface ResponseConstructor {
  prototype: Response;
  new (body?: BodyInit | null, init?: ResponseInit): Response;
  error(): Response;
  redirect(url: string, status?: number): Response;
}
