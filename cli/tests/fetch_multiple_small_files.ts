// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "../../std/http/server.ts";
import { assertEquals } from "../../std/testing/asserts.ts";
import { serveFile } from "../../std/http/file_server.ts";
import * as path from "../../std/path/mod.ts";

const addr = Deno.args[1] || "127.0.0.1:4555";
const __dirname = path.dirname(path.fromFileUrl(import.meta.url));
const filePath = path.join(__dirname, "./fetch_data/small_files");

async function runServer(): Promise<void> {
  const server = serve(addr);

  console.log(`Proxy server listening on http://${addr}/`);
  for await (const req of server) {
    req.respond(await serveFile(req, path.join(filePath, req.url)));
  }
}

async function testFetch(): Promise<void> {
  const files = Array.from(await Deno.readDirSync(filePath));
  const requests = [];
  for (let i = 0; i < files.length * 10; i++) {
    requests.push(new Promise(async (res, rej) => {
      const [remoteContent, localContent] = await Promise.all([
        new Promise<string>(async (res, rej) => {
          const response = await fetch(`http://${addr}/${files[i % files.length].name}`);
          assertEquals(response.status, 200);
          res(response.text());
        }),
        Deno.readTextFile(path.join(filePath, files[i % files.length].name)),
      ]);
      assertEquals(remoteContent, localContent);
      res();
    }));
  } 
  await Promise.all(requests);  
}

runServer();
await testFetch();
Deno.exit(0);
