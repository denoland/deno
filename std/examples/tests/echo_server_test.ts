// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertStrictEq, assertNotEquals } from "../../testing/asserts.ts";
import { BufReader, ReadLineResult } from "../../io/bufio.ts";
import { randomPort } from "../../http/test_util.ts";
const port = randomPort();

Deno.test("[examples/echo_server]", async () => {
  const encoder = new TextEncoder();
  const decoder = new TextDecoder();
  const process = Deno.run({
    cmd: [Deno.execPath(), "--allow-net", "echo_server.ts", `${port}`],
    cwd: "examples",
    stdout: "piped"
  });

  let conn: Deno.Conn | undefined;
  try {
    const processReader = new BufReader(process.stdout!);
    const message = await processReader.readLine();

    assertNotEquals(message, Deno.EOF);
    assertStrictEq(
      decoder.decode((message as ReadLineResult).line).trim(),
      "Listening on 0.0.0.0:" + port
    );

    conn = await Deno.connect({ hostname: "127.0.0.1", port });
    const connReader = new BufReader(conn);

    await conn.write(encoder.encode("Hello echo_server\n"));
    const result = await connReader.readLine();

    assertNotEquals(result, Deno.EOF);

    const actualResponse = decoder
      .decode((result as ReadLineResult).line)
      .trim();
    const expectedResponse = "Hello echo_server";

    assertStrictEq(actualResponse, expectedResponse);
  } finally {
    conn?.close();
    process.stdout!.close();
    process.close();
  }
});
