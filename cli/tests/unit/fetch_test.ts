// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrowsAsync,
  deferred,
  fail,
  unimplemented,
  unitTest,
} from "./test_util.ts";
import { Buffer } from "../../../test_util/std/io/buffer.ts";

unitTest(
  { perms: { net: true } },
  async function fetchRequiresOneArgument() {
    await assertThrowsAsync(
      fetch as unknown as () => Promise<void>,
      TypeError,
    );
  },
);

unitTest({ perms: { net: true } }, async function fetchProtocolError() {
  await assertThrowsAsync(
    async () => {
      await fetch("file:///");
    },
    TypeError,
    "not supported",
  );
});

function findClosedPortInRange(
  minPort: number,
  maxPort: number,
): number | never {
  let port = minPort;

  // If we hit the return statement of this loop
  // that means that we did not throw an
  // AddrInUse error when we executed Deno.listen.
  while (port < maxPort) {
    try {
      const listener = Deno.listen({ port });
      listener.close();
      return port;
    } catch (_e) {
      port++;
    }
  }

  unimplemented(
    `No available ports between ${minPort} and ${maxPort} to test fetch`,
  );
}

unitTest(
  { perms: { net: true } },
  async function fetchConnectionError() {
    const port = findClosedPortInRange(4000, 9999);
    await assertThrowsAsync(
      async () => {
        await fetch(`http://localhost:${port}`);
      },
      TypeError,
      "error trying to connect",
    );
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchDnsError() {
    await assertThrowsAsync(
      async () => {
        await fetch("http://nil/");
      },
      TypeError,
      "error trying to connect",
    );
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInvalidUriError() {
    await assertThrowsAsync(
      async () => {
        await fetch("http://<invalid>/");
      },
      TypeError,
    );
  },
);

unitTest({ perms: { net: true } }, async function fetchJsonSuccess() {
  const response = await fetch("http://localhost:4545/fixture.json");
  const json = await response.json();
  assertEquals(json.name, "deno");
});

unitTest(async function fetchPerm() {
  await assertThrowsAsync(async () => {
    await fetch("http://localhost:4545/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { net: true } }, async function fetchUrl() {
  const response = await fetch("http://localhost:4545/fixture.json");
  assertEquals(response.url, "http://localhost:4545/fixture.json");
  const _json = await response.json();
});

unitTest({ perms: { net: true } }, async function fetchURL() {
  const response = await fetch(
    new URL("http://localhost:4545/fixture.json"),
  );
  assertEquals(response.url, "http://localhost:4545/fixture.json");
  const _json = await response.json();
});

unitTest({ perms: { net: true } }, async function fetchHeaders() {
  const response = await fetch("http://localhost:4545/fixture.json");
  const headers = response.headers;
  assertEquals(headers.get("Content-Type"), "application/json");
  const _json = await response.json();
});

unitTest({ perms: { net: true } }, async function fetchBlob() {
  const response = await fetch("http://localhost:4545/fixture.json");
  const headers = response.headers;
  const blob = await response.blob();
  assertEquals(blob.type, headers.get("Content-Type"));
  assertEquals(blob.size, Number(headers.get("Content-Length")));
});

unitTest(
  { perms: { net: true } },
  async function fetchBodyUsedReader() {
    const response = await fetch(
      "http://localhost:4545/fixture.json",
    );
    assert(response.body !== null);

    const reader = response.body.getReader();
    // Getting a reader should lock the stream but does not consume the body
    // so bodyUsed should not be true
    assertEquals(response.bodyUsed, false);
    reader.releaseLock();
    await response.json();
    assertEquals(response.bodyUsed, true);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchBodyUsedCancelStream() {
    const response = await fetch(
      "http://localhost:4545/fixture.json",
    );
    assert(response.body !== null);

    assertEquals(response.bodyUsed, false);
    const promise = response.body.cancel();
    assertEquals(response.bodyUsed, true);
    await promise;
  },
);

unitTest({ perms: { net: true } }, async function fetchAsyncIterator() {
  const response = await fetch("http://localhost:4545/fixture.json");
  const headers = response.headers;

  assert(response.body !== null);
  let total = 0;
  for await (const chunk of response.body) {
    assert(chunk instanceof Uint8Array);
    total += chunk.length;
  }

  assertEquals(total, Number(headers.get("Content-Length")));
});

unitTest({ perms: { net: true } }, async function fetchBodyReader() {
  const response = await fetch("http://localhost:4545/fixture.json");
  const headers = response.headers;
  assert(response.body !== null);
  const reader = response.body.getReader();
  let total = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    assert(value);
    assert(value instanceof Uint8Array);
    total += value.length;
  }

  assertEquals(total, Number(headers.get("Content-Length")));
});

unitTest(
  { perms: { net: true } },
  async function fetchBodyReaderBigBody() {
    const data = "a".repeat(10 << 10); // 10mb
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: data,
    });
    assert(response.body !== null);
    const reader = await response.body.getReader();
    let total = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      assert(value);
      total += value.length;
    }

    assertEquals(total, data.length);
  },
);

unitTest({ perms: { net: true } }, async function responseClone() {
  const response = await fetch("http://localhost:4545/fixture.json");
  const response1 = response.clone();
  assert(response !== response1);
  assertEquals(response.status, response1.status);
  assertEquals(response.statusText, response1.statusText);
  const u8a = new Uint8Array(await response.arrayBuffer());
  const u8a1 = new Uint8Array(await response1.arrayBuffer());
  for (let i = 0; i < u8a.byteLength; i++) {
    assertEquals(u8a[i], u8a1[i]);
  }
});

unitTest(
  { perms: { net: true } },
  async function fetchMultipartFormDataSuccess() {
    const response = await fetch(
      "http://localhost:4545/multipart_form_data.txt",
    );
    const formData = await response.formData();
    assert(formData.has("field_1"));
    assertEquals(formData.get("field_1")!.toString(), "value_1 \r\n");
    assert(formData.has("field_2"));
    const file = formData.get("field_2") as File;
    assertEquals(file.name, "file.js");

    assertEquals(await file.text(), `console.log("Hi")`);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchMultipartFormBadContentType() {
    const response = await fetch(
      "http://localhost:4545/multipart_form_bad_content_type",
    );
    assert(response.body !== null);

    await assertThrowsAsync(
      async () => {
        await response.formData();
      },
      TypeError,
      "Body can not be decoded as form data",
    );
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchURLEncodedFormDataSuccess() {
    const response = await fetch(
      "http://localhost:4545/subdir/form_urlencoded.txt",
    );
    const formData = await response.formData();
    assert(formData.has("field_1"));
    assertEquals(formData.get("field_1")!.toString(), "Hi");
    assert(formData.has("field_2"));
    assertEquals(formData.get("field_2")!.toString(), "<Deno>");
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitFormDataBinaryFileBody() {
    // Some random bytes
    // deno-fmt-ignore
    const binaryFile = new Uint8Array([108,2,0,0,145,22,162,61,157,227,166,77,138,75,180,56,119,188,177,183]);
    const response = await fetch("http://localhost:4545/echo_multipart_file", {
      method: "POST",
      body: binaryFile,
    });
    const resultForm = await response.formData();
    const resultFile = resultForm.get("file") as File;

    assertEquals(resultFile.type, "application/octet-stream");
    assertEquals(resultFile.name, "file.bin");
    assertEquals(new Uint8Array(await resultFile.arrayBuffer()), binaryFile);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitFormDataMultipleFilesBody() {
    const files = [
      {
        // deno-fmt-ignore
        content: new Uint8Array([137,80,78,71,13,10,26,10, 137, 1, 25]),
        type: "image/png",
        name: "image",
        fileName: "some-image.png",
      },
      {
        // deno-fmt-ignore
        content: new Uint8Array([108,2,0,0,145,22,162,61,157,227,166,77,138,75,180,56,119,188,177,183]),
        name: "file",
        fileName: "file.bin",
        expectedType: "application/octet-stream",
      },
      {
        content: new TextEncoder().encode("deno land"),
        type: "text/plain",
        name: "text",
        fileName: "deno.txt",
      },
    ];
    const form = new FormData();
    form.append("field", "value");
    for (const file of files) {
      form.append(
        file.name,
        new Blob([file.content], { type: file.type }),
        file.fileName,
      );
    }
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: form,
    });
    const resultForm = await response.formData();
    assertEquals(form.get("field"), resultForm.get("field"));
    for (const file of files) {
      const inputFile = form.get(file.name) as File;
      const resultFile = resultForm.get(file.name) as File;
      assertEquals(inputFile.size, resultFile.size);
      assertEquals(inputFile.name, resultFile.name);
      assertEquals(file.expectedType || file.type, resultFile.type);
      assertEquals(
        new Uint8Array(await resultFile.arrayBuffer()),
        file.content,
      );
    }
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithRedirection() {
    const response = await fetch("http://localhost:4546/hello.txt");
    assertEquals(response.status, 200);
    assertEquals(response.statusText, "OK");
    assertEquals(response.url, "http://localhost:4545/hello.txt");
    const body = await response.text();
    assert(body.includes("Hello world!"));
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithRelativeRedirection() {
    const response = await fetch(
      "http://localhost:4545/001_hello.js",
    );
    assertEquals(response.status, 200);
    assertEquals(response.statusText, "OK");
    const body = await response.text();
    assert(body.includes("Hello"));
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithRelativeRedirectionUrl() {
    const cases = [
      ["end", "http://localhost:4550/a/b/end"],
      ["/end", "http://localhost:4550/end"],
    ];
    for (const [loc, redUrl] of cases) {
      const response = await fetch("http://localhost:4550/a/b/c", {
        headers: new Headers([["x-location", loc]]),
      });
      assertEquals(response.url, redUrl);
      assertEquals(response.redirected, true);
      assertEquals(response.status, 404);
      assertEquals(await response.text(), "");
    }
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithInfRedirection() {
    await assertThrowsAsync(
      () => fetch("http://localhost:4549"),
      TypeError,
      "redirect",
    );
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitStringBody() {
    const data = "Hello World";
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: data,
    });
    const text = await response.text();
    assertEquals(text, data);
    assert(response.headers.get("content-type")!.startsWith("text/plain"));
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchRequestInitStringBody() {
    const data = "Hello World";
    const req = new Request("http://localhost:4545/echo_server", {
      method: "POST",
      body: data,
    });
    const response = await fetch(req);
    const text = await response.text();
    assertEquals(text, data);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchSeparateInit() {
    // related to: https://github.com/denoland/deno/issues/10396
    const req = new Request("http://localhost:4545/001_hello.js");
    const init = {
      method: "GET",
    };
    req.headers.set("foo", "bar");
    const res = await fetch(req, init);
    assertEquals(res.status, 200);
    await res.text();
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitTypedArrayBody() {
    const data = "Hello World";
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: new TextEncoder().encode(data),
    });
    const text = await response.text();
    assertEquals(text, data);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitArrayBufferBody() {
    const data = "Hello World";
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: new TextEncoder().encode(data).buffer,
    });
    const text = await response.text();
    assertEquals(text, data);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitURLSearchParamsBody() {
    const data = "param1=value1&param2=value2";
    const params = new URLSearchParams(data);
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: params,
    });
    const text = await response.text();
    assertEquals(text, data);
    assert(
      response.headers
        .get("content-type")!
        .startsWith("application/x-www-form-urlencoded"),
    );
  },
);

unitTest({ perms: { net: true } }, async function fetchInitBlobBody() {
  const data = "const a = 1";
  const blob = new Blob([data], {
    type: "text/javascript",
  });
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: blob,
  });
  const text = await response.text();
  assertEquals(text, data);
  assert(response.headers.get("content-type")!.startsWith("text/javascript"));
});

unitTest(
  { perms: { net: true } },
  async function fetchInitFormDataBody() {
    const form = new FormData();
    form.append("field", "value");
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: form,
    });
    const resultForm = await response.formData();
    assertEquals(form.get("field"), resultForm.get("field"));
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitFormDataBlobFilenameBody() {
    const form = new FormData();
    form.append("field", "value");
    form.append("file", new Blob([new TextEncoder().encode("deno")]));
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: form,
    });
    const resultForm = await response.formData();
    assertEquals(form.get("field"), resultForm.get("field"));
    const file = resultForm.get("file");
    assert(file instanceof File);
    assertEquals(file.name, "blob");
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchInitFormDataTextFileBody() {
    const fileContent = "deno land";
    const form = new FormData();
    form.append("field", "value");
    form.append(
      "file",
      new Blob([new TextEncoder().encode(fileContent)], {
        type: "text/plain",
      }),
      "deno.txt",
    );
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: form,
    });
    const resultForm = await response.formData();
    assertEquals(form.get("field"), resultForm.get("field"));

    const file = form.get("file") as File;
    const resultFile = resultForm.get("file") as File;

    assertEquals(file.size, resultFile.size);
    assertEquals(file.name, resultFile.name);
    assertEquals(file.type, resultFile.type);
    assertEquals(await file.text(), await resultFile.text());
  },
);

