// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, Deferred, deferred } from "./test_util.ts";

function onListen<T>(
  p: Deferred<T>,
): ({ hostname, port }: { hostname: string; port: number }) => void {
  return () => {
    p.resolve();
  };
}

Deno.test(
  "should correctly return url property after websocket upgrade",
  async () => {
    const listeningPromise = deferred();
    const ac = new AbortController();
    let url;

    const server = Deno.serve(
      {
        hostname: "localhost",
        port: 4501,
        onListen: onListen(listeningPromise),
        signal: ac.signal,
      },
      (request) => {
        if (request.headers.get("upgrade") != "websocket") {
          return new Response(null, { status: 501 });
        }
        const { socket, response } = Deno.upgradeWebSocket(request);

        socket.addEventListener("open", () => {
          url = request.url;
          socket.close();
          ac.abort();
        });

        return response;
      },
    );

    new WebSocket("ws://127.0.0.1:4501/test");
    await listeningPromise;
    await server;
    assertEquals(url, "http://localhost:4501/test");
  },
);
