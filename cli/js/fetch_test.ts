// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert } from "./test_util.ts";

testPerm({ net: true }, async function fetchProtocolError(): Promise<void> {
  let err;
  try {
    await fetch("file:///");
  } catch (err_) {
    err = err_;
  }
  assert(err instanceof TypeError);
  assert.strContains(err.message, "not supported");
});

testPerm({ net: true }, async function fetchConnectionError(): Promise<void> {
  let err;
  try {
    await fetch("http://localhost:4000");
  } catch (err_) {
    err = err_;
  }
  assert(err instanceof Deno.errors.Http);
  assert.strContains(err.message, "error trying to connect");
});

testPerm({ net: true }, async function fetchJsonSuccess(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const json = await response.json();
  assert.equals(json.name, "deno");
});

test(async function fetchPerm(): Promise<void> {
  let err;
  try {
    await fetch("http://localhost:4545/cli/tests/fixture.json");
  } catch (err_) {
    err = err_;
  }
  assert(err instanceof Deno.errors.PermissionDenied);
  assert.equals(err.name, "PermissionDenied");
});

testPerm({ net: true }, async function fetchUrl(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  assert.equals(response.url, "http://localhost:4545/cli/tests/fixture.json");
});

testPerm({ net: true }, async function fetchURL(): Promise<void> {
  const response = await fetch(
    new URL("http://localhost:4545/cli/tests/fixture.json")
  );
  assert.equals(response.url, "http://localhost:4545/cli/tests/fixture.json");
});

testPerm({ net: true }, async function fetchHeaders(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const headers = response.headers;
  assert.equals(headers.get("Content-Type"), "application/json");
  assert(headers.get("Server")!.startsWith("SimpleHTTP"));
});

testPerm({ net: true }, async function fetchBlob(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const headers = response.headers;
  const blob = await response.blob();
  assert.equals(blob.type, headers.get("Content-Type"));
  assert.equals(blob.size, Number(headers.get("Content-Length")));
});

testPerm({ net: true }, async function fetchBodyUsed(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  assert.equals(response.bodyUsed, false);
  assert.throws((): void => {
    // Assigning to read-only property throws in the strict mode.
    response.bodyUsed = true;
  });
  await response.blob();
  assert.equals(response.bodyUsed, true);
});

testPerm({ net: true }, async function fetchAsyncIterator(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const headers = response.headers;
  let total = 0;
  for await (const chunk of response.body) {
    total += chunk.length;
  }

  assert.equals(total, Number(headers.get("Content-Length")));
});

testPerm({ net: true }, async function responseClone(): Promise<void> {
  const response = await fetch("http://localhost:4545/cli/tests/fixture.json");
  const response1 = response.clone();
  assert(response !== response1);
  assert.equals(response.status, response1.status);
  assert.equals(response.statusText, response1.statusText);
  const u8a = new Uint8Array(await response.arrayBuffer());
  const u8a1 = new Uint8Array(await response1.arrayBuffer());
  for (let i = 0; i < u8a.byteLength; i++) {
    assert.equals(u8a[i], u8a1[i]);
  }
});

testPerm({ net: true }, async function fetchEmptyInvalid(): Promise<void> {
  let err;
  try {
    await fetch("");
  } catch (err_) {
    err = err_;
  }
  assert(err instanceof URIError);
});

testPerm({ net: true }, async function fetchMultipartFormDataSuccess(): Promise<
  void
> {
  const response = await fetch(
    "http://localhost:4545/cli/tests/subdir/multipart_form_data.txt"
  );
  const formData = await response.formData();
  assert(formData.has("field_1"));
  assert.equals(formData.get("field_1")!.toString(), "value_1 \r\n");
  assert(formData.has("field_2"));
  /* TODO(ry) Re-enable this test once we bring back the global File type.
  const file = formData.get("field_2") as File;
  assert.equals(file.name, "file.js");
  */
  // Currently we cannot read from file...
});

testPerm(
  { net: true },
  async function fetchURLEncodedFormDataSuccess(): Promise<void> {
    const response = await fetch(
      "http://localhost:4545/cli/tests/subdir/form_urlencoded.txt"
    );
    const formData = await response.formData();
    assert(formData.has("field_1"));
    assert.equals(formData.get("field_1")!.toString(), "Hi");
    assert(formData.has("field_2"));
    assert.equals(formData.get("field_2")!.toString(), "<Deno>");
  }
);

testPerm({ net: true }, async function fetchWithRedirection(): Promise<void> {
  const response = await fetch("http://localhost:4546/"); // will redirect to http://localhost:4545/
  assert.equals(response.status, 200);
  assert.equals(response.statusText, "OK");
  assert.equals(response.url, "http://localhost:4545/");
  const body = await response.text();
  assert(body.includes("<title>Directory listing for /</title>"));
});

