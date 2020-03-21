// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve } from "../../http/server.ts";
import { assertStrictEq } from "../../testing/asserts.ts";
import { randomPort } from "../../http/test_util.ts";

const port = randomPort();
Deno.test({
  name: "[examples/curl] send a request to a specified url",
  // FIXME(bartlomieju): this test is leaking both resources and ops,
  // and causes interference with other tests
  ignore: true,
  fn: async () => {
    const server = serve({ port });
    (async (): Promise<void> => {
      for await (const req of server) {
        req.respond({ body: "Hello world" });
      }
    })();

    const decoder = new TextDecoder();
    const process = Deno.run({
      cmd: [
        Deno.execPath(),
        "--allow-net",
        "curl.ts",
        "http://localhost:" + port
      ],
      cwd: "examples",
      stdout: "piped"
    });

    try {
      const output = await process.output();
      const actual = decoder.decode(output).trim();
      const expected = "Hello world";

      assertStrictEq(actual, expected);
    } finally {
      process.close();
      server.close();
    }
  }
});
