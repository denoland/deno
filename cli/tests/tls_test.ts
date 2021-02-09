// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../../test_util/std/testing/asserts.ts";
import { BufReader } from "../../test_util/std/io/bufio.ts";
import { TextProtoReader } from "../../test_util/std/textproto/mod.ts";

Deno.test("connect with client certificate", async () => {
  const conn = await Deno.connectTls({
    hostname: "localhost",
    port: 4244,
    certChain: await Deno.readTextFile("tests/tls/localhost.crt"),
    privateKey: await Deno.readTextFile("tests/tls/localhost.key"),
    certFile: "tests/tls/RootCA.pem",
  });

  const reader = new TextProtoReader(new BufReader(conn));
  const result = await reader.readLine() as string;

  // Server will respond with PASS if client authentication was successful.
  assertEquals(result, "PASS");

  conn.close();
});
