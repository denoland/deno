// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "../../std/http/server.ts";
import { assertEquals } from "../../std/testing/asserts.ts";
import { serveFile } from "../../std/http/file_server.ts";
import * as path from "../../std/path/mod.ts";

const addr = Deno.args[1] || "127.0.0.1:4555";
const __dirname = path.dirname(path.fromFileUrl(import.meta.url));
const filePath = path.join(__dirname, "./fetch_data/large_file/bigfile.txt");

async function runServer(): Promise<void> {
  const server = serve(addr);

  console.log(`Proxy server listening on http://${addr}/`);
  for await (const req of server) {
    req.respond(await serveFile(req, filePath));
  }
}

async function testFetch(): Promise<void> {
  const [remoteContent, localContent] = await Promise.all([
    new Promise<string>((res, rej) => {
      fetch(`http://${addr}/`)
        .then((response) => {
          assertEquals(response.status, 200);
          res(response.text());
        });
    }),
    Deno.readTextFile(filePath),
  ]);
  assertEquals(remoteContent, localContent);
}

runServer();
await testFetch();
Deno.exit(0);
