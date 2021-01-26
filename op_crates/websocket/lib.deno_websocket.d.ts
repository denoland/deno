// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

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
