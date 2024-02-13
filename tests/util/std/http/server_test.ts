// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { ConnInfo, serve, serveListener, Server, serveTls } from "./server.ts";
import { mockConn as createMockConn } from "./_mock_conn.ts";
import { dirname, fromFileUrl, join, resolve } from "../path/mod.ts";
import { writeAll } from "../streams/write_all.ts";
import { readAll } from "../streams/read_all.ts";
import { delay } from "../async/mod.ts";
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
  assertStrictEquals,
  assertThrows,
  unreachable,
} from "../assert/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));
const testdataDir = resolve(moduleDir, "testdata");

let port = 4800;
function getPort() {
  return port++;
}

type AcceptCallSideEffect = ({
  acceptCallCount,
}: {
  acceptCallCount: number;
}) => void | Promise<void>;

class MockListener implements Deno.Listener {
  conn: Deno.Conn;
  #closed = false;
  #rejectionError?: Error;
  #rejectionCount: number;
  #acceptCallSideEffect: AcceptCallSideEffect;
  acceptCallTimes: number[] = [];
  acceptCallIntervals: number[] = [];
  acceptCallCount = 0;

  constructor({
    conn,
    rejectionError,
    rejectionCount = Infinity,
    acceptCallSideEffect = () => {},
  }: {
    conn: Deno.Conn;
    rejectionError?: Error;
    rejectionCount?: number;
    acceptCallSideEffect?: AcceptCallSideEffect;
  }) {
    this.conn = conn;
    this.#rejectionError = rejectionError;
    this.#rejectionCount = rejectionCount;
    this.#acceptCallSideEffect = acceptCallSideEffect;
  }

  get addr(): Deno.Addr {
    return this.conn.localAddr;
  }

  get rid(): number {
    return 4505;
  }

  #shouldReject(): boolean {
    return (
      typeof this.#rejectionError !== "undefined" &&
      this.acceptCallCount <= this.#rejectionCount
    );
  }

  async accept(): Promise<Deno.Conn> {
    if (this.#closed) {
      throw new Deno.errors.BadResource("MockListener has closed");
    }

    const now = performance.now();
    this.acceptCallIntervals.push(now - (this.acceptCallTimes.at(-1) ?? now));
    this.acceptCallTimes.push(now);
    this.acceptCallCount++;
    this.#acceptCallSideEffect({ acceptCallCount: this.acceptCallCount });

    await delay(0);

    return this.#shouldReject()
      ? Promise.reject(this.#rejectionError)
      : Promise.resolve(this.conn);
  }

  [Symbol.dispose]() {
    this.close();
  }

  close() {
    this.#closed = true;
  }

  async *[Symbol.asyncIterator](): AsyncIterableIterator<Deno.Conn> {
    while (true) {
      if (this.#closed) {
        break;
      }

      const now = performance.now();
      this.acceptCallIntervals.push(now - (this.acceptCallTimes.at(-1) ?? now));
      this.acceptCallTimes.push(now);
      this.acceptCallCount++;
      this.#acceptCallSideEffect({ acceptCallCount: this.acceptCallCount });

      await delay(0);

      if (this.#shouldReject()) {
        throw this.#rejectionError;
      }

      yield this.conn;
    }
  }

  ref() {
  }

  unref() {
  }
}

Deno.test(
  "Server.addrs should expose the addresses the server is listening on",
  async () => {
    const listenerOneOptions = {
      hostname: "127.0.0.1",
      port: getPort(),
    };
    const listenerTwoOptions = {
      hostname: "127.0.0.1",
      port: getPort(),
    };
    const listenerOne = Deno.listen(listenerOneOptions);
    const listenerTwo = Deno.listen(listenerTwoOptions);

    const addrHostname = "0.0.0.0";
    const addrPort = getPort();
    const handler = () => new Response();

    const server = new Server({
      port: addrPort,
      hostname: addrHostname,
      handler,
    });
    const servePromiseOne = server.serve(listenerOne);
    const servePromiseTwo = server.serve(listenerTwo);
    const servePromiseThree = server.listenAndServe();

    try {
      assertEquals(server.addrs.length, 3);
      assertEquals(server.addrs[0].transport, "tcp");
      assertEquals(
        (server.addrs[0] as Deno.NetAddr).hostname,
        listenerOneOptions.hostname,
      );
      assertEquals(
        (server.addrs[0] as Deno.NetAddr).port,
        listenerOneOptions.port,
      );
      assertEquals(server.addrs[1].transport, "tcp");
      assertEquals(
        (server.addrs[1] as Deno.NetAddr).hostname,
        listenerTwoOptions.hostname,
      );
      assertEquals(
        (server.addrs[1] as Deno.NetAddr).port,
        listenerTwoOptions.port,
      );
      assertEquals(server.addrs[2].transport, "tcp");
      assertEquals((server.addrs[2] as Deno.NetAddr).hostname, addrHostname);
      assertEquals((server.addrs[2] as Deno.NetAddr).port, addrPort);
    } finally {
      server.close();
      await servePromiseOne;
      await servePromiseTwo;
      await servePromiseThree;
    }
  },
);