unitTest({ perms: { net: true } }, async function fetchUserAgent() {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: new TextEncoder().encode(data),
  });
  assertEquals(response.headers.get("user-agent"), `Deno/${Deno.version.deno}`);
  await response.text();
});

// TODO(ry) The following tests work but are flaky. There's a race condition
// somewhere. Here is what one of these flaky failures looks like:
//
// unitTest fetchPostBodyString_permW0N1E0R0
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

function bufferServer(addr: string): Buffer {
  const [hostname, port] = addr.split(":");
  const listener = Deno.listen({
    hostname,
    port: Number(port),
  }) as Deno.Listener;
  const buf = new Buffer();
  listener.accept().then(async (conn: Deno.Conn) => {
    const p1 = buf.readFrom(conn);
    const p2 = conn.write(
      new TextEncoder().encode(
        "HTTP/1.0 404 Not Found\r\nContent-Length: 2\r\n\r\nNF",
      ),
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

unitTest(
  {
    perms: { net: true },
  },
  async function fetchRequest() {
    const addr = "127.0.0.1:4501";
    const buf = bufferServer(addr);
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Hello", "World"],
        ["Foo", "Bar"],
      ],
    });
    await response.arrayBuffer();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode(buf.bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "hello: World\r\n",
      "foo: Bar\r\n",
      "accept: */*\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip, br\r\n",
      `host: ${addr}\r\n\r\n`,
    ].join("");
    assertEquals(actual, expected);
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchPostBodyString() {
    const addr = "127.0.0.1:4502";
    const buf = bufferServer(addr);
    const body = "hello world";
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Hello", "World"],
        ["Foo", "Bar"],
      ],
      body,
    });
    await response.arrayBuffer();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode(buf.bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "hello: World\r\n",
      "foo: Bar\r\n",
      "content-type: text/plain;charset=UTF-8\r\n",
      "accept: */*\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip, br\r\n",
      `host: ${addr}\r\n`,
      `content-length: ${body.length}\r\n\r\n`,
      body,
    ].join("");
    assertEquals(actual, expected);
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchPostBodyTypedArray() {
    const addr = "127.0.0.1:4503";
    const buf = bufferServer(addr);
    const bodyStr = "hello world";
    const body = new TextEncoder().encode(bodyStr);
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Hello", "World"],
        ["Foo", "Bar"],
      ],
      body,
    });
    await response.arrayBuffer();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode(buf.bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "hello: World\r\n",
      "foo: Bar\r\n",
      "accept: */*\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip, br\r\n",
      `host: ${addr}\r\n`,
      `content-length: ${body.byteLength}\r\n\r\n`,
      bodyStr,
    ].join("");
    assertEquals(actual, expected);
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithNonAsciiRedirection() {
    const response = await fetch("http://localhost:4545/non_ascii_redirect", {
      redirect: "manual",
    });
    assertEquals(response.status, 301);
    assertEquals(response.headers.get("location"), "/redirect®");
    await response.text();
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithManualRedirection() {
    const response = await fetch("http://localhost:4546/", {
      redirect: "manual",
    }); // will redirect to http://localhost:4545/
    assertEquals(response.status, 301);
    assertEquals(response.url, "http://localhost:4546/");
    assertEquals(response.type, "basic");
    assertEquals(response.headers.get("Location"), "http://localhost:4545/");
    await response.body!.cancel();
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchWithErrorRedirection() {
    await assertThrowsAsync(
      () =>
        fetch("http://localhost:4546/", {
          redirect: "error",
        }),
      TypeError,
      "redirect",
    );
  },
);

unitTest(function responseRedirect() {
  const redir = Response.redirect("example.com/newLocation", 301);
  assertEquals(redir.status, 301);
  assertEquals(redir.statusText, "");
  assertEquals(redir.url, "");
  assertEquals(
    redir.headers.get("Location"),
    "http://js-unit-tests/foo/example.com/newLocation",
  );
  assertEquals(redir.type, "default");
});

unitTest(async function responseWithoutBody() {
  const response = new Response();
  assertEquals(await response.arrayBuffer(), new ArrayBuffer(0));
  const blob = await response.blob();
  assertEquals(blob.size, 0);
  assertEquals(await blob.arrayBuffer(), new ArrayBuffer(0));
  assertEquals(await response.text(), "");
  await assertThrowsAsync(async () => {
    await response.json();
  });
});

unitTest({ perms: { net: true } }, async function fetchBodyReadTwice() {
  const response = await fetch("http://localhost:4545/fixture.json");

  // Read body
  const _json = await response.json();
  assert(_json);

  // All calls after the body was consumed, should fail
  const methods = ["json", "text", "formData", "arrayBuffer"] as const;
  for (const method of methods) {
    try {
      await response[method]();
      fail(
        "Reading body multiple times should failed, the stream should've been locked.",
      );
    } catch {
      // pass
    }
  }
});

unitTest(
  { perms: { net: true } },
  async function fetchBodyReaderAfterRead() {
    const response = await fetch(
      "http://localhost:4545/fixture.json",
    );
    assert(response.body !== null);
    const reader = await response.body.getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      assert(value);
    }

    try {
      response.body.getReader();
      fail("The stream should've been locked.");
    } catch {
      // pass
    }
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchBodyReaderWithCancelAndNewReader() {
    const data = "a".repeat(1 << 10);
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: data,
    });
    assert(response.body !== null);
    const firstReader = await response.body.getReader();

    // Acquire reader without reading & release
    await firstReader.releaseLock();

    const reader = await response.body.getReader();

    let total = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      assert(value);
      total += value.length;
    }

    assertEquals(total, data.length);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchBodyReaderWithReadCancelAndNewReader() {
    const data = "a".repeat(1 << 10);

    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: data,
    });
    assert(response.body !== null);
    const firstReader = await response.body.getReader();

    // Do one single read with first reader
    const { value: firstValue } = await firstReader.read();
    assert(firstValue);
    await firstReader.releaseLock();

    // Continue read with second reader
    const reader = await response.body.getReader();
    let total = firstValue.length || 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      assert(value);
      total += value.length;
    }
    assertEquals(total, data.length);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchResourceCloseAfterStreamCancel() {
    const res = await fetch("http://localhost:4545/fixture.json");
    assert(res.body !== null);

    // After ReadableStream.cancel is called, resource handle must be closed
    // The test should not fail with: Test case is leaking resources
    await res.body.cancel();
  },
);

