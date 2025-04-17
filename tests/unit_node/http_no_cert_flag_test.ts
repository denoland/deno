// Copyright 2018-2025 the Deno authors. MIT license.

import https from "node:https";
import net from "node:net";

import { assert, assertEquals } from "@std/assert";

Deno.test("[node/https] request directly with key and cert as arguments", async () => {
  let body = "";
  const deferred1 = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<void>();
  const server = https.createServer(
    {
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
    },
    (req, res) => {
      // @ts-ignore: It exists on TLSSocket
      assert(req.socket.encrypted);
      res.end("success!");
    },
  );
  server.listen(() => {
    const req = https.request({
      method: "GET",
      hostname: "localhost",
      port: (server.address() as net.AddressInfo).port,
      key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
      ca: Deno.readTextFileSync("tests/testdata/tls/RootCA.pem"),
    }, (resp) => {
      resp.on("data", (chunk) => {
        body += chunk;
      });

      resp.on("end", () => {
        deferred2.resolve();
        server.close();
      });
    });
    req.on("error", (e) => deferred2.reject(e));
    req.end();
  });

  server.on("close", () => deferred1.resolve());
  server.on("error", (e) => deferred1.reject(e));

  await deferred1.promise;
  await deferred2.promise;
  assertEquals(body, "success!");
});

Deno.test("[node/https] request directly with key and cert as agent", async () => {
  let body = "";
  const deferred1 = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<void>();
  const server = https.createServer(
    {
      cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
      key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
    },
    (req, res) => {
      // @ts-ignore: It exists on TLSSocket
      assert(req.socket.encrypted);
      res.end("success!");
    },
  );
  server.listen(() => {
    const req = https.request({
      method: "GET",
      hostname: "localhost",
      port: (server.address() as never as net.AddressInfo).port,
      agent: new https.Agent({
        key: Deno.readTextFileSync("tests/testdata/tls/localhost.key"),
        cert: Deno.readTextFileSync("tests/testdata/tls/localhost.crt"),
        ca: Deno.readTextFileSync("tests/testdata/tls/RootCA.pem"),
      }),
    }, (resp) => {
      resp.on("data", (chunk) => {
        body += chunk;
      });

      resp.on("end", () => {
        deferred2.resolve();
        server.close();
      });
    });
    req.on("error", (e) => deferred2.reject(e));
    req.end();
  });

  server.on("close", () => deferred1.resolve());
  server.on("error", (e) => deferred1.reject(e));

  await deferred1.promise;
  await deferred2.promise;
  assertEquals(body, "success!");
});
