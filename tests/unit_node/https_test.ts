// Copyright 2018-2025 the Deno authors. MIT license.

import https from "node:https";
import { assert } from "../unit/test_util.ts";

Deno.test({
  name:
    "request.socket.authorized is true when successfully requested to https server",
  async fn() {
    const server = Deno.serve({
      port: 0,
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
      onListen({ port }) {
        const req = https.request(`https://localhost:${port}`, (res) => {
          // deno-lint-ignore no-explicit-any
          assert((req.socket as any).authorized);
          res.destroy();
          server.shutdown();
        });
      },
    }, () => {
      return new Response("hi");
    });
    await server.finished;
  },
});
