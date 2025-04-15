// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category WebSockets */
interface CloseEventInit extends EventInit {
  code?: number;
  reason?: string;
  wasClean?: boolean;
}

/** @category WebSockets */
interface CloseEvent extends Event {
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

/** @category WebSockets */
declare var CloseEvent: {
  readonly prototype: CloseEvent;
  new (type: string, eventInitDict?: CloseEventInit): CloseEvent;
};

/** @category WebSockets */
interface WebSocketEventMap {
  close: CloseEvent;
  error: Event;
  message: MessageEvent;
  open: Event;
}

/**
 * Provides the API for creating and managing a WebSocket connection to a
 * server, as well as for sending and receiving data on the connection.
 *
 * If you are looking to create a WebSocket server, please take a look at
 * `Deno.upgradeWebSocket()`.
 *
 * @see https://developer.mozilla.org/docs/Web/API/WebSocket
 *
 * @tags allow-net
 * @category WebSockets
 */
interface WebSocket extends EventTarget {
  /**
   * Returns a string that indicates how binary data from the WebSocket object is exposed to scripts:
   *
   * Can be set, to change how binary data is returned. The default is "blob".
   *
   * ```ts
   * const ws = new WebSocket("ws://localhost:8080");
   * ws.binaryType = "arraybuffer";
   * ```
   */
  binaryType: BinaryType;
  /**
   * Returns the number of bytes of application data (UTF-8 text and binary data) that have been queued using send() but not yet been transmitted to the network.
   *
   * If the WebSocket connection is closed, this attribute's value will only increase with each call to the send() method. (The number does not reset to zero once the connection closes.)
   *
   * ```ts
   * const ws = new WebSocket("ws://localhost:8080");
   * ws.send("Hello, world!");
   * console.log(ws.bufferedAmount); // 13
   * ```
   */
  readonly bufferedAmount: number;
  /**
   * Returns the extensions selected by the server, if any.
   *
   * WebSocket extensions add optional features negotiated during the handshake via
   * the `Sec-WebSocket-Extensions` header.
   *
   * At the time of writing, there are two registered extensions:
   *
   * - [`permessage-deflate`](https://www.rfc-editor.org/rfc/rfc7692.html): Enables per-message compression using DEFLATE.
   * - [`bbf-usp-protocol`](https://usp.technology/): Used by the Broadband Forum's User Services Platform (USP).
   *
   * See the full list at [IANA WebSocket Extensions](https://www.iana.org/assignments/websocket/websocket.xml#extension-name).
   *
   * Example:
   *
   * ```ts
   * const ws = new WebSocket("ws://localhost:8080");
   * console.log(ws.extensions); // e.g., "permessage-deflate"
   * ```
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
   * Closes the WebSocket connection, optionally using code as the WebSocket connection close code and reason as the WebSocket connection close reason.
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

/** @category WebSockets */
declare var WebSocket: {
  readonly prototype: WebSocket;
  new (url: string | URL, protocols?: string | string[]): WebSocket;
  readonly CLOSED: number;
  readonly CLOSING: number;
  readonly CONNECTING: number;
  readonly OPEN: number;
};

/** @category WebSockets */
type BinaryType = "arraybuffer" | "blob";
