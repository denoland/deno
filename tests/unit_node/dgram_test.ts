// Copyright 2018-2026 the Deno authors. MIT license.

import { assert, assertEquals } from "@std/assert";
import { execCode } from "../unit/test_util.ts";
import { createSocket, type Socket } from "node:dgram";

const listenPort = 4503;
const listenPort2 = 4504;

function bindSocket(socket: Socket, hostname: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const onError = (error: Error) => {
      socket.off("error", onError);
      reject(error);
    };
    socket.once("error", onError);
    socket.bind(0, hostname, () => {
      socket.off("error", onError);
      resolve();
    });
  });
}

function connectSocket(
  socket: Socket,
  port: number,
  hostname: string,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };
    const onConnect = () => {
      cleanup();
      resolve();
    };
    const cleanup = () => {
      socket.off("error", onError);
      socket.off("connect", onConnect);
    };
    socket.once("error", onError);
    socket.once("connect", onConnect);
    socket.connect(port, hostname);
  });
}

function receiveMessage(socket: Socket): Promise<string> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error("timed out waiting for a UDP message"));
    }, 5_000);
    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };
    const onMessage = (message: Uint8Array) => {
      cleanup();
      resolve(new TextDecoder().decode(message));
    };
    const cleanup = () => {
      clearTimeout(timer);
      socket.off("error", onError);
      socket.off("message", onMessage);
    };
    socket.once("error", onError);
    socket.once("message", onMessage);
  });
}

function sendTo(
  socket: Socket,
  message: string,
  port: number,
  hostname: string,
): Promise<void> {
  return new Promise((resolve, reject) => {
    socket.send(message, port, hostname, (error) => {
      if (error) {
        reject(error);
      } else {
        resolve();
      }
    });
  });
}

function sendConnected(socket: Socket, message: string): Promise<void> {
  return new Promise((resolve, reject) => {
    socket.send(message, (error) => {
      if (error) {
        reject(error);
      } else {
        resolve();
      }
    });
  });
}

async function closeSockets(...sockets: Socket[]): Promise<void> {
  await Promise.all(sockets.map((socket) =>
    new Promise<void>((resolve) => {
      try {
        socket.close(() => resolve());
      } catch {
        resolve();
      }
    })
  ));
}

async function connectedUdpPeerTest(
  type: "udp4" | "udp6",
  hostname: string,
): Promise<void> {
  const options = type === "udp6" ? { type, ipv6Only: true } : { type };
  const client = createSocket(options);
  const firstPeer = createSocket(options);
  const secondPeer = createSocket(options);

  try {
    try {
      await Promise.all([
        bindSocket(client, hostname),
        bindSocket(firstPeer, hostname),
        bindSocket(secondPeer, hostname),
      ]);
    } catch (error) {
      const code = (error as Error & { code?: string }).code;
      if (
        type === "udp6" &&
        (code === "EAFNOSUPPORT" || code === "EADDRNOTAVAIL")
      ) {
        return;
      }
      throw error;
    }

    const clientPort = client.address().port;
    const firstPeerPort = firstPeer.address().port;
    const secondPeerPort = secondPeer.address().port;

    await connectSocket(client, firstPeerPort, hostname);
    assertEquals(client.remoteAddress().port, firstPeerPort);

    const receivedFromFirstPeer = receiveMessage(client);
    for (let i = 0; i < 8; i++) {
      await sendTo(secondPeer, `other-${i}`, clientPort, hostname);
    }
    await sendTo(firstPeer, "first-peer", clientPort, hostname);
    assertEquals(await receivedFromFirstPeer, "first-peer");

    const firstPeerReceived = receiveMessage(firstPeer);
    await sendConnected(client, "connected-send");
    assertEquals(await firstPeerReceived, "connected-send");

    client.disconnect();
    const receivedAfterDisconnect = receiveMessage(client);
    await sendTo(secondPeer, "after-disconnect", clientPort, hostname);
    assertEquals(await receivedAfterDisconnect, "after-disconnect");

    await connectSocket(client, firstPeerPort, hostname);
    const sentBeforeReconnect = receiveMessage(firstPeer);
    client.send("before-reconnect");
    client.disconnect();

    await connectSocket(client, secondPeerPort, hostname);
    assertEquals(client.remoteAddress().port, secondPeerPort);

    const secondPeerReceived = receiveMessage(secondPeer);
    await sendConnected(client, "reconnected-send");
    assertEquals(await sentBeforeReconnect, "before-reconnect");
    assertEquals(await secondPeerReceived, "reconnected-send");

    const receivedFromSecondPeer = receiveMessage(client);
    for (let i = 0; i < 8; i++) {
      await sendTo(firstPeer, `old-peer-${i}`, clientPort, hostname);
    }
    await sendTo(secondPeer, "second-peer", clientPort, hostname);
    assertEquals(await receivedFromSecondPeer, "second-peer");
  } finally {
    await closeSockets(client, firstPeer, secondPeer);
  }
}

