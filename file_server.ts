#!/usr/bin/env deno --allow-net

// This program serves files in the current directory over HTTP.
// TODO Stream responses instead of reading them into memory.
// TODO Add tests like these:
// https://github.com/indexzero/http-server/blob/master/test/http-server-test.js

import { listenAndServe } from "./http";
import { cwd, readFile, DenoError, ErrorKind, args } from "deno";

const addr = "0.0.0.0:4500";
let currentDir = cwd();
const target = args[1];
if (target) {
  currentDir = `${currentDir}/${target}`;
}
const encoder = new TextEncoder();

listenAndServe(addr, async req => {
  const fileName = req.url.replace(/\/$/, '/index.html');
  const filePath = currentDir + fileName;
  let file;

  try {
    file = await readFile(filePath);
  } catch (e) {
    if (e instanceof DenoError && e.kind === ErrorKind.NotFound) {
      await req.respond({ status: 404, body: encoder.encode("Not found") });  
    } else {
      await req.respond({ status: 500, body: encoder.encode("Internal server error") });
    }
    return;
  }
  
  const headers = new Headers();
  headers.set('content-type', 'octet-stream');

  const res = {
    status: 200,
    body: file,
    headers,
  }
  await req.respond(res);
});

console.log(`HTTP server listening on http://${addr}/`);
