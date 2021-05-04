// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare interface WebSocketStreamOptions {
  protocols?: string[];
  signal?: AbortSignal;
}

declare interface WebSocketConnection {
  readable: ReadableStream<string | Uint8Array>;
  writable: WritableStream<string | Uint8Array>;
  extensions: string;
  protocol: string;
}

declare interface WebSocketCloseInfo {
  code?: number;
  reason?: string;
}

declare class WebSocketStream {
  constructor(url: string, options?: WebSocketStreamOptions);
  url: string;
  connection: Promise<WebSocketConnection>;
  closed: Promise<WebSocketCloseInfo>;
  close(closeInfo?: WebSocketCloseInfo): void;
}
