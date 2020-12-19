// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve } from "../http/server.ts";
import { assertStrictEquals } from "../testing/asserts.ts";
import { dirname, fromFileUrl } from "../path/mod.ts";

const moduleDir = dirname(fromFileUrl(import.meta.url));

Deno.test({
  name: "[examples/curl] send a request to a specified url",
  fn: async () => {
    const server = serve({ port: 8081 });
    const serverPromise = (async (): Promise<void> => {
      for await (const req of server) {
        req.respond({ body: "Hello world" });
      }
    })();

    const decoder = new TextDecoder();
    const process = Deno.run({
      cmd: [
        Deno.execPath(),
        "run",
        "--quiet",
        "--allow-net",
        "curl.ts",
        "http://localhost:8081",
      ],
      cwd: moduleDir,
      stdout: "piped",
    });

    try {
      const output = await process.output();
      const actual = decoder.decode(output).trim();
      const expected = "Hello world";

      assertStrictEquals(actual, expected);
    } finally {
      server.close();
      process.close();
      await serverPromise;
    }
  },
});
