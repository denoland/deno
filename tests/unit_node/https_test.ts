// Copyright 2018-2026 the Deno authors. MIT license.

import http from "node:http";
import https from "node:https";
import { assert, assertEquals } from "../unit/test_util.ts";
import type { AddressInfo } from "node:net";

Deno.test("[node/https] Server.address() includes family property", async () => {
  const certFile = "tests/testdata/tls/localhost.crt";
  const keyFile = "tests/testdata/tls/localhost.key";

  // Test IPv4
  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = https.createServer({
      cert: Deno.readTextFileSync(certFile),
      key: Deno.readTextFileSync(keyFile),
    }, (_req, res) => {
      res.end("ok");
    });

    server.listen(0, "127.0.0.1", () => {
      const addr = server.address() as AddressInfo;
      assertEquals(addr.address, "127.0.0.1");
      assertEquals(addr.family, "IPv4");
      assertEquals(typeof addr.port, "number");
      server.close(() => resolve());
    });

    await promise;
  }

  // Test IPv6
  {
    const { promise, resolve } = Promise.withResolvers<void>();
    const server = https.createServer({
      cert: Deno.readTextFileSync(certFile),
      key: Deno.readTextFileSync(keyFile),
    }, (_req, res) => {
      res.end("ok");
    });

    server.listen(0, "::1", () => {
      const addr = server.address() as AddressInfo;
      assertEquals(addr.address, "::1");
      assertEquals(addr.family, "IPv6");
      assertEquals(typeof addr.port, "number");
      server.close(() => resolve());
    });

    await promise;
  }
});

// https://github.com/denoland/deno/issues/31758
Deno.test("[node/https] address() returns assigned port immediately after listen()", async () => {
  const server = https.createServer({
    cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
    key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
  });
  server.listen(0);

  // address() should return the real port synchronously, not 0
  const addr = server.address() as AddressInfo;
  assert(typeof addr.port === "number");
  assert(addr.port > 0, `Expected port > 0, got ${addr.port}`);

  const { promise, resolve } = Promise.withResolvers<void>();
  server.close(() => resolve());
  await promise;
});

Deno.test({
  name:
    "request.socket.authorized is true when successfully requested to https server",
  async fn() {
    const { promise, resolve } = Promise.withResolvers<void>();
    let serverPort: number;
    const server = Deno.serve({
      port: 0,
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
      onListen({ port }) {
        serverPort = port;
        resolve();
      },
    }, () => {
      return new Response("hi");
    });

    await promise;
    const req = https.request(`https://localhost:${serverPort!}`, (res) => {
      // deno-lint-ignore no-explicit-any
      assert((req.socket as any).authorized);
      res.destroy();
      server.shutdown();
    });
    req.end();

    await server.finished;
  },
});

// Regression: `agent-base` (used by `@npmcli/agent`, `http-proxy-agent`, etc.)
// decides whether a polymorphic agent should behave as https by scanning the
// current stack for `node:https:`. Without that marker the agent reports
// `protocol: "http:"`, and `http.ClientRequest` then throws
// ERR_INVALID_PROTOCOL against an https URL — breaking `npm install` and
// anything else that relies on agent-base.
Deno.test("[node/https] request stack frame surfaces 'node:https:' for agent-base", () => {
  // Mirrors agent-base@7's `protocol` getter: if the protocol wasn't set
  // explicitly via the setter, scan the current stack for `node:https:` to
  // decide whether we're inside an https request. The slot is keyed off a
  // Symbol so it's only present after agent-base's own constructor runs —
  // any `this.protocol = ...` assignment from `http.Agent`'s constructor
  // (which runs first) is silently ignored, matching agent-base's behavior.
  const INTERNAL = Symbol("AgentBaseInternalState");
  // deno-lint-ignore no-explicit-any
  class PolymorphicAgent extends (http.Agent as any) {
    // deno-lint-ignore no-explicit-any
    constructor(opts?: any) {
      super(opts);
      // deno-lint-ignore no-explicit-any
      (this as any)[INTERNAL] = {};
    }
    get protocol() {
      // deno-lint-ignore no-explicit-any
      const state = (this as any)[INTERNAL];
      if (state?.protocol !== undefined) return state.protocol;
      const { stack } = new Error();
      return stack && /node:https:/.test(stack) ? "https:" : "http:";
    }
    set protocol(v: string) {
      // deno-lint-ignore no-explicit-any
      const state = (this as any)[INTERNAL];
      if (state) state.protocol = v;
    }
    // Prevent any actual socket/DNS work — we only care about the
    // synchronous validation in `http.ClientRequest`'s constructor.
    addRequest() {}
  }

  const agent = new PolymorphicAgent();
  // Before the fix this threw synchronously with ERR_INVALID_PROTOCOL
  // because `https.request`'s stack frame didn't contain `node:https:`,
  // so the polymorphic agent reported protocol `"http:"` while the URL
  // protocol was `"https:"`.
  // deno-lint-ignore no-explicit-any
  const req = https.request("https://example.com/", { agent: agent as any });
  // We never let the request connect; swallow the abort error and the
  // assertion is simply that construction did not throw.
  req.on("error", () => {});
  req.destroy();
});