Deno.test("Server.closed should expose whether it is closed", () => {
  const handler = () => new Response();
  const server = new Server({ handler });
  try {
    assertEquals(server.closed, false);
  } finally {
    server.close();
    assertEquals(server.closed, true);
  }
});

Deno.test(
  "Server.close should throw an error if the server is already closed",
  () => {
    const handler = () => new Response();
    const server = new Server({ handler });
    server.close();

    assertThrows(() => server.close(), Deno.errors.Http, "Server closed");
  },
);

Deno.test(
  "Server.serve should throw an error if the server is already closed",
  async () => {
    const handler = () => new Response();
    const server = new Server({ handler });
    server.close();

    const listenOptions = {
      hostname: "localhost",
      port: getPort(),
    };
    const listener = Deno.listen(listenOptions);

    await assertRejects(
      () => server.serve(listener),
      Deno.errors.Http,
      "Server closed",
    );

    try {
      listener.close();
    } catch (error) {
      if (!(error instanceof Deno.errors.BadResource)) {
        throw error;
      }
    }
  },
);

Deno.test(
  "Server.listenAndServe should throw an error if the server is already closed",
  async () => {
    const handler = () => new Response();
    const server = new Server({ handler });
    server.close();

    await assertRejects(
      () => server.listenAndServe(),
      Deno.errors.Http,
      "Server closed",
    );
  },
);

Deno.test(
  "Server.listenAndServeTls should throw an error if the server is already closed",
  async () => {
    const handler = () => new Response();
    const server = new Server({ handler });
    server.close();

    const certFile = join(testdataDir, "tls/localhost.crt");
    const keyFile = join(testdataDir, "tls/localhost.key");

    await assertRejects(
      () => server.listenAndServeTls(certFile, keyFile),
      Deno.errors.Http,
      "Server closed",
    );
  },
);

Deno.test(
  "serveListener should not overwrite an abort signal handler",
  async () => {
    const listenOptions = {
      hostname: "localhost",
      port: getPort(),
    };
    const listener = Deno.listen(listenOptions);
    const handler = () => new Response();
    const onAbort = () => {};
    const abortController = new AbortController();

    abortController.signal.onabort = onAbort;

    const servePromise = serveListener(listener, handler, {
      signal: abortController.signal,
    });

    try {
      assertStrictEquals(abortController.signal.onabort, onAbort);
    } finally {
      abortController.abort();
      await servePromise;
    }
  },
);

Deno.test(
  "serve should not overwrite an abort signal handler",
  async () => {
    const handler = () => new Response();
    const onAbort = () => {};
    const abortController = new AbortController();

    abortController.signal.onabort = onAbort;

    const servePromise = serve(handler, {
      hostname: "localhost",
      port: getPort(),
      signal: abortController.signal,
    });

    try {
      assertStrictEquals(abortController.signal.onabort, onAbort);
    } finally {
      abortController.abort();
      await servePromise;
    }
  },
);

Deno.test(
  "serveTls should not overwrite an abort signal handler",
  async () => {
    const certFile = join(testdataDir, "tls/localhost.crt");
    const keyFile = join(testdataDir, "tls/localhost.key");
    const handler = () => new Response();
    const onAbort = () => {};
    const abortController = new AbortController();

    abortController.signal.onabort = onAbort;

    const servePromise = serveTls(handler, {
      hostname: "localhost",
      port: getPort(),
      certFile,
      keyFile,
      signal: abortController.signal,
    });

    try {
      assertStrictEquals(abortController.signal.onabort, onAbort);
    } finally {
      abortController.abort();
      await servePromise;
    }
  },
);

Deno.test(
  "serveListener should not throw if abort when the server is already closed",
  async () => {
    const listenOptions = {
      hostname: "localhost",
      port: getPort(),
    };
    const listener = Deno.listen(listenOptions);
    const handler = () => new Response();
    const abortController = new AbortController();

    const servePromise = serveListener(listener, handler, {
      signal: abortController.signal,
    });

    abortController.abort();

    try {
      abortController.abort();
    } finally {
      await servePromise;
    }
  },
);

Deno.test(
  "serve should not throw if abort when the server is already closed",
  async () => {
    const handler = () => new Response();
    const abortController = new AbortController();

    const servePromise = serve(handler, {
      hostname: "localhost",
      port: getPort(),
      signal: abortController.signal,
    });

    abortController.abort();

    try {
      abortController.abort();
    } finally {
      await servePromise;
    }
  },
);

Deno.test(
  "serveTls should not throw if abort when the server is already closed",
  async () => {
    const certFile = join(testdataDir, "tls/localhost.crt");
    const keyFile = join(testdataDir, "tls/localhost.key");
    const handler = () => new Response();
    const abortController = new AbortController();

    const servePromise = serveTls(handler, {
      hostname: "localhost",
      port: getPort(),
      certFile,
      keyFile,
      signal: abortController.signal,
    });

    abortController.abort();

    try {
      abortController.abort();
    } finally {
      await servePromise;
    }
  },
);