// FIXME(bartlomieju): for reasons unknown after working for
// a few months without a problem; this test started failing
// consistently on Windows CI with following error:
// TypeError: error sending request for url (http://localhost:4545/echo_server):
// connection error: An established connection was aborted by
// the software in your host machine. (os error 10053)
unitTest(
  { perms: { net: true }, ignore: Deno.build.os == "windows" },
  async function fetchNullBodyStatus() {
    const nullBodyStatus = [101, 204, 205, 304];

    for (const status of nullBodyStatus) {
      const headers = new Headers([["x-status", String(status)]]);
      const res = await fetch("http://localhost:4545/echo_server", {
        body: "deno",
        method: "POST",
        headers,
      });
      assertEquals(res.body, null);
      assertEquals(res.status, status);
    }
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchResponseContentLength() {
    const body = new Uint8Array(2 ** 16);
    const headers = new Headers([["content-type", "application/octet-stream"]]);
    const res = await fetch("http://localhost:4545/echo_server", {
      body: body,
      method: "POST",
      headers,
    });
    assertEquals(Number(res.headers.get("content-length")), body.byteLength);

    const blob = await res.blob();
    // Make sure Body content-type is correctly set
    assertEquals(blob.type, "application/octet-stream");
    assertEquals(blob.size, body.byteLength);
  },
);

unitTest(function fetchResponseConstructorNullBody() {
  const nullBodyStatus = [204, 205, 304];

  for (const status of nullBodyStatus) {
    try {
      new Response("deno", { status });
      fail("Response with null body status cannot have body");
    } catch (e) {
      assert(e instanceof TypeError);
      assertEquals(
        e.message,
        "Response with null body status cannot have body",
      );
    }
  }
});

unitTest(function fetchResponseConstructorInvalidStatus() {
  const invalidStatus = [101, 600, 199, null, "", NaN];

  for (const status of invalidStatus) {
    try {
      // deno-lint-ignore ban-ts-comment
      // @ts-ignore
      new Response("deno", { status });
      fail(`Invalid status: ${status}`);
    } catch (e) {
      assert(e instanceof RangeError);
      assert(e.message.endsWith("is outside the range [200, 599]."));
    }
  }
});

unitTest(function fetchResponseEmptyConstructor() {
  const response = new Response();
  assertEquals(response.status, 200);
  assertEquals(response.body, null);
  assertEquals(response.type, "default");
  assertEquals(response.url, "");
  assertEquals(response.redirected, false);
  assertEquals(response.ok, true);
  assertEquals(response.bodyUsed, false);
  assertEquals([...response.headers], []);
});

// TODO(lucacasonato): reenable this test
unitTest(
  { perms: { net: true }, ignore: true },
  async function fetchCustomHttpClientParamCertificateSuccess(): Promise<
    void
  > {
    const client = Deno.createHttpClient(
      {
        caData: `-----BEGIN CERTIFICATE-----
MIIDIzCCAgugAwIBAgIJAMKPPW4tsOymMA0GCSqGSIb3DQEBCwUAMCcxCzAJBgNV
BAYTAlVTMRgwFgYDVQQDDA9FeGFtcGxlLVJvb3QtQ0EwIBcNMTkxMDIxMTYyODIy
WhgPMjExODA5MjcxNjI4MjJaMCcxCzAJBgNVBAYTAlVTMRgwFgYDVQQDDA9FeGFt
cGxlLVJvb3QtQ0EwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDMH/IO
2qtHfyBKwANNPB4K0q5JVSg8XxZdRpTTlz0CwU0oRO3uHrI52raCCfVeiQutyZop
eFZTDWeXGudGAFA2B5m3orWt0s+touPi8MzjsG2TQ+WSI66QgbXTNDitDDBtTVcV
5G3Ic+3SppQAYiHSekLISnYWgXLl+k5CnEfTowg6cjqjVr0KjL03cTN3H7b+6+0S
ws4rYbW1j4ExR7K6BFNH6572yq5qR20E6GqlY+EcOZpw4CbCk9lS8/CWuXze/vMs
OfDcc6K+B625d27wyEGZHedBomT2vAD7sBjvO8hn/DP1Qb46a8uCHR6NSfnJ7bXO
G1igaIbgY1zXirNdAgMBAAGjUDBOMB0GA1UdDgQWBBTzut+pwwDfqmMYcI9KNWRD
hxcIpTAfBgNVHSMEGDAWgBTzut+pwwDfqmMYcI9KNWRDhxcIpTAMBgNVHRMEBTAD
AQH/MA0GCSqGSIb3DQEBCwUAA4IBAQB9AqSbZ+hEglAgSHxAMCqRFdhVu7MvaQM0
P090mhGlOCt3yB7kdGfsIrUW6nQcTz7PPQFRaJMrFHPvFvPootkBUpTYR4hTkdce
H6RCRu2Jxl4Y9bY/uezd9YhGCYfUtfjA6/TH9FcuZfttmOOlxOt01XfNvVMIR6RM
z/AYhd+DeOXjr35F/VHeVpnk+55L0PYJsm1CdEbOs5Hy1ecR7ACuDkXnbM4fpz9I
kyIWJwk2zJReKcJMgi1aIinDM9ao/dca1G99PHOw8dnr4oyoTiv8ao6PWiSRHHMi
MNf4EgWfK+tZMnuqfpfO9740KzfcVoMNo4QJD4yn5YxroUOO/Azi
-----END CERTIFICATE-----
`,
      },
    );
    const response = await fetch(
      "https://localhost:5545/fixture.json",
      { client },
    );
    const json = await response.json();
    assertEquals(json.name, "deno");
    client.close();
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchCustomClientUserAgent(): Promise<
    void
  > {
    const data = "Hello World";
    const client = Deno.createHttpClient({});
    const response = await fetch("http://localhost:4545/echo_server", {
      client,
      method: "POST",
      body: new TextEncoder().encode(data),
    });
    assertEquals(
      response.headers.get("user-agent"),
      `Deno/${Deno.version.deno}`,
    );
    await response.text();
    client.close();
  },
);

unitTest(
  {
    perms: { net: true },
  },
  async function fetchPostBodyReadableStream() {
    const addr = "127.0.0.1:4502";
    const buf = bufferServer(addr);
    const stream = new TransformStream();
    const writer = stream.writable.getWriter();
    // transformer writes don't resolve until they are read, so awaiting these
    // will cause the transformer to hang, as the suspend the transformer, it
    // is also illogical to await for the reads, as that is the whole point of
    // streams is to have a "queue" which gets drained...
    writer.write(new TextEncoder().encode("hello "));
    writer.write(new TextEncoder().encode("world"));
    writer.close();
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Hello", "World"],
        ["Foo", "Bar"],
      ],
      body: stream.readable,
    });
    await response.arrayBuffer();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode(buf.bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "hello: World\r\n",
      "foo: Bar\r\n",
      "accept: */*\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip, br\r\n",
      `host: ${addr}\r\n`,
      `transfer-encoding: chunked\r\n\r\n`,
      "6\r\n",
      "hello \r\n",
      "5\r\n",
      "world\r\n",
      "0\r\n\r\n",
    ].join("");
    assertEquals(actual, expected);
  },
);