async function runRestrictedNetCode(
  code: string,
  allowNet = "0.0.0.0:0",
): Promise<string> {
  const tempFile = Deno.makeTempFileSync({ suffix: ".ts" });
  Deno.writeTextFileSync(tempFile, code);
  try {
    const { stdout, stderr } = await new Deno.Command(Deno.execPath(), {
      args: [
        "run",
        "--no-prompt",
        `--allow-net=${allowNet}`,
        `--allow-read=${tempFile}`,
        tempFile,
      ],
      stdout: "piped",
      stderr: "piped",
    }).output();
    return new TextDecoder().decode(stdout) +
      new TextDecoder().decode(stderr);
  } finally {
    Deno.removeSync(tempFile);
  }
}

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

Deno.test("[node/dgram] connected udp4 uses one peer", async () => {
  await connectedUdpPeerTest("udp4", "127.0.0.1");
});

Deno.test("[node/dgram] connected udp6 uses one peer", async () => {
  await connectedUdpPeerTest("udp6", "::1");
});

Deno.test("[node/dgram] failed connect can be retried", async () => {
  let useWrongFamily = true;
  const peer = createSocket("udp4");
  const socket = createSocket({
    type: "udp4",
    lookup(hostname, _family, callback) {
      const address = hostname === "retry.test" && useWrongFamily
        ? "::1"
        : "127.0.0.1";
      queueMicrotask(() =>
        callback(
          null,
          address,
          address === "::1" ? 6 : 4,
        )
      );
    },
  });

  try {
    await Promise.all([
      bindSocket(peer, "127.0.0.1"),
      bindSocket(socket, "127.0.0.1"),
    ]);

    let connectError: Error | undefined;
    try {
      await connectSocket(socket, peer.address().port, "retry.test");
    } catch (error) {
      connectError = error as Error;
    }
    assert(connectError instanceof Error);

    let remoteAddressFailed = false;
    try {
      socket.remoteAddress();
    } catch {
      remoteAddressFailed = true;
    }
    assert(remoteAddressFailed);

    useWrongFamily = false;
    await connectSocket(socket, peer.address().port, "retry.test");
    assertEquals(socket.remoteAddress().port, peer.address().port);

    const received = receiveMessage(peer);
    await sendConnected(socket, "retry-connected");
    assertEquals(await received, "retry-connected");
  } finally {
    await closeSockets(socket, peer);
  }
});

Deno.test("[node/dgram] addMembership works", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const socket = createSocket("udp4");
  socket.bind(0, () => {
    try {
      socket.addMembership("224.0.0.114");
    } finally {
      socket.close();
    }
  });
  socket.on("close", () => resolve());
  await promise;
});

