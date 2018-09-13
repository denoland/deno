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

test(async function headersAppend() {
  let err;
  try {
    const headers = new Headers([["foo", "bar", "baz"]]);
  } catch (e) {
    err = e;
  }
  assert(err instanceof TypeError);
});
