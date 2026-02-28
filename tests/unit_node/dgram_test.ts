// Copyright 2018-2026 the Deno authors. MIT license.

import { assertEquals, assertStrictEquals } from "@std/assert";
import { execCode } from "../unit/test_util.ts";
import { createSocket, type Socket } from "node:dgram";
import { networkInterfaces } from "node:os";

const listenPort = 4503;
const listenPort2 = 4504;

Deno.test("[node/dgram] udp ref and unref", {
  permissions: { read: true, run: true, net: true },
}, async () => {
  const { promise, resolve } = Promise.withResolvers<void>();

  const udpSocket = createSocket("udp4");
  udpSocket.bind(listenPort);

  udpSocket.unref();
  udpSocket.ref();

  let data;
  udpSocket.on("message", (buffer, _rinfo) => {
    data = Uint8Array.from(buffer);
    udpSocket.close();
  });
  udpSocket.on("close", () => {
    resolve();
  });

  const conn = await Deno.listenDatagram({
    port: listenPort2,
    transport: "udp",
  });
  await conn.send(new Uint8Array([0, 1, 2, 3]), {
    transport: "udp",
    port: listenPort,
    hostname: "127.0.0.1",
  });

  await promise;
  conn.close();
  assertEquals(data, new Uint8Array([0, 1, 2, 3]));
});

Deno.test("[node/dgram] udp unref", {
  permissions: { read: true, run: true, net: true },
}, async () => {
  const [statusCode, _output] = await execCode(`
      import { createSocket } from "node:dgram";
      const udpSocket = createSocket('udp4');
      udpSocket.bind(${listenPort2});
      // This should let the program to exit without waiting for the
      // udp socket to close.
      udpSocket.unref();
      udpSocket.on('message', (buffer, rinfo) => {
      });
    `);
  assertEquals(statusCode, 0);
});

Deno.test("[node/dgram] createSocket, reuseAddr option", async () => {
  const { promise, resolve } = Promise.withResolvers<string>();
  const socket0 = createSocket({ type: "udp4", reuseAddr: true });
  let socket1: Socket | undefined;
  socket0.bind(0, "0.0.0.0", () => {
    const port = socket0.address().port;
    socket1 = createSocket({ type: "udp4", reuseAddr: true });
    socket1.bind(port, "0.0.0.0", () => {
      const socket = createSocket({ type: "udp4" });
      socket.send("hello", port, "localhost", () => {
        socket.close();
      });
    });
    socket1.on("message", (msg) => {
      resolve(msg.toString());
    });
  });
  socket0.on("message", (msg) => {
    resolve(msg.toString());
  });
  assertEquals(await promise, "hello");
  socket0.close();
  socket1?.close();
});

// https://github.com/denoland/deno/issues/24694
Deno.test({
  name: "[node/dgram] udp6 link-local address with scope ID",
  permissions: { net: true, sys: ["networkInterfaces"] },
  ignore: Deno.build.os === "windows",
  async fn() {
    // Find a link-local IPv6 interface
    let iface: { address: string; ifname: string } | undefined;
    for (
      const [ifname, entries] of Object.entries(networkInterfaces())
    ) {
      for (const entry of entries!) {
        if (entry.family === "IPv6" && entry.address.startsWith("fe80:")) {
          iface = { address: entry.address, ifname };
          break;
        }
      }
      if (iface) break;
    }
    if (!iface) return; // No link-local IPv6 interface available

    const address = `${iface.address}%${iface.ifname}`;
    const message = "Hello, local world!";

    const { promise, resolve, reject } = Promise.withResolvers<void>();
    const client = createSocket("udp6");
    const server = createSocket("udp6");

    const timer = setTimeout(() => {
      reject(new Error("Timed out"));
      server.close();
      client.close();
    }, 5000);

    server.on("listening", () => {
      const port = server.address().port;
      client.send(message, 0, message.length, port, address);
    });

    server.on("message", (buf, info) => {
      clearTimeout(timer);
      try {
        assertStrictEquals(buf.toString(), message);
        // The remote address should include the scope ID
        assertStrictEquals(info.address, address);
        resolve();
      } catch (e) {
        reject(e);
      }
      server.close();
      client.close();
    });

    server.bind({ address });
    await promise;
  },
});