Deno.test("[node/dgram] addMembership accepts scoped IPv6 interface", async () => {
  // Regression test for https://github.com/denoland/deno/issues/34838.
  // A scoped IPv6 interface whose zone id names no existing interface must
  // resolve to the default interface (index 0) — exactly like passing no
  // interface at all — instead of being rejected with EINVAL. Node.js (via
  // libuv's `uv_ip6_addr`) behaves the same.
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const socket = createSocket({ type: "udp6", ipv6Only: true });
  let bound = false;
  socket.on("error", (err) => {
    // IPv6 may be unavailable before binding; treat that as a skip.
    socket.close();
    if (bound) {
      reject(err);
    } else {
      resolve();
    }
  });
  socket.bind(0, () => {
    bound = true;
    try {
      // First join without an interface to determine whether IPv6 multicast is
      // available at all in this environment (some CI sandboxes have no
      // multicast-capable default interface). If it fails, skip the test.
      socket.addMembership("ff02::fb");
    } catch {
      socket.close(() => resolve());
      return;
    }
    try {
      // The scoped interface resolves to the same default interface as the
      // baseline join above, so it must be accepted too. On Unix a numeric
      // zone is resolved via `if_nametoindex` (-> 0 for non-names), which is
      // the reporter's exact "::%12" case; on Windows a numeric zone is a
      // literal index (`atoi`), so use a non-numeric unknown name there.
      const scopedIface = Deno.build.os === "windows"
        ? "::%nonexistent0"
        : "::%9999999";
      // Use a different multicast group so this is a fresh join rather than a
      // duplicate of the baseline join above.
      socket.addMembership("ff02::1:3", scopedIface);
      socket.close(() => resolve());
    } catch (err) {
      socket.close();
      reject(err);
    }
  });
  await promise;
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

Deno.test("[node/dgram] send checks destination permission", {
  permissions: { read: true, write: true, run: true, net: true },
}, async () => {
  // Verify that a subprocess with restricted --allow-net cannot send to
  // destinations outside the allowed set.
  const output = await runRestrictedNetCode(
    `import dgram from "node:dgram";
const socket = dgram.createSocket("udp4");
socket.bind(0, "0.0.0.0", () => {
  socket.send("test", 9999, "127.0.0.1", (err) => {
    if (err) {
      console.log("SEND_BLOCKED:" + err.message);
    } else {
      console.log("SEND_ALLOWED");
    }
    socket.close();
  });
});
socket.on("error", (err) => {
  console.log("ERROR:" + err.message);
  socket.close();
});
`,
  );
  assert(
    !output.includes("SEND_ALLOWED"),
    `Send should have been blocked, but got: ${output}`,
  );
});

Deno.test("[node/dgram] connect checks destination permission", {
  permissions: { read: true, write: true, run: true, net: true },
}, async () => {
  const output = await runRestrictedNetCode(
    `import dgram from "node:dgram";
const socket = dgram.createSocket("udp4");
socket.bind(0, "0.0.0.0", () => {
  socket.connect(9999, "127.0.0.1", () => {
    console.log("CONNECT_ALLOWED");
    socket.close();
  });
});
socket.on("error", (err) => {
  console.log("ERROR:" + err.message);
  socket.close();
});
`,
  );
  assert(
    !output.includes("CONNECT_ALLOWED"),
    `Connect should have been blocked, but got: ${output}`,
  );
});

Deno.test("[node/dgram] connected send rechecks destination permission", {
  permissions: { read: true, write: true, run: true, net: true },
}, async () => {
  const output = await runRestrictedNetCode(
    `import dgram from "node:dgram";
const peer = dgram.createSocket("udp4");
const socket = dgram.createSocket("udp4");
peer.bind(0, "127.0.0.1", () => {
  socket.bind(0, "0.0.0.0", () => {
    socket.connect(peer.address().port, "127.0.0.1", async () => {
      await Deno.permissions.revoke({ name: "net" });
      let synchronous = true;
      socket.send("test", (err) => {
        console.log(err ? "SEND_BLOCKED" : "SEND_ALLOWED");
        console.log(synchronous ? "CALLBACK_SYNC" : "CALLBACK_ASYNC");
        socket.close();
        peer.close();
      });
      synchronous = false;
    });
  });
});
socket.on("error", (err) => {
  console.log("ERROR:" + err.message);
  socket.close();
  peer.close();
});
`,
    "0.0.0.0:0,127.0.0.1",
  );
  assert(
    output.includes("SEND_BLOCKED") && output.includes("CALLBACK_ASYNC") &&
      !output.includes("SEND_ALLOWED") && !output.includes("CALLBACK_SYNC"),
    `Connected send should have been blocked, but got: ${output}`,
  );
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
