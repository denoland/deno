// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  test,
  testPerm,
  assert,
  assertEquals,
  assertStrContains,
  assertThrows
} from "./test_util.ts";

testPerm({ net: true }, async function fetchConnectionError(): Promise<void> {
  let err;
  try {
    await fetch("http://localhost:4000");
  } catch (err_) {
    err = err_;
  }
  assertEquals(err.kind, Deno.ErrorKind.HttpOther);
  assertEquals(err.name, "HttpOther");
  assertStrContains(err.message, "error trying to connect");
});

testPerm({ net: true }, async function fetchJsonSuccess(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const json = await response.json();
  assertEquals(json.name, "deno");
});

test(async function fetchPerm(): Promise<void> {
  let err;
  try {
    await fetch("http://localhost:4545/cli/tests/fixture.json");
  } catch (err_) {
    err = err_;
  }
  assertEquals(err.kind, Deno.ErrorKind.PermissionDenied);
  assertEquals(err.name, "PermissionDenied");
});

testPerm({ net: true }, async function fetchUrl(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  assertEquals(response.url, "http://localhost:4545/cli/tests/fixture.json");
});

testPerm({ net: true }, async function fetchURL(): Promise<void> {
  const response = await fetch(
    new URL("http://localhost:4545/cli/tests/fixture.json")
  );
  assertEquals(response.url, "http://localhost:4545/cli/tests/fixture.json");
});

testPerm({ net: true }, async function fetchHeaders(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const headers = response.headers;
  assertEquals(headers.get("Content-Type"), "application/json");
  assert(headers.get("Server").startsWith("SimpleHTTP"));
});

testPerm({ net: true }, async function fetchBlob(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const headers = response.headers;
  const blob = await response.blob();
  assertEquals(blob.type, headers.get("Content-Type"));
  assertEquals(blob.size, Number(headers.get("Content-Length")));
});

testPerm({ net: true }, async function fetchBodyUsed(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  assertEquals(response.bodyUsed, false);
  assertThrows((): void => {
    // Assigning to read-only property throws in the strict mode.
    response.bodyUsed = true;
  });
  await response.blob();
  assertEquals(response.bodyUsed, true);
});

testPerm({ net: true }, async function fetchAsyncIterator(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const headers = response.headers;
  let total = 0;
  for await (const chunk of response.body) {
    total += chunk.length;
  }

  assertEquals(total, Number(headers.get("Content-Length")));
});

testPerm({ net: true }, async function responseClone(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const response1 = response.clone();
  assert(response !== response1);
  assertEquals(response.status, response1.status);
  assertEquals(response.statusText, response1.statusText);
  const ab = await response.arrayBuffer();
  const ab1 = await response1.arrayBuffer();
  for (let i = 0; i < ab.byteLength; i++) {
    assertEquals(ab[i], ab1[i]);
  }
});

testPerm({ net: true }, async function fetchEmptyInvalid(): Promise<void> {
  let err;
  try {
    await fetch("");
  } catch (err_) {
    err = err_;
  }
  assertEquals(err.kind, Deno.ErrorKind.RelativeUrlWithoutBase);
  assertEquals(err.name, "RelativeUrlWithoutBase");
});

testPerm({ net: true }, async function fetchMultipartFormDataSuccess(): Promise<
  void
> {
  const response = await fetch(
    "http://localhost:4545/tests/subdir/multipart_form_data.txt"
  );
  const formData = await response.formData();
  assert(formData.has("field_1"));
  assertEquals(formData.get("field_1").toString(), "value_1 \r\n");
  assert(formData.has("field_2"));
  /* TODO(ry) Re-enable this test once we bring back the global File type.
  const file = formData.get("field_2") as File;
  assertEquals(file.name, "file.js");
  */
  // Currently we cannot read from file...
});

testPerm(
  { net: true },
  async function fetchURLEncodedFormDataSuccess(): Promise<void> {
    const response = await fetch(
      "http://localhost:4545/tests/subdir/form_urlencoded.txt"
    );
    const formData = await response.formData();
    assert(formData.has("field_1"));
    assertEquals(formData.get("field_1").toString(), "Hi");
    assert(formData.has("field_2"));
    assertEquals(formData.get("field_2").toString(), "<Deno>");
  }
);

testPerm({ net: true }, async function fetchWithRedirection(): Promise<void> {
  const response = await fetch("http://localhost:4546/"); // will redirect to http://localhost:4545/
  assertEquals(response.status, 200);
  assertEquals(response.statusText, "OK");
  assertEquals(response.url, "http://localhost:4545/");
  const body = await response.text();
  assert(body.includes("<title>Directory listing for /</title>"));
});

testPerm({ net: true }, async function fetchWithRelativeRedirection(): Promise<
  void
> {
  const response = await fetch("http://localhost:4545/tests"); // will redirect to /tests/
  assertEquals(response.status, 200);
  assertEquals(response.statusText, "OK");
  const body = await response.text();
  assert(body.includes("<title>Directory listing for /tests/</title>"));
});

