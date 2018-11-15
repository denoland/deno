// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

testPerm({ net: true }, async function fetchJsonSuccess() {
  const response = await fetch("http://localhost:4545/package.json");
  const json = await response.json();
  assertEqual(json.name, "deno");
});

test(async function fetchPerm() {
  let err;
  try {
    await fetch("http://localhost:4545/package.json");
  } catch (err_) {
    err = err_;
  }
  assertEqual(err.kind, deno.ErrorKind.PermissionDenied);
  assertEqual(err.name, "PermissionDenied");
});

testPerm({ net: true }, async function fetchHeaders() {
  const response = await fetch("http://localhost:4545/package.json");
  const headers = response.headers;
  assertEqual(headers.get("Content-Type"), "application/json");
  assert(headers.get("Server").startsWith("SimpleHTTP"));
});

testPerm({ net: true }, async function fetchBlob() {
  const response = await fetch("http://localhost:4545/package.json");
  const headers = response.headers;
  const blob = await response.blob();
  assertEqual(blob.type, headers.get("Content-Type"));
  assertEqual(blob.size, Number(headers.get("Content-Length")));
});

testPerm({ net: true }, async function responseClone() {
  const response = await fetch("http://localhost:4545/package.json");
  const response1 = response.clone();
  assert(response !== response1);
  assertEqual(response.status, response1.status);
  assertEqual(response.statusText, response1.statusText);
  const ab = await response.arrayBuffer();
  const ab1 = await response1.arrayBuffer();
  for (let i = 0; i < ab.byteLength; i++) {
    assertEqual(ab[i], ab1[i]);
  }
});

testPerm({ net: true }, async function fetchRequest() {
  const addr = "127.0.0.1:4501";
  const listener = deno.listen("tcp", addr);
  const buf = new deno.Buffer();
  listener.accept().then(async conn => {
    buf.readFrom(conn);
    await conn.write(
      new TextEncoder().encode(
        "HTTP/1.0 404 Not Found\r\nContent-Length: 2\r\n\r\nNF"
      )
    );
    conn.close();
  });
  const response = await fetch(`http://${addr}/blah`, {
    method: "POST",
    headers: [["Hello", "World"], ["Foo", "Bar"]]
  });
  listener.close();
  assertEqual(response.status, 404);
  assertEqual(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n\r\n`
  ].join("");
  assertEqual(actual, expected);
});
