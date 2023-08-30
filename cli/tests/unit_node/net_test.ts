// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as net from "node:net";
import {
  assert,
  assertEquals,
} from "../../../test_util/std/testing/asserts.ts";
import { Deferred, deferred } from "../../../test_util/std/async/deferred.ts";
import * as path from "../../../test_util/std/path/mod.ts";
import * as http from "node:http";

Deno.test("[node/net] close event emits after error event", async () => {
  const socket = net.createConnection(27009, "doesnotexist");
  const events: ("error" | "close")[] = [];
  const errorEmitted = deferred();
  const closeEmitted = deferred();
  socket.once("error", () => {
    events.push("error");
    errorEmitted.resolve();
  });
  socket.once("close", () => {
    events.push("close");
    closeEmitted.resolve();
  });
  await Promise.all([errorEmitted, closeEmitted]);

  // `error` happens before `close`
  assertEquals(events, ["error", "close"]);
});

Deno.test("[node/net] the port is available immediately after close callback", async () => {
  const p = deferred();

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
  httpServer.on("error", (e) => p.reject(e));
  httpServer.listen(port, () => {
    httpServer.close(() => p.resolve());
  });
  await p;
});

Deno.test("[node/net] net.connect().unref() works", async () => {
  const ctl = new AbortController();
  const server = Deno.serve({
    signal: ctl.signal,
    handler: () => new Response("hello"),
    onListen: async ({ port, hostname }) => {
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
      assert(e instanceof Deno.errors.PermissionDenied);
    }
  },
});

Deno.test("[node/net] connection event has socket value", async () => {
  const p = deferred();
  const p2 = deferred();

  const server = net.createServer();
  server.on("error", p.reject);
  server.on("connection", (socket) => {
    assert(socket !== undefined);
    socket.end();
    server.close(() => {
      p.resolve();
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

    p2.resolve();
  });

  await Promise.all([p, p2]);
});

/// We need to make sure that any shared buffers are never used concurrently by two reads.
// https://github.com/denoland/deno/issues/20188
Deno.test("[node/net] multiple Sockets should get correct server data", async () => {
  const socketCount = 9;

  class TestSocket {
    dataReceived: Deferred<undefined> = deferred();
    events: string[] = [];
    socket: net.Socket | undefined;
  }

  const finished = deferred();
  const server = net.createServer();
  server.on("connection", (socket) => {
    assert(socket !== undefined);
    socket.on("data", (data) => {
      socket.write(new TextDecoder().decode(data));
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

    await Promise.all(sockets.map((socket) => socket.dataReceived));

    for (let i = 0; i < socketCount; i++) {
      sockets[i].socket?.end();
    }

    server.close(() => {
      finished.resolve();
    });
  });

  await finished;

  for (let i = 0; i < socketCount; i++) {
    assertEquals(sockets[i].events, [`${i}`.repeat(3), `${i}`.repeat(3)]);
  }
});
