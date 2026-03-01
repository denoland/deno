// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals } from "@std/assert";
import { execCode } from "../unit/test_util.ts";
import { createSocket, type Socket } from "node:dgram";

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

Deno.test("[node/dgram] addMembership, setBroadcast, setMulticastTTL after bind", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();

  const socket = createSocket({ type: "udp4", reuseAddr: true });

  socket.on("error", (err) => {
    reject(err);
  });

  socket.bind(0, "0.0.0.0", () => {
    try {
      socket.addMembership("239.255.255.250");
      socket.setBroadcast(true);
      socket.setMulticastTTL(4);
      socket.dropMembership("239.255.255.250");
      resolve();
    } catch (err) {
      reject(err);
    } finally {
      socket.close();
    }
  });

  await promise;
});

Deno.test("[node/dgram] setTTL sets unicast TTL without error", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const socket = createSocket("udp4");
  socket.bind(0, () => {
    socket.setTTL(128);
    socket.close(() => resolve());
  });
  await promise;
});

Deno.test("[node/dgram] setTTL throws on invalid TTL", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const socket = createSocket("udp4");
  socket.bind(0, () => {
    try {
      socket.setTTL(0);
      assert(false, "should have thrown");
    } catch (e) {
      assert(e instanceof Error);
    }
    try {
      socket.setTTL(256);
      assert(false, "should have thrown");
    } catch (e) {
      assert(e instanceof Error);
    }
    socket.close(() => resolve());
  });
  await promise;
});

Deno.test("[node/dgram] setMulticastInterface sets interface without error", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const socket = createSocket("udp4");
  socket.bind(0, () => {
    socket.setMulticastInterface("0.0.0.0");
    socket.close(() => resolve());
  });
  await promise;
});

Deno.test("[node/dgram] addSourceSpecificMembership and dropSourceSpecificMembership", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const socket = createSocket("udp4");
  socket.bind(0, () => {
    socket.addSourceSpecificMembership("127.0.0.1", "232.1.1.1");
    socket.dropSourceSpecificMembership("127.0.0.1", "232.1.1.1");
    socket.close(() => resolve());
  });
  await promise;
});

Deno.test("[node/dgram] large recvBufferSize and sendBufferSize do not throw", async () => {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const socket = createSocket({
    type: "udp4",
    recvBufferSize: 4194304,
    sendBufferSize: 4194304,
  });
  socket.on("error", (err) => {
    reject(err);
  });
  socket.bind(0, () => {
    socket.close(() => resolve());
  });
  await promise;
});
