// Copyright 2018-2026 the Deno authors. MIT license.

import { once } from "node:events";
import https from "node:https";
import tls from "node:tls";
import type { AddressInfo } from "node:net";
import type { Duplex } from "node:stream";

import { assert, assertStrictEquals } from "@std/assert";

Deno.test({
  name: "[node/https] client upgrade reuses TLSSocket from createConnection",
  permissions: {
    net: true,
    read: [
      "tests/testdata/tls/localhost.crt",
      "tests/testdata/tls/localhost.key",
    ],
  },
}, async () => {
  const cert = Deno.readTextFileSync("tests/testdata/tls/localhost.crt");
  const key = Deno.readTextFileSync("tests/testdata/tls/localhost.key");
  let serverSocket: Duplex | undefined;

  const server = https.createServer({ cert, key });
  server.on("upgrade", (_req, socket, _head) => {
    serverSocket = socket;
    socket.write(
      "HTTP/1.1 101 Switching Protocols\r\n" +
        "Connection: Upgrade\r\n" +
        "Upgrade: websocket\r\n" +
        "\r\n",
    );
  });

  server.listen(0, "127.0.0.1");
  await once(server, "listening");

  const { port } = server.address() as AddressInfo;
  const tlsSocket = tls.connect({
    host: "127.0.0.1",
    port,
    rejectUnauthorized: false,
    servername: "localhost",
  });

  const req = https.request({
    host: "127.0.0.1",
    port,
    method: "GET",
    createConnection: () => tlsSocket,
    headers: {
      Connection: "Upgrade",
      Upgrade: "websocket",
      "Sec-WebSocket-Key": "dGhlIHNhbXBsZSBub25jZQ==",
      "Sec-WebSocket-Version": "13",
    },
  });

  const [_res, socket, _head] = await new Promise<
    [unknown, Duplex, unknown]
  >((resolve, reject) => {
    req.on("upgrade", (...args) => resolve(args));
    req.on("error", reject);
    req.end();
  });

  assertStrictEquals(socket, tlsSocket);
  // @ts-ignore TLSSocket-specific property
  assert(socket.encrypted);

  const socketClosed = once(socket, "close");
  const peerClosed = serverSocket
    ? once(serverSocket, "close")
    : Promise.resolve();
  const serverClosed = once(server, "close");
  socket.destroy();
  serverSocket?.destroy();
  server.close();
  await Promise.all([socketClosed, peerClosed, serverClosed]);
});