Deno.test(`Server.serve should response with internal server error if response body is already consumed`, async () => {
  const listenOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listener = Deno.listen(listenOptions);

  const url = `http://${listenOptions.hostname}:${listenOptions.port}`;
  const body = "Internal Server Error";
  const status = 500;

  async function handler() {
    const response = new Response("Hello, world!");
    await response.text();
    return response;
  }

  const server = new Server({ handler });
  const servePromise = server.serve(listener);

  try {
    const response = await fetch(url);
    assertEquals(await response.text(), body);
    assertEquals(response.status, status);
  } finally {
    server.close();
    await servePromise;
  }
});

Deno.test(`Server.serve should handle requests`, async () => {
  const listenOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listener = Deno.listen(listenOptions);

  const url = `http://${listenOptions.hostname}:${listenOptions.port}`;
  const status = 418;
  const method = "GET";
  const body = `${method}: ${url} - Hello Deno on HTTP!`;

  const handler = () => new Response(body, { status });

  const server = new Server({ handler });
  const servePromise = server.serve(listener);

  try {
    const response = await fetch(url, { method });
    assertEquals(await response.text(), body);
    assertEquals(response.status, status);
  } finally {
    server.close();
    await servePromise;
  }
});

Deno.test(`Server.listenAndServe should handle requests`, async () => {
  const hostname = "localhost";
  const port = getPort();
  const url = `http://${hostname}:${port}`;
  const status = 418;
  const method = "POST";
  const body = `${method}: ${url} - Hello Deno on HTTP!`;

  const handler = () => new Response(body, { status });

  const server = new Server({ hostname, port, handler });
  const servePromise = server.listenAndServe();

  try {
    const response = await fetch(url, { method });
    assertEquals(await response.text(), body);
    assertEquals(response.status, status);
  } finally {
    server.close();
    await servePromise;
  }
});

Deno.test({
  // PermissionDenied: Permission denied (os error 13)
  // Will pass if run as root user.
  ignore: true,
  name: `Server.listenAndServe should handle requests on the default HTTP port`,
  fn: async () => {
    const addr = "localhost";
    const url = `http://${addr}`;
    const status = 418;
    const method = "PATCH";
    const body = `${method}: ${url} - Hello Deno on HTTP!`;

    const handler = () => new Response(body, { status });

    const server = new Server({ hostname: addr, handler });
    const servePromise = server.listenAndServe();

    try {
      const response = await fetch(url, { method });
      assertEquals(await response.text(), body);
      assertEquals(response.status, status);
    } finally {
      server.close();
      await servePromise;
    }
  },
});

Deno.test(`Server.listenAndServeTls should handle requests`, async () => {
  const hostname = "localhost";
  const port = getPort();
  const addr = `${hostname}:${port}`;
  const certFile = join(testdataDir, "tls/localhost.crt");
  const keyFile = join(testdataDir, "tls/localhost.key");
  const url = `http://${addr}`;
  const status = 418;
  const method = "DELETE";
  const body = `${method}: ${url} - Hello Deno on HTTPS!`;

  const handler = () => new Response(body, { status });

  const server = new Server({ hostname, port, handler });
  const servePromise = server.listenAndServeTls(certFile, keyFile);

  try {
    // Invalid certificate, connection should throw on first read or write
    // but should not crash the server.
    const badConn = await Deno.connectTls({
      hostname,
      port,
      // missing certFile
    });

    await assertRejects(
      () => badConn.read(new Uint8Array(1)),
      Deno.errors.InvalidData,
      "invalid peer certificate: UnknownIssuer",
      "Read with missing certFile didn't throw an InvalidData error when it should have.",
    );

    badConn.close();

    // Valid request after invalid
    const conn = await Deno.connectTls({
      hostname,
      port,
      certFile: join(testdataDir, "tls/RootCA.pem"),
    });

    await writeAll(
      conn,
      new TextEncoder().encode(`${method.toUpperCase()} / HTTP/1.0\r\n\r\n`),
    );

    const response = new TextDecoder().decode(await readAll(conn));

    conn.close();

    assert(response.includes(`HTTP/1.0 ${status}`), "Status code not correct");
    assert(response.includes(body), "Response body not correct");
  } finally {
    server.close();
    await servePromise;
  }
});

Deno.test({
  // PermissionDenied: Permission denied (os error 13)
  // Will pass if run as root user.
  ignore: true,
  name:
    `Server.listenAndServeTls should handle requests on the default HTTPS port`,
  fn: async () => {
    const hostname = "localhost";
    const port = 443;
    const addr = hostname;
    const certFile = join(testdataDir, "tls/localhost.crt");
    const keyFile = join(testdataDir, "tls/localhost.key");
    const url = `http://${addr}`;
    const status = 418;
    const method = "PUT";
    const body = `${method}: ${url} - Hello Deno on HTTPS!`;

    const handler = () => new Response(body, { status });

    const server = new Server({ hostname, port, handler });
    const servePromise = server.listenAndServeTls(certFile, keyFile);

    try {
      // Invalid certificate, connection should throw on first read or write
      // but should not crash the server.
      const badConn = await Deno.connectTls({
        hostname,
        port,
        // missing certFile
      });

      await assertRejects(
        () => badConn.read(new Uint8Array(1)),
        Deno.errors.InvalidData,
        "invalid peer certificate contents: invalid peer certificate: UnknownIssuer",
        "Read with missing certFile didn't throw an InvalidData error when it should have.",
      );

      badConn.close();

      // Valid request after invalid
      const conn = await Deno.connectTls({
        hostname,
        port,
        certFile: join(testdataDir, "tls/RootCA.pem"),
      });

      await writeAll(
        conn,
        new TextEncoder().encode(`${method.toUpperCase()} / HTTP/1.0\r\n\r\n`),
      );

      const response = new TextDecoder().decode(await readAll(conn));

      conn.close();

      assert(
        response.includes(`HTTP/1.0 ${status}`),
        "Status code not correct",
      );
      assert(response.includes(body), "Response body not correct");
    } finally {
      server.close();
      await servePromise;
    }
  },
});

