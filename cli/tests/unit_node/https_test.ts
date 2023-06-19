// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { serveTls } from "../../../test_util/std/http/server.ts";
import { fromFileUrl, join } from "../../../test_util/std/path/mod.ts";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import { Agent } from "node:https";

const tlsTestdataDir = fromFileUrl(
  new URL("../testdata/tls", import.meta.url),
);
const keyFile = join(tlsTestdataDir, "localhost.key");
const certFile = join(tlsTestdataDir, "localhost.crt");
const dec = new TextDecoder();
const httpsRequestFilePath = "cli/tests/unit_node/testdata/https_request.ts";
// todo: restore "repetitions" to the original value of 1_000
const repetitions = 1_0;

Deno.test("[node/https] request makes https request", async () => {
  const controller = new AbortController();
  const signal = controller.signal;

  const serveFinish = serveTls((_req) => {
    return new Response("abcd\n".repeat(repetitions));
  }, { keyFile, certFile, port: 4505, hostname: "localhost", signal });

  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--unstable",
      "--allow-all",
      "--no-check",
      httpsRequestFilePath,
    ],
    env: {
      NODE_EXTRA_CA_CERTS: join(tlsTestdataDir, "RootCA.pem"),
    },
  });
  const { stderr, stdout } = await command.output();
  assertEquals(dec.decode(stderr), "");
  assertEquals(dec.decode(stdout), "abcd\n".repeat(repetitions) + "\n");
  controller.abort();
  await serveFinish;
});

Deno.test("[node/https] get makes https GET request", async () => {
  const controller = new AbortController();
  const signal = controller.signal;

  const serveFinish = serveTls((_req) => {
    return new Response("abcd\n".repeat(repetitions));
  }, { keyFile, certFile, port: 4505, hostname: "localhost", signal });

  const command = new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--unstable",
      "--allow-all",
      "--no-check",
      httpsRequestFilePath,
    ],
    env: {
      NODE_EXTRA_CA_CERTS: join(tlsTestdataDir, "RootCA.pem"),
    },
  });
  const { stdout, stderr } = await command.output();
  assertEquals(dec.decode(stderr), "");
  assertEquals(dec.decode(stdout), "abcd\n".repeat(repetitions) + "\n");
  controller.abort();
  await serveFinish;
});

Deno.test("new Agent doesn't throw", () => {
  new Agent();
});
