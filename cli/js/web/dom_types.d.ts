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

export type RequestInfo = Request | string;

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

export interface Body {
  readonly body: ReadableStream<Uint8Array> | null;
  readonly bodyUsed: boolean;
  arrayBuffer(): Promise<ArrayBuffer>;
  blob(): Promise<Blob>;
  formData(): Promise<FormData>;
  json(): Promise<any>;
  text(): Promise<string>;
}

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
