// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  export interface RequestEvent {
    readonly request: Request;
    respondWith(r: Response | Promise<Response>): Promise<void>;
  }

  export interface HttpConn extends AsyncIterable<RequestEvent> {
    readonly rid: number;

    nextRequest(): Promise<RequestEvent | null>;
    close(): void;
  }

  export interface WebSocketUpgrade {
    response: Response;
    websocket: WebSocket;
  }

  export interface UpgradeWebSocketOptions {
    protocol?: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Used to upgrade an incoming HTTP request to a WebSocket.
   *
   * Given a request, returns a pair of WebSocket and Response. The original
   * request must be responded to with the returned response for the websocket
   * upgrade to be successful.
   *
   * ```ts
   * const conn = await Deno.connect({ port: 80, hostname: "127.0.0.1" });
   * const httpConn = Deno.serveHttp(conn);
   * const e = await httpConn.nextRequest();
   * if (e) {
   *   const { websocket, response } = Deno.upgradeWebSocket(e.request);
   *   websocket.onopen = () => {
   *     websocket.send("Hello World!");
   *   };
   *   websocket.onmessage = (e) => {
   *     console.log(e.data);
   *     websocket.close();
   *   };
   *   websocket.onclose = () => console.log("WebSocket has been closed.");
   *   websocket.onerror = (e) => console.error("WebSocket error:", e.message);
   *   e.respondWith(response);
   * }
   * ```
   *
   * If the request body is disturbed (read from) before the upgrade is
   * completed, upgrading fails.
   *
   * This operation does not yet consume the request or open the websocket. This
   * only happens once the returned response has been passed to `respondWith`.
   */
  export function upgradeWebSocket(
    request: Request,
    options?: UpgradeWebSocketOptions,
  ): WebSocketUpgrade;
}