testPerm({ net: true }, async function fetchWithRelativeRedirection(): Promise<
  void
> {
  const response = await fetch("http://localhost:4545/cli/tests"); // will redirect to /cli/tests/
  assert.equals(response.status, 200);
  assert.equals(response.statusText, "OK");
  const body = await response.text();
  assert(body.includes("<title>Directory listing for /cli/tests/</title>"));
});

// The feature below is not implemented, but the test should work after implementation
/*
testPerm({ net: true }, async function fetchWithInfRedirection(): Promise<
  void
> {
  const response = await fetch("http://localhost:4549/cli/tests"); // will redirect to the same place
  assert.equals(response.status, 0); // network error
});
*/

testPerm({ net: true }, async function fetchInitStringBody(): Promise<void> {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: data
  });
  const text = await response.text();
  assert.equals(text, data);
  assert(response.headers.get("content-type")!.startsWith("text/plain"));
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
  assert.equals(text, data);
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
  assert.equals(text, data);
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
  assert.equals(text, data);
  assert(
    response.headers
      .get("content-type")!
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
  assert.equals(text, data);
  assert(response.headers.get("content-type")!.startsWith("text/javascript"));
});

testPerm({ net: true }, async function fetchUserAgent(): Promise<void> {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: new TextEncoder().encode(data)
  });
  assert.equals(
    response.headers.get("user-agent"),
    `Deno/${Deno.version.deno}`
  );
  await response.text();
});

// TODO(ry) The following tests work but are flaky. There's a race condition
// somewhere. Here is what one of these flaky failures looks like:
//
// test fetchPostBodyString_permW0N1E0R0
// assert.equals failed. actual =   expected = POST /blah HTTP/1.1
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
//     at Object.assert.equals (file:///C:/deno/js/testing/util.ts:29:11)
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
  assert.equals(response.status, 404);
  assert.equals(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n\r\n`
  ].join("");
  assert.equals(actual, expected);
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
  assert.equals(response.status, 404);
  assert.equals(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n`,
    `content-length: ${body.length}\r\n\r\n`,
    body
  ].join("");
  assert.equals(actual, expected);
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
  assert.equals(response.status, 404);
  assert.equals(response.headers.get("Content-Length"), "2");

  const actual = new TextDecoder().decode(buf.bytes());
  const expected = [
    "POST /blah HTTP/1.1\r\n",
    "hello: World\r\n",
    "foo: Bar\r\n",
    `host: ${addr}\r\n`,
    `content-length: ${body.byteLength}\r\n\r\n`,
    bodyStr
  ].join("");
  assert.equals(actual, expected);
});
*/

testPerm({ net: true }, async function fetchWithManualRedirection(): Promise<
  void
> {
  const response = await fetch("http://localhost:4546/", {
    redirect: "manual"
  }); // will redirect to http://localhost:4545/
  assert.equals(response.status, 0);
  assert.equals(response.statusText, "");
  assert.equals(response.url, "");
  assert.equals(response.type, "opaqueredirect");
  try {
    await response.text();
    assert.unreachable(
      "Reponse.text() didn't throw on a filtered response without a body (type opaqueredirect)"
    );
  } catch (e) {
    return;
  }
});

testPerm({ net: true }, async function fetchWithErrorRedirection(): Promise<
  void
> {
  const response = await fetch("http://localhost:4546/", {
    redirect: "error"
  }); // will redirect to http://localhost:4545/
  assert.equals(response.status, 0);
  assert.equals(response.statusText, "");
  assert.equals(response.url, "");
  assert.equals(response.type, "error");
  try {
    await response.text();
    assert.unreachable(
      "Reponse.text() didn't throw on a filtered response without a body (type error)"
    );
  } catch (e) {
    return;
  }
});

test(function responseRedirect(): void {
  const response = new Response(
    "example.com/beforeredirect",
    200,
    "OK",
    [["This-Should", "Disappear"]],
    -1,
    false,
    null
  );
  const redir = response.redirect("example.com/newLocation", 301);
  assert.equals(redir.status, 301);
  assert.equals(redir.statusText, "");
  assert.equals(redir.url, "");
  assert.equals(redir.headers.get("Location"), "example.com/newLocation");
  assert.equals(redir.type, "default");
});

test(function responseConstructionHeaderRemoval(): void {
  const res = new Response(
    "example.com",
    200,
    "OK",
    [["Set-Cookie", "mysessionid"]],
    -1,
    false,
    "basic",
    null
  );
  assert(res.headers.get("Set-Cookie") != "mysessionid");
});
