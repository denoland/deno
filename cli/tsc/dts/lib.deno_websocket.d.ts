// Copyright 2018-2025 the Deno authors. MIT license.

// deno-lint-ignore-file no-explicit-any no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/**
 * Configuration options for a `WebSocket` "close" event.
 *
 * @example
 * ```ts
 * // Creating a custom close event with specific parameters
 * const closeEventInit: CloseEventInit = {
 *   code: 1000,
 *   reason: "Normal closure",
 *   wasClean: true,
 * };
 * const event = new CloseEvent("close", closeEventInit);
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/CloseEvent/CloseEvent
 * @category WebSockets
 */
interface CloseEventInit extends EventInit {
  code?: number;
  reason?: string;
  wasClean?: boolean;
}

/**
 * The `CloseEvent` interface represents an event that occurs when a `WebSocket` connection is closed.
 *
 * This event is sent to the client when the connection is closed, providing information about
 * why the connection was closed through the `code`, `reason`, and `wasClean` properties.
 *
 * @example
 * ```ts
 * // Handling a close event
 * ws.addEventListener("close", (event: CloseEvent) => {
 *   console.log(`Connection closed with code ${event.code}`);
 *   console.log(`Reason: ${event.reason}`);
 *   console.log(`Clean close: ${event.wasClean}`);
 *
 *   if (event.code === 1006) {
 *     console.log("Connection closed abnormally");
 *   }
 * });
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/CloseEvent
 * @category WebSockets
 */
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

/**
 * Constructor interface for creating `CloseEvent` instances.
 *
 * @example
 * ```ts
 * // Creating a custom close event
 * const event = new CloseEvent("close", {
 *   code: 1000,
 *   reason: "Normal closure",
 *   wasClean: true,
 * });
 *
 * // Dispatching the event
 * myWebSocket.dispatchEvent(event);
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/CloseEvent/CloseEvent
 * @category WebSockets
 */
declare var CloseEvent: {
  readonly prototype: CloseEvent;
  new (type: string, eventInitDict?: CloseEventInit): CloseEvent;
};

/**
 * Interface mapping `WebSocket` event names to their corresponding event types.
 * Used for strongly typed event handling with `addEventListener` and `removeEventListener`.
 *
 * @example
 * ```ts
 * // Using with TypeScript for strongly-typed event handling
 * const ws = new WebSocket("ws://localhost:8080");
 *
 * ws.addEventListener("open", (event) => {
 *   console.log("Connection established");
 * });
 *
 * ws.addEventListener("message", (event: MessageEvent) => {
 *   console.log(`Received: ${event.data}`);
 * });
 * ```
 *
 * @category WebSockets
 */
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
 * @example
 * ```ts
 * // Creating a WebSocket connection
 * const ws = new WebSocket("ws://localhost:8080");
 *
 * // Setting up event handlers
 * ws.onopen = (event) => {
 *   console.log("Connected to the server");
 *   ws.send("Hello Server!");
 * };
 *
 * ws.onmessage = (event) => {
 *   console.log(`Received: ${event.data}`);
 * };
 *
 * ws.onerror = (event) => {
 *   console.error("WebSocket error observed:", event);
 * };
 *
 * ws.onclose = (event) => {
 *   console.log(`WebSocket closed: Code=${event.code}, Reason=${event.reason}`);
 * };
 * ```
 *
 * @see https://developer.mozilla.org/docs/Web/API/WebSocket
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

/**
 * Constructor interface for creating `WebSocket` instances.
 *
 * The `WebSocket` constructor creates and returns a new `WebSocket` object
 * that represents a connection to a `WebSocket` server.
 *
 * @example
 * ```ts
 * // Basic WebSocket connection
 * const ws = new WebSocket("ws://localhost:8080");
 *
 * // WebSocket with protocol specification
 * const wsWithProtocol = new WebSocket("ws://localhost:8080", "json");
 *
 * // WebSocket with multiple protocol options (server will select one)
 * const wsWithProtocols = new WebSocket("ws://localhost:8080", ["json", "xml"]);
 *
 * // Using URL object instead of string
 * const url = new URL("ws://localhost:8080/path");
 * const wsWithUrl = new WebSocket(url);
 *
 * // WebSocket with headers
 * const wsWithProtocols = new WebSocket("ws://localhost:8080", {
 *   headers: {
 *     "Authorization": "Bearer foo",
 *   },
 * });
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/WebSocket
 * @category WebSockets
 */
declare var WebSocket: {
  readonly prototype: WebSocket;
  new (
    url: string | URL,
    protocolsOrOptions?: string | string[] | WebSocketOptions,
  ): WebSocket;
  readonly CLOSED: number;
  readonly CLOSING: number;
  readonly CONNECTING: number;
  readonly OPEN: number;
};

/**
 * Options for a WebSocket instance.
 * This feature is non-standard.
 *
 * @category WebSockets
 */
interface WebSocketOptions {
  /**
   * The sub-protocol(s) that the client would like to use, in order of preference.
   */
  protocols?: string | string[];
  /**
   * A Headers object, an object literal, or an array of two-item arrays to set handshake's headers.
   * This feature is non-standard.
   */
  headers?: HeadersInit;
  /**
   * An `HttpClient` instance to use when creating the WebSocket connection.
   * This is useful when you need to connect through a proxy or customize TLS settings.
   *
   * ```ts
   * const client = Deno.createHttpClient({
   *   proxy: {
   *     transport: "unix",
   *     path: "/path/to/socket",
   *   },
   * });
   *
   * const ws = new WebSocket("ws://localhost:8000/socket", { client });
   * ```
   *
   * @experimental
   */
  client?: Deno.HttpClient;
}

/**
 * Specifies the type of binary data being received over a `WebSocket` connection.
 *
 * - `"blob"`: Binary data is returned as `Blob` objects
 * - `"arraybuffer"`: Binary data is returned as `ArrayBuffer` objects
 *
 * @example
 * ```ts
 * // Setting up WebSocket for binary data as ArrayBuffer
 * const ws = new WebSocket("ws://localhost:8080");
 * ws.binaryType = "arraybuffer";
 *
 * ws.onmessage = (event) => {
 *   if (event.data instanceof ArrayBuffer) {
 *     // Process binary data
 *     const view = new Uint8Array(event.data);
 *     console.log(`Received binary data of ${view.length} bytes`);
 *   } else {
 *     // Process text data
 *     console.log(`Received text: ${event.data}`);
 *   }
 * };
 *
 * // Sending binary data
 * const binaryData = new Uint8Array([1, 2, 3, 4]);
 * ws.send(binaryData.buffer);
 * ```
 *
 * @see https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/binaryType
 * @category WebSockets
 */
type BinaryType = "arraybuffer" | "blob";