Deno.test(`serve should handle requests`, async () => {
  const listenOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listener = Deno.listen(listenOptions);

  const url = `http://${listenOptions.hostname}:${listenOptions.port}`;
  const status = 418;
  const method = "GET";
  const body = `${method}: ${url} - Hello Deno on HTTP!`;

  const handler = () => new Response(body, { status });
  const abortController = new AbortController();

  const servePromise = serveListener(listener, handler, {
    signal: abortController.signal,
  });

  try {
    const response = await fetch(url, { method });
    assertEquals(await response.text(), body);
    assertEquals(response.status, status);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test(`serve should handle requests`, async () => {
  const hostname = "localhost";
  const port = getPort();
  const url = `http://${hostname}:${port}`;
  const status = 418;
  const method = "POST";
  const body = `${method}: ${url} - Hello Deno on HTTP!`;

  const handler = () => new Response(body, { status });
  const abortController = new AbortController();

  const servePromise = serve(handler, {
    hostname,
    port,
    signal: abortController.signal,
  });

  try {
    const response = await fetch(url, { method });
    assertEquals(await response.text(), body);
    assertEquals(response.status, status);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test(`serve listens on the port 8000 by default`, async () => {
  const url = "http://localhost:8000";
  const body = "Hello from port 8000";

  const handler = () => new Response(body);
  const abortController = new AbortController();

  const servePromise = serve(handler, {
    signal: abortController.signal,
  });
  servePromise.catch(() => {});

  try {
    const response = await fetch(url);
    assertEquals(await response.text(), body);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test(`serve should handle websocket requests`, async () => {
  const hostname = "localhost";
  const port = getPort();
  const url = `ws://${hostname}:${port}`;
  const message = `${url} - Hello Deno on WebSocket!`;

  const abortController = new AbortController();

  const servePromise = serve(
    (request) => {
      const { socket, response } = Deno.upgradeWebSocket(request);
      // Return the received message as it is
      socket.onmessage = (event) => socket.send(event.data);
      return response;
    },
    {
      hostname,
      port,
      signal: abortController.signal,
    },
  );

  const ws = new WebSocket(url);
  const closePromise = new Promise((resolve) => {
    ws.onclose = resolve;
  });
  try {
    ws.onopen = () => ws.send(message);
    const response = await new Promise((resolve) => {
      ws.onmessage = (event) => resolve(event.data);
    });
    assertEquals(response, message);
  } finally {
    ws.close();
    abortController.abort();
    await servePromise;
    await closePromise;
  }
});

Deno.test(`Server.listenAndServeTls should handle requests`, async () => {
  const hostname = "localhost";
  const port = getPort();
  const addr = `${hostname}:${port}`;
  const certFile = join(testdataDir, "tls/localhost.crt");
  const keyFile = join(testdataDir, "tls/localhost.key");
  const url = `http://${addr}`;
  const status = 418;
  const method = "PATCH";
  const body = `${method}: ${url} - Hello Deno on HTTPS!`;

  const handler = () => new Response(body, { status });
  const abortController = new AbortController();

  const servePromise = serveTls(handler, {
    hostname,
    port,
    certFile,
    keyFile,
    signal: abortController.signal,
  });

  try {
    // Invalid certificate, connection should throw on first read or write
    // but should not crash the server.
    const badConn = await Deno.connectTls({
      hostname,
      port,
      // missing certFile
    });

    await assertRejects(
      () => badConn.read(new Uint8Array(1)),
      Deno.errors.InvalidData,
      "invalid peer certificate: UnknownIssuer",
      "Read with missing certFile didn't throw an InvalidData error when it should have.",
    );

    badConn.close();

    // Valid request after invalid
    const conn = await Deno.connectTls({
      hostname,
      port,
      certFile: join(testdataDir, "tls/RootCA.pem"),
    });

    await writeAll(
      conn,
      new TextEncoder().encode(`${method.toUpperCase()} / HTTP/1.0\r\n\r\n`),
    );

    const response = new TextDecoder().decode(await readAll(conn));

    conn.close();

    assert(response.includes(`HTTP/1.0 ${status}`), "Status code not correct");
    assert(response.includes(body), "Response body not correct");
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test(
  "Server should not reject when the listener is closed (though the server will continually try and fail to accept connections on the listener until it is closed)",
  async () => {
    const listener = Deno.listen({ port: getPort() });
    const handler = () => new Response();
    const server = new Server({ handler });
    listener.close();

    let servePromise;

    try {
      servePromise = server.serve(listener);
      await delay(10);
    } finally {
      server.close();
      await servePromise;
    }
  },
);

Deno.test(
  "Server should not reject when there is a tls handshake with tcp corruption",
  async () => {
    const conn = createMockConn();
    const rejectionError = new Deno.errors.InvalidData(
      "test-tcp-corruption-error",
    );
    const listener = new MockListener({ conn, rejectionError });
    const handler = () => new Response();
    const server = new Server({ handler });

    let servePromise;

    try {
      servePromise = server.serve(listener);
      await delay(10);
    } finally {
      server.close();
      await servePromise;
    }
  },
);

Deno.test(
  "Server should not reject when the tls session is aborted",
  async () => {
    const conn = createMockConn();
    const rejectionError = new Deno.errors.ConnectionReset(
      "test-tls-session-aborted-error",
    );
    const listener = new MockListener({ conn, rejectionError });
    const handler = () => new Response();
    const server = new Server({ handler });

    let servePromise;

    try {
      servePromise = server.serve(listener);
      await delay(10);
    } finally {
      server.close();
      await servePromise;
    }
  },
);

Deno.test("Server should not reject when the socket is closed", async () => {
  const conn = createMockConn();
  const rejectionError = new Deno.errors.NotConnected(
    "test-socket-closed-error",
  );
  const listener = new MockListener({ conn, rejectionError });
  const handler = () => new Response();
  const server = new Server({ handler });

  let servePromise;

  try {
    servePromise = server.serve(listener);
    await delay(10);
  } finally {
    server.close();
    await servePromise;
  }
});

Deno.test(
  "Server should implement a backoff delay when accepting a connection throws an expected error and reset the backoff when successfully accepting a connection again",
  async () => {
    // acceptDelay(n) = 5 * 2^n for n=0...7 capped at 1000 afterwards.
    const expectedBackoffDelays = [
      5,
      10,
      20,
      40,
      80,
      160,
      320,
      640,
      1000,
      1000,
    ];
    const rejectionCount = expectedBackoffDelays.length;

    let resolver: (value: unknown) => void;

    // Construct a promise we know will only resolve after listener.accept() has
    // been called enough times to assert on our expected backoff delays, i.e.
    // the number of rejections + 1 success.
    const expectedBackoffDelaysCompletedPromise = new Promise((resolve) => {
      resolver = resolve;
    });

    const acceptCallSideEffect = ({
      acceptCallCount,
    }: {
      acceptCallCount: number;
    }) => {
      if (acceptCallCount > rejectionCount + 1) {
        resolver(undefined);
      }
    };

    const conn = createMockConn();
    const rejectionError = new Deno.errors.NotConnected(
      "test-socket-closed-error",
    );

    const listener = new MockListener({
      conn,
      rejectionError,
      rejectionCount,
      acceptCallSideEffect,
    });

    const handler = () => new Response();
    const server = new Server({ handler });
    const servePromise = server.serve(listener);

    // Wait for all the expected failures / backoff periods to have completed.
    await expectedBackoffDelaysCompletedPromise;

    server.close();
    await servePromise;

    listener.acceptCallIntervals.shift();
    console.log("\n Accept call intervals vs expected backoff intervals:");
    console.table(
      listener.acceptCallIntervals.map((col, i) => [
        col,
        expectedBackoffDelays[i] ?? "<1000, reset",
      ]),
    );

    // Assert that the time between the accept calls is greater than or equal to
    // the expected backoff delay.
    for (let i = 0; i < rejectionCount; i++) {
      assertEquals(
        listener.acceptCallIntervals[i] >= expectedBackoffDelays[i],
        true,
      );
    }

    // Assert that the backoff delay has been reset following successfully
    // accepting a connection, i.e. it doesn't remain at 1000ms.
    assertEquals(listener.acceptCallIntervals[rejectionCount] < 1000, true);
  },
);

Deno.test("Server should not leak async ops when closed", () => {
  const hostname = "127.0.0.1";
  const port = getPort();
  const handler = () => new Response();
  const server = new Server({ port, hostname, handler });
  server.listenAndServe();
  server.close();
  // Otherwise, the test would fail with: AssertionError: Test case is leaking async ops.
});

Deno.test("Server should abort accept backoff delay when closing", async () => {
  const hostname = "127.0.0.1";
  const port = getPort();
  const handler = () => new Response();

  const rejectionError = new Deno.errors.NotConnected(
    "test-socket-closed-error",
  );
  const rejectionCount = 1;
  const conn = createMockConn();

  const listener = new MockListener({
    conn,
    rejectionError,
    rejectionCount,
  });

  const server = new Server({ port, hostname, handler });
  server.serve(listener);

  // Wait until the connection is rejected and the backoff delay starts.
  await delay(0);

  // Close the server, this should end the test without still having an active timer that would trigger an
  // AssertionError: Test case is leaking async ops.
  server.close();
});

Deno.test("Server should reject if the listener throws an unexpected error accepting a connection", async () => {
  const conn = createMockConn();
  const rejectionError = new Error("test-unexpected-error");
  const listener = new MockListener({ conn, rejectionError });
  const handler = () => new Response();
  const server = new Server({ handler });
  await assertRejects(
    () => server.serve(listener),
    Error,
    rejectionError.message,
  );
});

Deno.test(
  "Server should reject if the listener throws an unexpected error accepting a connection",
  async () => {
    const conn = createMockConn();
    const rejectionError = new Error("test-unexpected-error");
    const listener = new MockListener({ conn, rejectionError });
    const handler = () => new Response();
    const server = new Server({ handler });
    await assertRejects(
      () => server.serve(listener),
      Error,
      rejectionError.message,
    );
  },
);

Deno.test(
  "Server should not reject when the connection is closed before the message is complete",
  async () => {
    const listenOptions = {
      hostname: "localhost",
      port: getPort(),
    };
    const listener = Deno.listen(listenOptions);

    const onRequest = Promise.withResolvers<void>();
    const postRespondWith = Promise.withResolvers<void>();

    const handler = async () => {
      onRequest.resolve();

      await delay(0);

      try {
        return new Response("test-response");
      } finally {
        postRespondWith.resolve();
      }
    };

    const server = new Server({ handler });
    const servePromise = server.serve(listener);

    const conn = await Deno.connect(listenOptions);

    await writeAll(conn, new TextEncoder().encode(`GET / HTTP/1.0\r\n\r\n`));

    await onRequest.promise;
    conn.close();

    await postRespondWith.promise;
    server.close();

    await servePromise;
  },
);

Deno.test("Server should not reject when the handler throws", async () => {
  const listenOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listener = Deno.listen(listenOptions);

  const postRespondWith = Promise.withResolvers<void>();

  const handler = () => {
    try {
      throw new Error("test-error");
    } finally {
      postRespondWith.resolve();
    }
  };

  const server = new Server({ handler });
  const servePromise = server.serve(listener);

  const conn = await Deno.connect(listenOptions);

  await writeAll(conn, new TextEncoder().encode(`GET / HTTP/1.0\r\n\r\n`));

  await postRespondWith.promise;
  conn.close();
  server.close();
  await servePromise;
});

Deno.test("Server should not close the http2 downstream connection when the response stream throws", async () => {
  const listenOptions = {
    hostname: "localhost",
    port: getPort(),
    certFile: join(testdataDir, "tls/localhost.crt"),
    keyFile: join(testdataDir, "tls/localhost.key"),
    alpnProtocols: ["h2"],
  };
  const listener = Deno.listenTls(listenOptions);
  const url = `https://${listenOptions.hostname}:${listenOptions.port}/`;

  let n = 0;
  const a = Promise.withResolvers<void>();
  const connections = new Set();

  const handler = (_req: Request, connInfo: ConnInfo) => {
    connections.add(connInfo);
    return new Response(
      new ReadableStream({
        async start(controller) {
          n++;
          if (n === 3) {
            throw new Error("test-error");
          }
          await a.promise;
          controller.enqueue(new TextEncoder().encode("a"));
          controller.close();
        },
      }),
    );
  };

  const server = new Server({ handler });
  const servePromise = server.serve(listener);

  const caCert = await Deno.readTextFile(
    join(testdataDir, "tls/RootCA.pem"),
  );
  const client = Deno.createHttpClient({
    caCerts: [caCert],
  });
  const resp1 = await fetch(url, { client });
  const resp2 = await fetch(url, { client });

  const err = await assertRejects(async () => {
    const resp3 = await fetch(url, { client });
    const _data = await resp3.text();
  });
  assert(err);
  a.resolve();
  assertEquals(await resp1.text(), "a");
  assertEquals(await resp2.text(), "a");

  const numConns = connections.size;
  assertEquals(
    numConns,
    1,
    `fetch should have reused a single connection, but used ${numConns} instead.`,
  );
  assertEquals(n, 3, "The handler should have been called three times");

  client.close();
  server.close();
  await servePromise;
});

Deno.test("Server should be able to parse IPV6 addresses", async () => {
  const hostname = "[::1]";
  const port = getPort();
  const url = `http://${hostname}:${port}`;
  const method = "GET";
  const status = 418;
  const body = `${method}: ${url} - Hello Deno on HTTP!`;

  const handler = () => new Response(body, { status });
  const abortController = new AbortController();

  const servePromise = serve(handler, {
    hostname,
    port,
    signal: abortController.signal,
  });

  try {
    const response = await fetch(url, { method });
    assertEquals(await response.text(), body);
    assertEquals(response.status, status);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test("Server.serve can be called multiple times", async () => {
  const listenerOneOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listenerTwoOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listenerOne = Deno.listen(listenerOneOptions);
  const listenerTwo = Deno.listen(listenerTwoOptions);

  const handler = (_request: Request, connInfo: ConnInfo) => {
    if ((connInfo.localAddr as Deno.NetAddr).port === listenerOneOptions.port) {
      return new Response("Hello listener one!");
    } else if (
      (connInfo.localAddr as Deno.NetAddr).port === listenerTwoOptions.port
    ) {
      return new Response("Hello listener two!");
    }

    unreachable();
  };

  const server = new Server({ handler });
  const servePromiseOne = server.serve(listenerOne);
  const servePromiseTwo = server.serve(listenerTwo);

  try {
    const responseOne = await fetch(
      `http://${listenerOneOptions.hostname}:${listenerOneOptions.port}`,
    );
    assertEquals(await responseOne.text(), "Hello listener one!");

    const responseTwo = await fetch(
      `http://${listenerTwoOptions.hostname}:${listenerTwoOptions.port}`,
    );
    assertEquals(await responseTwo.text(), "Hello listener two!");
  } finally {
    server.close();
    await servePromiseOne;
    await servePromiseTwo;
  }
});

Deno.test(
  "Server.listenAndServe should throw if called multiple times",
  async () => {
    const handler = () => unreachable();

    const server = new Server({ port: 4505, handler });
    const servePromise = server.listenAndServe();

    try {
      await assertRejects(() => server.listenAndServe(), Deno.errors.AddrInUse);
    } finally {
      server.close();
      await servePromise;
    }
  },
);

Deno.test(
  "Server.listenAndServeTls should throw if called multiple times",
  async () => {
    const handler = () => unreachable();

    const certFile = join(testdataDir, "tls/localhost.crt");
    const keyFile = join(testdataDir, "tls/localhost.key");

    const server = new Server({ port: 4505, handler });
    const servePromise = server.listenAndServeTls(certFile, keyFile);

    try {
      await assertRejects(
        () => server.listenAndServeTls(certFile, keyFile),
        Deno.errors.AddrInUse,
      );
    } finally {
      server.close();
      await servePromise;
    }
  },
);

Deno.test(
  "Handler is called with the request instance and connection information",
  async () => {
    const hostname = "127.0.0.1";
    const port = getPort();
    const addr = `${hostname}:${port}`;

    let receivedRequest: Request;
    let receivedConnInfo: ConnInfo;

    const handler = (request: Request, connInfo: ConnInfo) => {
      receivedRequest = request;
      receivedConnInfo = connInfo;

      return new Response("Hello Deno!");
    };

    const server = new Server({ hostname, port, handler });
    const servePromise = server.listenAndServe();

    const url = `http://${addr}/`;

    try {
      const response = await fetch(url);
      await response.text();

      assertEquals(receivedRequest!.url, url);
      assertEquals(receivedConnInfo!.localAddr.transport, "tcp");
      assertEquals(
        (receivedConnInfo!.localAddr as Deno.NetAddr).hostname,
        hostname,
      );
      assertEquals((receivedConnInfo!.localAddr as Deno.NetAddr).port, port);
      assertEquals(receivedConnInfo!.remoteAddr.transport, "tcp");
      assertEquals(
        (receivedConnInfo!.remoteAddr as Deno.NetAddr).hostname,
        hostname,
      );
    } finally {
      server.close();
      await servePromise;
    }
  },
);

Deno.test("Default onError is called when Handler throws", async () => {
  const hostname = "localhost";
  const port = getPort();
  const url = `http://${hostname}:${port}`;
  const handler = (_request: Request, _connInfo: ConnInfo) => {
    throw new Error("I failed to serve the request");
  };
  const abortController = new AbortController();

  const servePromise = serve(handler, {
    hostname,
    port,
    signal: abortController.signal,
  });

  try {
    const response = await fetch(url);
    assertEquals(await response.text(), "Internal Server Error");
    assertEquals(response.status, 500);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test("Custom onError is called when Handler throws", async () => {
  const hostname = "localhost";
  const port = getPort();
  const url = `http://${hostname}:${port}`;
  const handler = (_request: Request, _connInfo: ConnInfo) => {
    throw new Error("I failed to serve the request");
  };
  const onError = (_error: unknown) => {
    return new Response("custom error page", { status: 500 });
  };
  const abortController = new AbortController();

  const servePromise = serve(handler, {
    hostname,
    port,
    onError,
    signal: abortController.signal,
  });

  try {
    const response = await fetch(url);
    assertEquals(await response.text(), "custom error page");
    assertEquals(response.status, 500);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test("Custom onError is called when Handler throws", async () => {
  const listenOptions = {
    hostname: "localhost",
    port: getPort(),
  };
  const listener = Deno.listen(listenOptions);

  const url = `http://${listenOptions.hostname}:${listenOptions.port}`;
  const handler = (_request: Request, _connInfo: ConnInfo) => {
    throw new Error("I failed to serve the request");
  };
  const onError = (_error: unknown) => {
    return new Response("custom error page", { status: 500 });
  };
  const abortController = new AbortController();

  const servePromise = serveListener(listener, handler, {
    onError,
    signal: abortController.signal,
  });

  try {
    const response = await fetch(url);
    assertEquals(await response.text(), "custom error page");
    assertEquals(response.status, 500);
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test("Server.listenAndServeTls should support custom onError", async () => {
  const hostname = "localhost";
  const port = getPort();
  const certFile = join(testdataDir, "tls/localhost.crt");
  const keyFile = join(testdataDir, "tls/localhost.key");
  const status = 500;
  const method = "PATCH";
  const body = "custom error page";

  const handler = () => {
    throw new Error("I failed to serve the request.");
  };
  const onError = (_error: unknown) => new Response(body, { status });
  const abortController = new AbortController();

  const servePromise = serveTls(handler, {
    hostname,
    port,
    certFile,
    keyFile,
    onError,
    signal: abortController.signal,
  });

  try {
    const conn = await Deno.connectTls({
      hostname,
      port,
      certFile: join(testdataDir, "tls/RootCA.pem"),
    });

    await writeAll(
      conn,
      new TextEncoder().encode(
        `${method.toUpperCase()} / HTTP/1.0\r\n\r\n`,
      ),
    );

    const response = new TextDecoder().decode(await readAll(conn));

    conn.close();

    assert(
      response.includes(`HTTP/1.0 ${status}`),
      "Status code not correct",
    );
    assert(
      response.includes(body),
      "Response body not correct",
    );
  } finally {
    abortController.abort();
    await servePromise;
  }
});

Deno.test("serve - onListen callback is called when the server started listening", () => {
  const abortController = new AbortController();
  return serve((_) => new Response("hello"), {
    async onListen({ hostname, port }) {
      const responseText = await (await fetch("http://localhost:8000/")).text();
      assertEquals(hostname, "0.0.0.0");
      assertEquals(port, 8000);
      assertEquals(responseText, "hello");
      abortController.abort();
    },
    signal: abortController.signal,
  });
});

Deno.test("serve - onListen callback is called with ephemeral port", () => {
  const abortController = new AbortController();
  return serve((_) => new Response("hello"), {
    port: 0,
    async onListen({ hostname, port }) {
      assertEquals(hostname, "0.0.0.0");
      assertNotEquals(port, 0);
      const responseText = await (await fetch(`http://localhost:${port}/`))
        .text();
      assertEquals(responseText, "hello");
      abortController.abort();
    },
    signal: abortController.signal,
  });
});

Deno.test("serve - doesn't print the message when onListen set to undefined", async () => {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      `
        import { serve } from "./http/server.ts";
        serve(() => new Response("hello"), { onListen: undefined });
        Deno.exit(0);
      `,
    ],
  });
  const { code, stdout } = await command.output();
  assertEquals(code, 0);
  assertEquals(new TextDecoder().decode(stdout), "");
});

Deno.test("serve - can print customized start-up message in onListen handler", async () => {
  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "eval",
      `
        import { serve } from "./http/server.ts";
        serve(() => new Response("hello"), { onListen({ port, hostname }) {
          console.log("Server started at " + hostname + " port " + port);
        } });
        Deno.exit(0);
      `,
    ],
  });
  const { stdout, code } = await command.output();
  assertEquals(code, 0);
  assertEquals(
    new TextDecoder().decode(stdout),
    "Server started at 0.0.0.0 port 8000\n",
  );
});

Deno.test("serveTls - onListen callback is called with ephemeral port", () => {
  const abortController = new AbortController();
  return serveTls((_) => new Response("hello"), {
    port: 0,
    certFile: join(testdataDir, "tls/localhost.crt"),
    keyFile: join(testdataDir, "tls/localhost.key"),
    async onListen({ hostname, port }) {
      assertEquals(hostname, "0.0.0.0");
      assertNotEquals(port, 0);
      const caCert = await Deno.readTextFile(
        join(testdataDir, "tls/RootCA.pem"),
      );
      const client = Deno.createHttpClient({ caCerts: [caCert] });
      const responseText =
        await (await fetch(`https://localhost:${port}/`, { client }))
          .text();
      client.close();
      assertEquals(responseText, "hello");
      abortController.abort();
    },
    signal: abortController.signal,
  });
});

Deno.test("serveTls - cert, key can be injected directly from memory rather than file system.", () => {
  const abortController = new AbortController();
  return serveTls((_) => new Response("hello"), {
    port: 0,
    cert: Deno.readTextFileSync(join(testdataDir, "tls/localhost.crt")),
    key: Deno.readTextFileSync(join(testdataDir, "tls/localhost.key")),
    async onListen({ hostname, port }) {
      assertEquals(hostname, "0.0.0.0");
      assertNotEquals(port, 0);
      const caCert = await Deno.readTextFile(
        join(testdataDir, "tls/RootCA.pem"),
      );
      const client = Deno.createHttpClient({ caCerts: [caCert] });
      const responseText = await (
        await fetch(`https://localhost:${port}/`, { client })
      ).text();
      client.close();
      assertEquals(responseText, "hello");
      abortController.abort();
    },
    signal: abortController.signal,
  });
});

Deno.test("serve - doesn't throw with string port number", () => {
  const ac = new AbortController();
  return serve((_) => new Response("hello"), {
    // deno-lint-ignore no-explicit-any
    port: "0" as any,
    onListen() {
      ac.abort();
    },
    signal: ac.signal,
  });
});

Deno.test("serveTls - doesn't throw with string port number", () => {
  const ac = new AbortController();
  return serveTls((_) => new Response("hello"), {
    // deno-lint-ignore no-explicit-any
    port: "0" as any,
    cert: Deno.readTextFileSync(join(testdataDir, "tls/localhost.crt")),
    key: Deno.readTextFileSync(join(testdataDir, "tls/localhost.key")),
    onListen() {
      ac.abort();
    },
    signal: ac.signal,
  });
});
