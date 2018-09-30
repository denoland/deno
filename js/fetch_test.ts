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

testPerm({ net: true }, async function fetchBlob() {
  const response = await fetch("http://localhost:4545/package.json");
  const headers = response.headers;
  const blob = await response.blob();
  assertEqual(blob.type, headers.get("Content-Type"));
  assertEqual(blob.size, Number(headers.get("Content-Length")));
});

// Logic heavily copied from web-platform-tests, make
// sure pass mostly header basic test
/* tslint:disable-next-line:max-line-length */
// ref: https://github.com/web-platform-tests/wpt/blob/7c50c216081d6ea3c9afe553ee7b64534020a1b2/fetch/api/headers/headers-basic.html
/* tslint:disable:no-unused-expression */
test(function newHeaderTest() {
  new Headers();
  new Headers(undefined);
  new Headers({});
  try {
    new Headers(null);
  } catch (e) {
    assertEqual(e.message, "Failed to construct 'Headers': Invalid value");
  }

  try {
    const init = [["a", "b", "c"]];
    new Headers(init);
  } catch (e) {
    assertEqual(e.message, "Failed to construct 'Headers': Invalid value");
  }
});

const headerDict = {
  name1: "value1",
  name2: "value2",
  name3: "value3",
  name4: undefined,
  "Content-Type": "value4"
};
const headerSeq = [];
for (const name in headerDict) {
  headerSeq.push([name, headerDict[name]]);
}

test(function newHeaderWithSequence() {
  const headers = new Headers(headerSeq);
  for (const name in headerDict) {
    assertEqual(headers.get(name), String(headerDict[name]));
  }
  assertEqual(headers.get("length"), null);
});

test(function newHeaderWithRecord() {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assertEqual(headers.get(name), String(headerDict[name]));
  }
});

test(function newHeaderWithHeadersInstance() {
  const headers = new Headers(headerDict);
  const headers2 = new Headers(headers);
  for (const name in headerDict) {
    assertEqual(headers2.get(name), String(headerDict[name]));
  }
});

test(function headerAppendSuccess() {
  const headers = new Headers();
  for (const name in headerDict) {
    headers.append(name, headerDict[name]);
    assertEqual(headers.get(name), String(headerDict[name]));
  }
});

test(function headerSetSuccess() {
  const headers = new Headers();
  for (const name in headerDict) {
    headers.set(name, headerDict[name]);
    assertEqual(headers.get(name), String(headerDict[name]));
  }
});

test(function headerHasSuccess() {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assert(headers.has(name), "headers has name " + name);
    /* tslint:disable-next-line:max-line-length */
    assert(
      !headers.has("nameNotInHeaders"),
      "headers do not have header: nameNotInHeaders"
    );
  }
});

test(function headerDeleteSuccess() {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assert(headers.has(name), "headers have a header: " + name);
    headers.delete(name);
    assert(!headers.has(name), "headers do not have anymore a header: " + name);
  }
});

test(function headerGetSuccess() {
  const headers = new Headers(headerDict);
  for (const name in headerDict) {
    assertEqual(headers.get(name), String(headerDict[name]));
    assertEqual(headers.get("nameNotInHeaders"), null);
  }
});

const headerEntriesDict = {
  name1: "value1",
  Name2: "value2",
  name: "value3",
  "content-Type": "value4",
  "Content-Typ": "value5",
  "Content-Types": "value6"
};

test(function headerForEachSuccess() {
  const headers = new Headers(headerEntriesDict);
  const keys = Object.keys(headerEntriesDict);
  keys.forEach(key => {
    const value = headerEntriesDict[key];
    const newkey = key.toLowerCase();
    headerEntriesDict[newkey] = value;
  });
  let callNum = 0;
  headers.forEach((value, key, container) => {
    assertEqual(headers, container);
    assertEqual(value, headerEntriesDict[key]);
    callNum++;
  });
  assertEqual(callNum, keys.length);
});
