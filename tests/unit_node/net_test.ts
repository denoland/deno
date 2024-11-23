// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import * as net from "node:net";
import { assert, assertEquals } from "@std/assert";
import * as path from "@std/path";
import * as http from "node:http";

Deno.test("[node/net] close event emits after error event - when host is not found", async () => {
  const socket = net.createConnection(27009, "doesnotexist");
  const events: ("error" | "close")[] = [];
  const errorEmitted = Promise.withResolvers<void>();
  const closeEmitted = Promise.withResolvers<void>();
  socket.once("error", () => {
    events.push("error");
    errorEmitted.resolve();
  });
  socket.once("close", () => {
    events.push("close");
    closeEmitted.resolve();
  });
  await Promise.all([errorEmitted.promise, closeEmitted.promise]);

  // `error` happens before `close`
  assertEquals(events, ["error", "close"]);
});

Deno.test("[node/net] close event emits after error event - when connection is refused", async () => {
  const socket = net.createConnection(27009, "127.0.0.1");
  const events: ("error" | "close")[] = [];
  const errorEmitted = Promise.withResolvers<void>();
  const closeEmitted = Promise.withResolvers<void>();
  socket.once("error", () => {
    events.push("error");
    errorEmitted.resolve();
  });
  socket.once("close", () => {
    events.push("close");
    closeEmitted.resolve();
  });
  await Promise.all([errorEmitted.promise, closeEmitted.promise]);

  // `error` happens before `close`
  assertEquals(events, ["error", "close"]);
});

Deno.test("[node/net] the port is available immediately after close callback", async () => {
  const deferred = Promise.withResolvers<void>();

  // This simulates what get-port@5.1.1 does.
  const getAvailablePort = (port: number) =>
    new Promise((resolve, reject) => {
      const server = net.createServer();
      server.on("error", reject);
      server.listen({ port }, () => {
        // deno-lint-ignore no-explicit-any
        const { port } = server.address() as any;
        server.close(() => {
          resolve(port);
        });
      });
    });

  const port = await getAvailablePort(5555);

  const httpServer = http.createServer();
  httpServer.on("error", (e) => deferred.reject(e));
  httpServer.listen(port, () => {
    httpServer.close(() => deferred.resolve());
  });
  await deferred.promise;
});

Deno.test("[node/net] net.connect().unref() works", async () => {
  const ctl = new AbortController();
  const server = Deno.serve({
    signal: ctl.signal,
    port: 0, // any available port will do
    handler: () => new Response("hello"),
    onListen: async ({ port, hostname }) => {
      hostname = Deno.build.os === "windows" ? "localhost" : hostname;
      const { stdout, stderr } = await new Deno.Command(Deno.execPath(), {
        args: [
          "eval",
          `
            import * as net from "node:net";
            const socket = net.connect(${port}, "${hostname}", () => {
              console.log("connected");
              socket.unref();
              socket.on("data", (data) => console.log(data.toString()));
              socket.write("GET / HTTP/1.1\\n\\n");
            });
          `,
        ],
        cwd: path.dirname(path.fromFileUrl(import.meta.url)),
      }).output();
      if (stderr.length > 0) {
        console.log(new TextDecoder().decode(stderr));
      }
      assertEquals(new TextDecoder().decode(stdout), "connected\n");
      ctl.abort();
    },
  });
  await server.finished;
});

Deno.test({
  name: "[node/net] throws permission error instead of unknown error",
  permissions: "none",
  fn: () => {
    try {
      const s = new net.Server();
      s.listen(3000);
    } catch (e) {
      assert(e instanceof Deno.errors.NotCapable);
    }
  },
});

Deno.test("[node/net] connection event has socket value", async () => {
  const deferred = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<void>();

  const server = net.createServer();
  server.on("error", deferred.reject);
  server.on("connection", (socket) => {
    assert(socket !== undefined);
    socket.end();
    server.close(() => {
      deferred.resolve();
    });
  });
  server.listen(async () => {
    // deno-lint-ignore no-explicit-any
    const { port } = server.address() as any;

    const conn = await Deno.connect({
      port,
      transport: "tcp",
    });

    for await (const _ of conn.readable) {
      //
    }

    deferred2.resolve();
  });

  await Promise.all([deferred.promise, deferred2.promise]);
});

/// We need to make sure that any shared buffers are never used concurrently by two reads.
// https://github.com/denoland/deno/issues/20188
Deno.test("[node/net] multiple Sockets should get correct server data", async () => {
  const socketCount = 9;

  class TestSocket {
    dataReceived: ReturnType<typeof Promise.withResolvers<void>> = Promise
      .withResolvers<void>();
    events: string[] = [];
    socket: net.Socket | undefined;
  }

  const finished = Promise.withResolvers<void>();
  const serverSocketsClosed: ReturnType<typeof Promise.withResolvers<void>>[] =
    [];
  const server = net.createServer();
  server.on("connection", (socket) => {
    assert(socket !== undefined);
    const i = serverSocketsClosed.push(Promise.withResolvers<void>());
    socket.on("data", (data) => {
      socket.write(new TextDecoder().decode(data));
    });
    socket.on("close", () => {
      serverSocketsClosed[i - 1].resolve();
    });
  });

  const sockets: TestSocket[] = [];
  for (let i = 0; i < socketCount; i++) {
    sockets[i] = new TestSocket();
  }

  server.listen(async () => {
    // deno-lint-ignore no-explicit-any
    const { port } = server.address() as any;

    for (let i = 0; i < socketCount; i++) {
      const socket = sockets[i].socket = net.createConnection(port);
      socket.on("data", (data) => {
        const count = sockets[i].events.length;
        sockets[i].events.push(new TextDecoder().decode(data));
        if (count === 0) {
          // Trigger an immediate second write
          sockets[i].socket?.write(`${i}`.repeat(3));
        } else {
          sockets[i].dataReceived.resolve();
        }
      });
    }

    for (let i = 0; i < socketCount; i++) {
      sockets[i].socket?.write(`${i}`.repeat(3));
    }

    await Promise.all(sockets.map((socket) => socket.dataReceived.promise));

    for (let i = 0; i < socketCount; i++) {
      sockets[i].socket?.end();
    }

    server.close(() => {
      finished.resolve();
    });
  });

  await finished.promise;
  await Promise.all(serverSocketsClosed.map(({ promise }) => promise));

  for (let i = 0; i < socketCount; i++) {
    assertEquals(sockets[i].events, [`${i}`.repeat(3), `${i}`.repeat(3)]);
  }
});

Deno.test("[node/net] BlockList doesn't leak resources", () => {
  const blockList = new net.BlockList();
  blockList.addAddress("1.1.1.1");
  assert(blockList.check("1.1.1.1"));
});

Deno.test("[node/net] net.Server can listen on the same port immediately after it's closed", async () => {
  const serverClosed = Promise.withResolvers<void>();
  const server = net.createServer();
  server.on("error", (e) => {
    console.error(e);
  });
  server.listen(0, () => {
    // deno-lint-ignore no-explicit-any
    const { port } = server.address() as any;
    server.close();
    server.listen(port, () => {
      server.close(() => {
        serverClosed.resolve();
      });
    });
  });
  await serverClosed.promise;
});