// The feature below is not implemented, but the test should work after implementation
/*
testPerm({ net: true }, async function fetchWithInfRedirection(): Promise<
  void
> {
  const response = await fetch("http://localhost:4549/tests"); // will redirect to the same place
  assertEquals(response.status, 0); // network error
});
*/

testPerm({ net: true }, async function fetchInitStringBody(): Promise<void> {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: data
  });
  const text = await response.text();
  assertEquals(text, data);
  assert(response.headers.get("content-type").startsWith("text/plain"));
});

testPerm({ net: true }, async function fetchRequestInitStringBody(): Promise<
  void
> {
  const data = "Hello World";
  const req = new Request("http://localhost:4545/echo_server", {
    method: "POST",
    body: data
  });
  const response = await fetch(req);
  const text = await response.text();
  assertEquals(text, data);
});

testPerm({ net: true }, async function fetchInitTypedArrayBody(): Promise<
  void
> {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: new TextEncoder().encode(data)
  });
  const text = await response.text();
  assertEquals(text, data);
});

testPerm({ net: true }, async function fetchInitURLSearchParamsBody(): Promise<
  void
> {
  const data = "param1=value1&param2=value2";
  const params = new URLSearchParams(data);
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: params
  });
  const text = await response.text();
  assertEquals(text, data);
  assert(
    response.headers
      .get("content-type")
      .startsWith("application/x-www-form-urlencoded")
  );
});

testPerm({ net: true }, async function fetchInitBlobBody(): Promise<void> {
  const data = "const a = 1";
  const blob = new Blob([data], {
    type: "text/javascript"
  });
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: blob
  });
  const text = await response.text();
  assertEquals(text, data);
  assert(response.headers.get("content-type").startsWith("text/javascript"));
});

testPerm({ net: true }, async function fetchUserAgent(): Promise<void> {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: new TextEncoder().encode(data)
  });
  assertEquals(response.headers.get("user-agent"), `Deno/${Deno.version.deno}`);
  await response.text();
});

// TODO(ry) The following tests work but are flaky. There's a race condition
// somewhere. Here is what one of these flaky failures looks like:
//
// test fetchPostBodyString_permW0N1E0R0
// assertEquals failed. actual =   expected = POST /blah HTTP/1.1
// hello: World
// foo: Bar
// host: 127.0.0.1:4502
// content-length: 11
// hello world
// Error: actual:  expected: POST /blah HTTP/1.1
// hello: World
// foo: Bar
// host: 127.0.0.1:4502
// content-length: 11
// hello world
//     at Object.assertEquals (file:///C:/deno/js/testing/util.ts:29:11)
//     at fetchPostBodyString (file

/*
function bufferServer(addr: string): Deno.Buffer {
  const listener = Deno.listen(addr);
  const buf = new Deno.Buffer();
  listener.accept().then(async conn => {
    const p1 = buf.readFrom(conn);
    const p2 = conn.write(
      new TextEncoder().encode(
        "HTTP/1.0 404 Not Found\r\nContent-Length: 2\r\n\r\nNF"
      )
    );
    // Wait for both an EOF on the read side of the socket and for the write to
    // complete before closing it. Due to keep-alive, the EOF won't be sent
    // until the Connection close (HTTP/1.0) response, so readFrom() can't
    // proceed write. Conversely, if readFrom() is async, waiting for the
    // write() to complete is not a guarantee that we've read the incoming
    // request.
    await Promise.all([p1, p2]);
    conn.close();
    listener.close();
  });
  return buf;
}

testPerm({ net: true }, async function fetchRequest():Promise<void> {
  const addr = "127.0.0.1:4501";
  const buf = bufferServer(addr);
  const response = await fetch(`http://${addr}/blah`, {
    method: "POST",
    headers: [["Hello", "World"], ["Foo", "Bar"]]
  });
  assertEquals(response.status, 404);
  assertEquals(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n\r\n`
  ].join("");
  assertEquals(actual, expected);
});

testPerm({ net: true }, async function fetchPostBodyString():Promise<void> {
  const addr = "127.0.0.1:4502";
  const buf = bufferServer(addr);
  const body = "hello world";
  const response = await fetch(`http://${addr}/blah`, {
    method: "POST",
    headers: [["Hello", "World"], ["Foo", "Bar"]],
    body
  });
  assertEquals(response.status, 404);
  assertEquals(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n`,
    `content-length: ${body.length}\r\n\r\n`,
    body
  ].join("");
  assertEquals(actual, expected);
});

testPerm({ net: true }, async function fetchPostBodyTypedArray():Promise<void> {
  const addr = "127.0.0.1:4503";
  const buf = bufferServer(addr);
  const bodyStr = "hello world";
  const body = new TextEncoder().encode(bodyStr);
  const response = await fetch(`http://${addr}/blah`, {
    method: "POST",
    headers: [["Hello", "World"], ["Foo", "Bar"]],
    body
  });
  assertEquals(response.status, 404);
  assertEquals(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n`,
    `content-length: ${body.byteLength}\r\n\r\n`,
    bodyStr
  ].join("");
  assertEquals(actual, expected);
});
*/