unitTest({}, function fetchWritableRespProps() {
  const original = new Response("https://deno.land", {
    status: 404,
    headers: { "x-deno": "foo" },
  });
  const new_ = new Response("https://deno.land", original);
  assertEquals(original.status, new_.status);
  assertEquals(new_.headers.get("x-deno"), "foo");
});

function returnHostHeaderServer(addr: string): Deno.Listener {
  const [hostname, port] = addr.split(":");
  const listener = Deno.listen({
    hostname,
    port: Number(port),
  }) as Deno.Listener;

  listener.accept().then(async (conn: Deno.Conn) => {
    const httpConn = Deno.serveHttp(conn);

    await httpConn.nextRequest()
      .then(async (requestEvent: Deno.RequestEvent | null) => {
        const hostHeader = requestEvent?.request.headers.get("Host");
        const headersToReturn = hostHeader ? { "Host": hostHeader } : undefined;

        await requestEvent?.respondWith(
          new Response("", {
            status: 200,
            headers: headersToReturn,
          }),
        );
      });

    httpConn.close();
  });

  return listener;
}

unitTest(
  { perms: { net: true } },
  async function fetchFilterOutCustomHostHeader(): Promise<
    void
  > {
    const addr = "127.0.0.1:4502";
    const listener = returnHostHeaderServer(addr);
    const response = await fetch(`http://${addr}/`, {
      headers: { "Host": "example.com" },
    });
    await response.text();
    listener.close();

    assertEquals(response.headers.get("Host"), addr);
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchNoServerReadableStreamBody() {
    const done = deferred();
    const body = new ReadableStream({
      start(controller) {
        controller.enqueue(new Uint8Array([1]));
        setTimeout(() => {
          controller.enqueue(new Uint8Array([2]));
          done.resolve();
        }, 1000);
      },
    });
    const nonExistantHostname = "http://localhost:47582";
    await assertThrowsAsync(async () => {
      await fetch(nonExistantHostname, { body, method: "POST" });
    }, TypeError);
    await done;
  },
);

unitTest(
  { perms: { net: true } },
  async function fetchHeadRespBody() {
    const res = await fetch("http://localhost:4545/echo_server", {
      method: "HEAD",
    });
    assertEquals(res.body, null);
  },
);
