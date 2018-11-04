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
