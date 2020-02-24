// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Server, serve } from "../../http/server.ts";
import { assertStrictEq } from "../../testing/asserts.ts";

let server: Server | undefined;

async function startTestServer(): Promise<void> {
  server = await serve({ port: 8080 });
  (async (): Promise<void> => {
    for await (const req of server) {
      req.respond({ body: "Hello world" });
    }
  })();
}

Deno.test("[examples/curl] beforeAll", async () => {
  await startTestServer();
});

Deno.test("[examples/curl] send a request to a specified url", async () => {
  const decoder = new TextDecoder();
  const process = Deno.run({
    args: [Deno.execPath(), "--allow-net", "curl.ts", "http://localhost:8080"],
    cwd: "examples",
    stdout: "piped"
  });

  try {
    const output = await Deno.readAll(process.stdout!);
    const actual = decoder.decode(output).trim();
    const expected = "Hello world";

    assertStrictEq(actual, expected);
  } finally {
    process.close();
  }
});

Deno.test("[examples/curl] afterAll", () => {
  server?.close();
});
