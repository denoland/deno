// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import {
  assert,
  assertEquals,
  assertRejects,
  assertStringIncludes,
  assertThrows,
  delay,
  fail,
  unimplemented,
} from "./test_util.ts";
import { Buffer } from "@std/io/buffer";

const listenPort = 4506;

Deno.test(
  { permissions: { net: true } },
  async function fetchRequiresOneArgument() {
    await assertRejects(
      fetch as unknown as () => Promise<void>,
      TypeError,
    );
  },
);

Deno.test({ permissions: { net: true } }, async function fetchProtocolError() {
  await assertRejects(
    async () => {
      await fetch("ftp://localhost:21/a/file");
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

Deno.test(
  // TODO(bartlomieju): reenable this test
  // https://github.com/denoland/deno/issues/18350
  { ignore: Deno.build.os === "windows", permissions: { net: true } },
  async function fetchConnectionError() {
    const port = findClosedPortInRange(4000, 9999);
    await assertRejects(
      async () => {
        await fetch(`http://localhost:${port}`);
      },
      TypeError,
      "client error (Connect)",
    );
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchDnsError() {
    await assertRejects(
      async () => {
        await fetch("http://nil/");
      },
      TypeError,
      "client error (Connect)",
    );
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchInvalidUriError() {
    await assertRejects(
      async () => {
        await fetch("http://<invalid>/");
      },
      TypeError,
    );
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchMalformedUriError() {
    await assertRejects(
      async () => {
        const url = new URL("http://{{google/");
        await fetch(url);
      },
      TypeError,
    );
  },
);

Deno.test({ permissions: { net: true } }, async function fetchJsonSuccess() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
  const json = await response.json();
  assertEquals(json.name, "deno");
});

Deno.test({ permissions: { net: false } }, async function fetchPerm() {
  await assertRejects(async () => {
    await fetch("http://localhost:4545/assets/fixture.json");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { net: true } }, async function fetchUrl() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
  assertEquals(response.url, "http://localhost:4545/assets/fixture.json");
  const _json = await response.json();
});

Deno.test({ permissions: { net: true } }, async function fetchURL() {
  const response = await fetch(
    new URL("http://localhost:4545/assets/fixture.json"),
  );
  assertEquals(response.url, "http://localhost:4545/assets/fixture.json");
  const _json = await response.json();
});

Deno.test({ permissions: { net: true } }, async function fetchHeaders() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
  const headers = response.headers;
  assertEquals(headers.get("Content-Type"), "application/json");
  const _json = await response.json();
});

Deno.test({ permissions: { net: true } }, async function fetchBlob() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
  const headers = response.headers;
  const blob = await response.blob();
  assertEquals(blob.type, headers.get("Content-Type"));
  assertEquals(blob.size, Number(headers.get("Content-Length")));
});

Deno.test(
  { permissions: { net: true } },
  async function fetchBodyUsedReader() {
    const response = await fetch(
      "http://localhost:4545/assets/fixture.json",
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

Deno.test(
  { permissions: { net: true } },
  async function fetchBodyUsedCancelStream() {
    const response = await fetch(
      "http://localhost:4545/assets/fixture.json",
    );
    assert(response.body !== null);

    assertEquals(response.bodyUsed, false);
    const promise = response.body.cancel();
    assertEquals(response.bodyUsed, true);
    await promise;
  },
);

Deno.test({ permissions: { net: true } }, async function fetchAsyncIterator() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
  const headers = response.headers;

  assert(response.body !== null);
  let total = 0;
  for await (const chunk of response.body) {
    assert(chunk instanceof Uint8Array);
    total += chunk.length;
  }

  assertEquals(total, Number(headers.get("Content-Length")));
});

Deno.test({ permissions: { net: true } }, async function fetchBodyReader() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test({ permissions: { net: true } }, async function responseClone() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");
  const response1 = response.clone();
  assert(response !== response1);
  assertEquals(response.status, response1.status);
  assertEquals(response.statusText, response1.statusText);
  const u8a = await response.bytes();
  const u8a1 = await response1.bytes();
  for (let i = 0; i < u8a.byteLength; i++) {
    assertEquals(u8a[i], u8a1[i]);
  }
});

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
  async function fetchMultipartFormBadContentType() {
    const response = await fetch(
      "http://localhost:4545/multipart_form_bad_content_type",
    );
    assert(response.body !== null);

    await assertRejects(
      async () => {
        await response.formData();
      },
      TypeError,
      "Body can not be decoded as form data",
    );
  },
);

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchWithRedirection() {
    const response = await fetch("http://localhost:4546/assets/hello.txt");
    assertEquals(response.status, 200);
    assertEquals(response.statusText, "OK");
    assertEquals(response.url, "http://localhost:4545/assets/hello.txt");
    const body = await response.text();
    assert(body.includes("Hello world!"));
  },
);

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchWithRelativeRedirection() {
    const response = await fetch(
      "http://localhost:4545/run/001_hello.js",
    );
    assertEquals(response.status, 200);
    assertEquals(response.statusText, "OK");
    const body = await response.text();
    assert(body.includes("Hello"));
  },
);

Deno.test(
  {
    permissions: { net: true },
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

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchWithInfRedirection() {
    await assertRejects(
      () => fetch("http://localhost:4549"),
      TypeError,
      "redirect",
    );
  },
);

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
  async function fetchSeparateInit() {
    // related to: https://github.com/denoland/deno/issues/10396
    const req = new Request("http://localhost:4545/run/001_hello.js");
    const init = {
      method: "GET",
    };
    req.headers.set("foo", "bar");
    const res = await fetch(req, init);
    assertEquals(res.status, 200);
    await res.text();
  },
);

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test({ permissions: { net: true } }, async function fetchInitBlobBody() {
  const data = "const a = 1 🦕";
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
  async function fetchInitFormDataBlobFilenameBody() {
    const form = new FormData();
    form.append("field", "value");
    form.append(
      "file",
      new Blob([new TextEncoder().encode("deno")]),
      "file name",
    );
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: form,
    });
    const resultForm = await response.formData();
    assertEquals(form.get("field"), resultForm.get("field"));
    const file = resultForm.get("file");
    assert(file instanceof File);
    assertEquals(file.name, "file name");
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchInitFormDataFileFilenameBody() {
    const form = new FormData();
    form.append("field", "value");
    form.append(
      "file",
      new File([new Blob([new TextEncoder().encode("deno")])], "file name"),
    );
    const response = await fetch("http://localhost:4545/echo_server", {
      method: "POST",
      body: form,
    });
    const resultForm = await response.formData();
    assertEquals(form.get("field"), resultForm.get("field"));
    const file = resultForm.get("file");
    assert(file instanceof File);
    assertEquals(file.name, "file name");
  },
);

Deno.test(
  { permissions: { net: true } },
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

Deno.test({ permissions: { net: true } }, async function fetchUserAgent() {
  const data = "Hello World";
  const response = await fetch("http://localhost:4545/echo_server", {
    method: "POST",
    body: new TextEncoder().encode(data),
  });
  assertEquals(response.headers.get("user-agent"), `Deno/${Deno.version.deno}`);
  await response.text();
});

function bufferServer(addr: string): Promise<Buffer> {
  const [hostname, port] = addr.split(":");
  const listener = Deno.listen({
    hostname,
    port: Number(port),
  }) as Deno.Listener;
  return listener.accept().then(async (conn: Deno.Conn) => {
    const buf = new Buffer();
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
    return buf;
  });
}

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchRequest() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Hello", "World"],
        ["Foo", "Bar"],
      ],
    });
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "content-length: 0\r\n",
      "hello: World\r\n",
      "foo: Bar\r\n",
      "accept: */*\r\n",
      "accept-language: *\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n",
      `host: ${addr}\r\n\r\n`,
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchRequestAcceptHeaders() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Accept", "text/html"],
        ["Accept-Language", "en-US"],
      ],
    });
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "content-length: 0\r\n",
      "accept: text/html\r\n",
      "accept-language: en-US\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n",
      `host: ${addr}\r\n\r\n`,
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchPostBodyString() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
    const body = "hello world";
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Hello", "World"],
        ["Foo", "Bar"],
      ],
      body,
    });
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      `content-length: ${body.length}\r\n`,
      "hello: World\r\n",
      "foo: Bar\r\n",
      "content-type: text/plain;charset=UTF-8\r\n",
      "accept: */*\r\n",
      "accept-language: *\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n",
      `host: ${addr}\r\n`,
      `\r\n`,
      body,
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchPostBodyTypedArray() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
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
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      `content-length: ${body.byteLength}\r\n`,
      "hello: World\r\n",
      "foo: Bar\r\n",
      "accept: */*\r\n",
      "accept-language: *\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n",
      `host: ${addr}\r\n`,
      `\r\n`,
      bodyStr,
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchUserSetContentLength() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Content-Length", "10"],
      ],
    });
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "content-length: 0\r\n",
      "accept: */*\r\n",
      "accept-language: *\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n",
      `host: ${addr}\r\n\r\n`,
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchUserSetTransferEncoding() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
    const response = await fetch(`http://${addr}/blah`, {
      method: "POST",
      headers: [
        ["Transfer-Encoding", "chunked"],
      ],
    });
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "content-length: 0\r\n",
      `host: ${addr}\r\n`,
      "accept: */*\r\n",
      "accept-language: *\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n\r\n",
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test(
  {
    permissions: { net: true },
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

Deno.test(
  {
    permissions: { net: true },
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

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchWithErrorRedirection() {
    await assertRejects(
      () =>
        fetch("http://localhost:4546/", {
          redirect: "error",
        }),
      TypeError,
      "redirect",
    );
  },
);

Deno.test(function responseRedirect() {
  const redir = Response.redirect("http://example.com/newLocation", 301);
  assertEquals(redir.status, 301);
  assertEquals(redir.statusText, "");
  assertEquals(redir.url, "");
  assertEquals(
    redir.headers.get("Location"),
    "http://example.com/newLocation",
  );
  assertEquals(redir.type, "default");
});

Deno.test(function responseRedirectTakeURLObjectAsParameter() {
  const redir = Response.redirect(new URL("https://example.com/"));
  assertEquals(
    redir.headers.get("Location"),
    "https://example.com/",
  );
});

Deno.test(async function responseWithoutBody() {
  const response = new Response();
  assertEquals(await response.bytes(), new Uint8Array(0));
  const blob = await response.blob();
  assertEquals(blob.size, 0);
  assertEquals(await blob.arrayBuffer(), new ArrayBuffer(0));
  assertEquals(await response.text(), "");
  await assertRejects(async () => {
    await response.json();
  });
});

Deno.test({ permissions: { net: true } }, async function fetchBodyReadTwice() {
  const response = await fetch("http://localhost:4545/assets/fixture.json");

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

Deno.test(
  { permissions: { net: true } },
  async function fetchBodyReaderAfterRead() {
    const response = await fetch(
      "http://localhost:4545/assets/fixture.json",
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
  async function fetchResourceCloseAfterStreamCancel() {
    const res = await fetch("http://localhost:4545/assets/fixture.json");
    assert(res.body !== null);

    // After ReadableStream.cancel is called, resource handle must be closed
    // The test should not fail with: Test case is leaking resources
    await res.body.cancel();
  },
);

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  { permissions: { net: true } },
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

Deno.test(function fetchResponseConstructorNullBody() {
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

Deno.test(function fetchResponseConstructorInvalidStatus() {
  const invalidStatus = [100, 600, 199, null, "", NaN];

  for (const status of invalidStatus) {
    try {
      // deno-lint-ignore ban-ts-comment
      // @ts-ignore
      new Response("deno", { status });
      fail(`Invalid status: ${status}`);
    } catch (e) {
      assert(e instanceof RangeError);
      assert(
        e.message.endsWith(
          "is not equal to 101 and outside the range [200, 599]",
        ),
      );
    }
  }
});

Deno.test(function fetchResponseEmptyConstructor() {
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

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchCustomHttpClientParamCertificateSuccess(): Promise<
    void
  > {
    const caCert = Deno.readTextFileSync("tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    assert(client instanceof Deno.HttpClient);
    const response = await fetch("https://localhost:5545/assets/fixture.json", {
      client,
    });
    const json = await response.json();
    assertEquals(json.name, "deno");
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  function createHttpClientAcceptPoolIdleTimeout() {
    const client = Deno.createHttpClient({
      poolIdleTimeout: 1000,
    });
    client.close();
  },
);

Deno.test(
  { permissions: { net: true } },
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

Deno.test(
  {
    permissions: { net: true },
  },
  async function fetchPostBodyReadableStream() {
    const addr = `127.0.0.1:${listenPort}`;
    const bufPromise = bufferServer(addr);
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
    await response.body?.cancel();
    assertEquals(response.status, 404);
    assertEquals(response.headers.get("Content-Length"), "2");

    const actual = new TextDecoder().decode((await bufPromise).bytes());
    const expected = [
      "POST /blah HTTP/1.1\r\n",
      "hello: World\r\n",
      "foo: Bar\r\n",
      "accept: */*\r\n",
      "accept-language: *\r\n",
      `user-agent: Deno/${Deno.version.deno}\r\n`,
      "accept-encoding: gzip,br\r\n",
      `host: ${addr}\r\n`,
      `transfer-encoding: chunked\r\n\r\n`,
      "B\r\n",
      "hello world\r\n",
      "0\r\n\r\n",
    ].join("");
    assertEquals(actual, expected);
  },
);

Deno.test({}, function fetchWritableRespProps() {
  const original = new Response("https://deno.land", {
    status: 404,
    headers: { "x-deno": "foo" },
  });
  const new_ = new Response("https://deno.land", original);
  assertEquals(original.status, new_.status);
  assertEquals(new_.headers.get("x-deno"), "foo");
});

Deno.test(
  { permissions: { net: true } },
  async function fetchFilterOutCustomHostHeader() {
    const addr = `127.0.0.1:${listenPort}`;
    const server = Deno.serve({ port: listenPort }, (req) => {
      return new Response(`Host header was ${req.headers.get("Host")}`);
    });
    const response = await fetch(`http://${addr}/`, {
      headers: { "Host": "example.com" },
    });
    assertEquals(await response.text(), `Host header was ${addr}`);
    await server.shutdown();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchNoServerReadableStreamBody() {
    const completed = Promise.withResolvers<void>();
    const failed = Promise.withResolvers<void>();
    const body = new ReadableStream({
      start(controller) {
        controller.enqueue(new Uint8Array([1]));
        setTimeout(async () => {
          // This is technically a race. If the fetch has failed by this point, the enqueue will
          // throw. If not, it will succeed. Windows appears to take a while to time out the fetch,
          // so we will just wait for that here before we attempt to enqueue so it's consistent
          // across platforms.
          await failed.promise;
          assertThrows(() => controller.enqueue(new Uint8Array([2])));
          completed.resolve();
        }, 1000);
      },
    });
    const nonExistentHostname = "http://localhost:47582";
    await assertRejects(async () => {
      await fetch(nonExistentHostname, { body, method: "POST" });
    }, TypeError);
    failed.resolve();
    await completed.promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchHeadRespBody() {
    const res = await fetch("http://localhost:4545/echo_server", {
      method: "HEAD",
    });
    assertEquals(res.body, null);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function fetchClientCertWrongPrivateKey(): Promise<void> {
    await assertRejects(async () => {
      const client = Deno.createHttpClient({
        cert: "bad data",
        key: await Deno.readTextFile(
          "tests/testdata/tls/localhost.key",
        ),
      });
      await fetch("https://localhost:5552/assets/fixture.json", {
        client,
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function fetchClientCertBadPrivateKey(): Promise<void> {
    await assertRejects(async () => {
      const client = Deno.createHttpClient({
        cert: await Deno.readTextFile(
          "tests/testdata/tls/localhost.crt",
        ),
        key: "bad data",
      });
      await fetch("https://localhost:5552/assets/fixture.json", {
        client,
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function fetchClientCertNotPrivateKey(): Promise<void> {
    await assertRejects(async () => {
      const client = Deno.createHttpClient({
        cert: await Deno.readTextFile(
          "tests/testdata/tls/localhost.crt",
        ),
        key: "",
      });
      await fetch("https://localhost:5552/assets/fixture.json", {
        client,
      });
    }, Deno.errors.InvalidData);
  },
);

Deno.test(
  { permissions: { read: true, net: true } },
  async function fetchCustomClientPrivateKey(): Promise<
    void
  > {
    const data = "Hello World";
    const caCert = await Deno.readTextFile("tests/testdata/tls/RootCA.crt");
    const client = Deno.createHttpClient({
      cert: await Deno.readTextFile(
        "tests/testdata/tls/localhost.crt",
      ),
      key: await Deno.readTextFile(
        "tests/testdata/tls/localhost.key",
      ),
      caCerts: [caCert],
    });
    const response = await fetch("https://localhost:5552/echo_server", {
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

Deno.test(
  { permissions: { net: true } },
  async function fetchAbortWhileUploadStreaming(): Promise<void> {
    const abortController = new AbortController();
    try {
      await fetch(
        "http://localhost:5552/echo_server",
        {
          method: "POST",
          body: new ReadableStream({
            pull(controller) {
              abortController.abort();
              controller.enqueue(new Uint8Array([1, 2, 3, 4]));
            },
          }),
          signal: abortController.signal,
        },
      );
      fail("Fetch didn't reject.");
    } catch (error) {
      assert(error instanceof DOMException);
      assertEquals(error.name, "AbortError");
      assertEquals(error.message, "The signal has been aborted");
    }
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchAbortWhileUploadStreamingWithReason(): Promise<void> {
    const abortController = new AbortController();
    const abortReason = new Error();
    try {
      await fetch(
        "http://localhost:5552/echo_server",
        {
          method: "POST",
          body: new ReadableStream({
            pull(controller) {
              abortController.abort(abortReason);
              controller.enqueue(new Uint8Array([1, 2, 3, 4]));
            },
          }),
          signal: abortController.signal,
        },
      );
      fail("Fetch didn't reject.");
    } catch (error) {
      assertEquals(error, abortReason);
    }
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchAbortWhileUploadStreamingWithPrimitiveReason(): Promise<
    void
  > {
    const abortController = new AbortController();
    try {
      await fetch(
        "http://localhost:5552/echo_server",
        {
          method: "POST",
          body: new ReadableStream({
            pull(controller) {
              abortController.abort("Abort reason");
              controller.enqueue(new Uint8Array([1, 2, 3, 4]));
            },
          }),
          signal: abortController.signal,
        },
      );
      fail("Fetch didn't reject.");
    } catch (error) {
      assertEquals(error, "Abort reason");
    }
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchHeaderValueShouldNotPanic() {
    for (let i = 0; i < 0x21; i++) {
      if (i === 0x09 || i === 0x0A || i === 0x0D || i === 0x20) {
        continue; // these header value will be normalized, will not cause an error.
      }
      // ensure there will be an error instead of panic.
      await assertRejects(() =>
        fetch("http://localhost:4545/echo_server", {
          method: "HEAD",
          headers: { "val": String.fromCharCode(i) },
        }), TypeError);
    }
    await assertRejects(() =>
      fetch("http://localhost:4545/echo_server", {
        method: "HEAD",
        headers: { "val": String.fromCharCode(127) },
      }), TypeError);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchHeaderNameShouldNotPanic() {
    const validTokens =
      "!#$%&'*+-.0123456789ABCDEFGHIJKLMNOPQRSTUWVXYZ^_`abcdefghijklmnopqrstuvwxyz|~"
        .split("");
    for (let i = 0; i <= 255; i++) {
      const token = String.fromCharCode(i);
      if (validTokens.includes(token)) {
        continue;
      }
      // ensure there will be an error instead of panic.
      await assertRejects(() =>
        fetch("http://localhost:4545/echo_server", {
          method: "HEAD",
          headers: { [token]: "value" },
        }), TypeError);
    }
    await assertRejects(() =>
      fetch("http://localhost:4545/echo_server", {
        method: "HEAD",
        headers: { "": "value" },
      }), TypeError);
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchSupportsHttpsOverIpAddress() {
    const caCert = await Deno.readTextFile("tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const res = await fetch("https://localhost:5546/http_version", { client });
    assert(res.ok);
    assertEquals(await res.text(), "HTTP/1.1");
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchSupportsHttp1Only() {
    const caCert = await Deno.readTextFile("tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const res = await fetch("https://localhost:5546/http_version", { client });
    assert(res.ok);
    assertEquals(await res.text(), "HTTP/1.1");
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchSupportsHttp2() {
    const caCert = await Deno.readTextFile("tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const res = await fetch("https://localhost:5547/http_version", { client });
    assert(res.ok);
    assertEquals(await res.text(), "HTTP/2.0");
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchForceHttp1OnHttp2Server() {
    const client = Deno.createHttpClient({ http2: false, http1: true });
    await assertRejects(
      () => fetch("http://localhost:5549/http_version", { client }),
      TypeError,
    );
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchForceHttp2OnHttp1Server() {
    const client = Deno.createHttpClient({ http2: true, http1: false });
    await assertRejects(
      () => fetch("http://localhost:5548/http_version", { client }),
      TypeError,
    );
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function fetchPrefersHttp2() {
    const caCert = await Deno.readTextFile("tests/testdata/tls/RootCA.pem");
    const client = Deno.createHttpClient({ caCerts: [caCert] });
    const res = await fetch("https://localhost:5545/http_version", { client });
    assert(res.ok);
    assertEquals(await res.text(), "HTTP/2.0");
    client.close();
  },
);

Deno.test(
  { permissions: { net: true, read: true } },
  async function createHttpClientAllowHost() {
    const client = Deno.createHttpClient({
      allowHost: true,
    });
    const res = await fetch("http://localhost:4545/echo_server", {
      headers: {
        "host": "example.com",
      },
      client,
    });
    assert(res.ok);
    assertEquals(res.headers.get("host"), "example.com");
    await res.body?.cancel();
    client.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function createHttpClientExplicitResourceManagement() {
    using client = Deno.createHttpClient({});
    const response = await fetch("http://localhost:4545/assets/fixture.json", {
      client,
    });
    const json = await response.json();
    assertEquals(json.name, "deno");
  },
);

Deno.test(
  { permissions: { net: true } },
  async function createHttpClientExplicitResourceManagementDoubleClose() {
    using client = Deno.createHttpClient({});
    const response = await fetch("http://localhost:4545/assets/fixture.json", {
      client,
    });
    const json = await response.json();
    assertEquals(json.name, "deno");
    // Close the client even though we declared it with `using` to confirm that
    // the cleanup done as per `Symbol.dispose` will not throw any errors.
    client.close();
  },
);

Deno.test({ permissions: { read: false } }, async function fetchFilePerm() {
  await assertRejects(async () => {
    await fetch(import.meta.resolve("../testdata/subdir/json_1.json"));
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { read: false } },
  async function fetchFilePermDoesNotExist() {
    await assertRejects(async () => {
      await fetch(import.meta.resolve("./bad.json"));
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fetchFileBadMethod() {
    await assertRejects(
      async () => {
        await fetch(
          import.meta.resolve("../testdata/subdir/json_1.json"),
          {
            method: "POST",
          },
        );
      },
      TypeError,
      "Fetching files only supports the GET method: received POST",
    );
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fetchFileDoesNotExist() {
    await assertRejects(
      async () => {
        await fetch(import.meta.resolve("./bad.json"));
      },
      TypeError,
    );
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fetchFile() {
    const res = await fetch(
      import.meta.resolve("../testdata/subdir/json_1.json"),
    );
    assert(res.ok);
    const fixture = await Deno.readTextFile(
      "tests/testdata/subdir/json_1.json",
    );
    assertEquals(await res.text(), fixture);
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchContentLengthPost() {
    const response = await fetch("http://localhost:4545/content_length", {
      method: "POST",
    });
    const length = await response.text();
    assertEquals(length, 'Some("0")');
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchContentLengthPut() {
    const response = await fetch("http://localhost:4545/content_length", {
      method: "PUT",
    });
    const length = await response.text();
    assertEquals(length, 'Some("0")');
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchContentLengthPatch() {
    const response = await fetch("http://localhost:4545/content_length", {
      method: "PATCH",
    });
    const length = await response.text();
    assertEquals(length, "None");
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchContentLengthPostWithStringBody() {
    const response = await fetch("http://localhost:4545/content_length", {
      method: "POST",
      body: "Hey!",
    });
    const length = await response.text();
    assertEquals(length, 'Some("4")');
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchContentLengthPostWithBufferBody() {
    const response = await fetch("http://localhost:4545/content_length", {
      method: "POST",
      body: new TextEncoder().encode("Hey!"),
    });
    const length = await response.text();
    assertEquals(length, 'Some("4")');
  },
);

Deno.test(async function staticResponseJson() {
  const data = { hello: "world" };
  const resp = Response.json(data);
  assertEquals(resp.status, 200);
  assertEquals(resp.headers.get("content-type"), "application/json");
  const res = await resp.json();
  assertEquals(res, data);
});

function invalidServer(addr: string, body: Uint8Array): Deno.Listener {
  const [hostname, port] = addr.split(":");
  const listener = Deno.listen({
    hostname,
    port: Number(port),
  }) as Deno.Listener;

  (async () => {
    for await (const conn of listener) {
      const p1 = conn.read(new Uint8Array(2 ** 14));
      const p2 = conn.write(body);

      await Promise.all([p1, p2]);
      conn.close();
    }
  })();

  return listener;
}

Deno.test(
  { permissions: { net: true } },
  async function fetchWithInvalidContentLengthAndTransferEncoding(): Promise<
    void
  > {
    const addr = `127.0.0.1:${listenPort}`;
    const data = "a".repeat(10 << 10);

    const body = new TextEncoder().encode(
      `HTTP/1.1 200 OK\r\nContent-Length: ${
        Math.round(data.length * 2)
      }\r\nTransfer-Encoding: chunked\r\n\r\n${
        data.length.toString(16)
      }\r\n${data}\r\n0\r\n\r\n`,
    );

    // if transfer-encoding is sent, content-length is ignored
    // even if it has an invalid value (content-length > totalLength)
    const listener = invalidServer(addr, body);
    const response = await fetch(`http://${addr}/`);

    const res = await response.bytes();
    const buf = new TextEncoder().encode(data);
    assertEquals(res, buf);

    listener.close();
  },
);

Deno.test(
  // TODO(bartlomieju): reenable this test
  // https://github.com/denoland/deno/issues/18350
  { ignore: Deno.build.os === "windows", permissions: { net: true } },
  async function fetchWithInvalidContentLength(): Promise<
    void
  > {
    const addr = `127.0.0.1:${listenPort}`;
    const data = "a".repeat(10 << 10);

    const body = new TextEncoder().encode(
      `HTTP/1.1 200 OK\r\nContent-Length: ${
        Math.round(data.length / 2)
      }\r\nContent-Length: ${data.length}\r\n\r\n${data}`,
    );

    // It should fail if multiple content-length headers with different values are sent
    const listener = invalidServer(addr, body);
    await assertRejects(
      async () => {
        await fetch(`http://${addr}/`);
      },
      TypeError,
      "client error",
    );

    listener.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchWithInvalidContentLength2(): Promise<
    void
  > {
    const addr = `127.0.0.1:${listenPort}`;
    const data = "a".repeat(10 << 10);

    const contentLength = data.length / 2;
    const body = new TextEncoder().encode(
      `HTTP/1.1 200 OK\r\nContent-Length: ${contentLength}\r\n\r\n${data}`,
    );

    const listener = invalidServer(addr, body);
    const response = await fetch(`http://${addr}/`);

    // If content-length < totalLength, a maximum of content-length bytes
    // should be returned.
    const res = await response.bytes();
    const buf = new TextEncoder().encode(data);
    assertEquals(res.byteLength, contentLength);
    assertEquals(res, buf.subarray(contentLength));

    listener.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchWithInvalidContentLength3(): Promise<
    void
  > {
    const addr = `127.0.0.1:${listenPort}`;
    const data = "a".repeat(10 << 10);

    const contentLength = data.length * 2;
    const body = new TextEncoder().encode(
      `HTTP/1.1 200 OK\r\nContent-Length: ${contentLength}\r\n\r\n${data}`,
    );

    const listener = invalidServer(addr, body);
    const response = await fetch(`http://${addr}/`);
    // If content-length > totalLength, a maximum of content-length bytes
    // should be returned.
    await assertRejects(
      async () => {
        await response.arrayBuffer();
      },
      Error,
      "body",
    );

    listener.close();
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchBlobUrl(): Promise<void> {
    const blob = new Blob(["ok"], { type: "text/plain" });
    const url = URL.createObjectURL(blob);
    assert(url.startsWith("blob:"), `URL was ${url}`);
    const res = await fetch(url);
    assertEquals(res.url, url);
    assertEquals(res.status, 200);
    assertEquals(res.headers.get("content-length"), "2");
    assertEquals(res.headers.get("content-type"), "text/plain");
    assertEquals(await res.text(), "ok");
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchResponseStreamIsLockedWhileReading() {
    const response = await fetch("http://localhost:4545/echo_server", {
      body: new Uint8Array(5000),
      method: "POST",
    });

    assertEquals(response.body!.locked, false);
    const promise = response.arrayBuffer();
    assertEquals(response.body!.locked, true);

    await promise;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchConstructorClones() {
    const req = new Request("https://example.com", {
      method: "POST",
      body: "foo",
    });
    assertEquals(await req.text(), "foo");
    await assertRejects(() => req.text());

    const req2 = new Request(req, { method: "PUT", body: "bar" }); // should not have any impact on req
    await assertRejects(() => req.text());
    assertEquals(await req2.text(), "bar");

    assertEquals(req.method, "POST");
    assertEquals(req2.method, "PUT");

    assertEquals(req.headers.get("x-foo"), null);
    assertEquals(req2.headers.get("x-foo"), null);
    req2.headers.set("x-foo", "bar"); // should not have any impact on req
    assertEquals(req.headers.get("x-foo"), null);
    assertEquals(req2.headers.get("x-foo"), "bar");
  },
);

Deno.test(
  // TODO(bartlomieju): reenable this test
  // https://github.com/denoland/deno/issues/18350
  { ignore: Deno.build.os === "windows", permissions: { net: true } },
  async function fetchRequestBodyErrorCatchable() {
    const listener = Deno.listen({ hostname: "127.0.0.1", port: listenPort });
    const server = (async () => {
      const conn = await listener.accept();
      listener.close();
      const buf = new Uint8Array(256);
      const n = await conn.read(buf);
      const data = new TextDecoder().decode(buf.subarray(0, n!)); // this is the request headers + first body chunk
      assert(data.startsWith("POST / HTTP/1.1\r\n"));
      assert(data.endsWith("1\r\na\r\n"));
      const n2 = await conn.read(buf);
      assertEquals(n2, 6); // this is the second body chunk
      const n3 = await conn.read(buf);
      assertEquals(n3, null); // the connection now abruptly closes because the client has errored
      conn.close();
    })();

    const stream = new ReadableStream({
      async start(controller) {
        controller.enqueue(new TextEncoder().encode("a"));
        await delay(1000);
        controller.enqueue(new TextEncoder().encode("b"));
        await delay(1000);
        controller.error(new Error("foo"));
      },
    });

    const url = `http://localhost:${listenPort}/`;
    const err = await assertRejects(() =>
      fetch(url, {
        body: stream,
        method: "POST",
      })
    );

    assert(err instanceof TypeError, `err was ${err}`);

    assertStringIncludes(
      err.message,
      "error sending request from 127.0.0.1:",
      `err.message was ${err.message}`,
    );
    assertStringIncludes(
      err.message,
      ` for http://localhost:${listenPort}/ (127.0.0.1:${listenPort}): client error (SendRequest): error from user's Body stream`,
      `err.message was ${err.message}`,
    );

    assert(err.cause, `err.cause was null ${err}`);
    assert(
      err.cause instanceof Error,
      `err.cause was not an Error ${err.cause}`,
    );
    assertEquals(err.cause.message, "foo");

    await server;
  },
);

Deno.test(
  { permissions: { net: true } },
  async function fetchRequestBodyEmptyStream() {
    const body = new ReadableStream({
      start(controller) {
        controller.enqueue(new Uint8Array([]));
        controller.close();
      },
    });

    await assertRejects(
      async () => {
        const controller = new AbortController();
        const promise = fetch("http://localhost:4545/echo_server", {
          body,
          method: "POST",
          signal: controller.signal,
        });
        try {
          controller.abort();
        } catch (e) {
          console.log(e);
          fail("abort should not throw");
        }
        await promise;
      },
      DOMException,
      "The signal has been aborted",
    );
  },
);

Deno.test("Request with subarray TypedArray body", async () => {
  const body = new Uint8Array([1, 2, 3, 4, 5]).subarray(1);
  const req = new Request("https://example.com", { method: "POST", body });
  const actual = await req.bytes();
  const expected = new Uint8Array([2, 3, 4, 5]);
  assertEquals(actual, expected);
});

Deno.test("Response with subarray TypedArray body", async () => {
  const body = new Uint8Array([1, 2, 3, 4, 5]).subarray(1);
  const req = new Response(body);
  const actual = await req.bytes();
  const expected = new Uint8Array([2, 3, 4, 5]);
  assertEquals(actual, expected);
});

// Regression test for https://github.com/denoland/deno/issues/24697
Deno.test("URL authority is used as 'Authorization' header", async () => {
  const deferred = Promise.withResolvers<string | null | undefined>();
  const ac = new AbortController();

  const server = Deno.serve({ port: 4502, signal: ac.signal }, (req) => {
    deferred.resolve(req.headers.get("authorization"));
    return new Response("Hello world");
  });

  const res = await fetch("http://deno:land@localhost:4502");
  await res.text();
  const authHeader = await deferred.promise;
  ac.abort();
  await server.finished;
  assertEquals(authHeader, "Basic ZGVubzpsYW5k");
});

Deno.test(
  { permissions: { net: true } },
  async function errorMessageIncludesUrlAndDetailsWithNoTcpInfo() {
    await assertRejects(
      () => fetch("http://example.invalid"),
      TypeError,
      "error sending request for url (http://example.invalid/): client error (Connect): dns error: ",
    );
  },
);

Deno.test(
  { permissions: { net: true } },
  async function errorMessageIncludesUrlAndDetailsWithTcpInfo() {
    const listener = Deno.listen({ port: listenPort });
    const server = (async () => {
      const conn = await listener.accept();
      listener.close();
      // Immediately close the connection to simulate a connection error
      conn.close();
    })();

    const url = `http://localhost:${listenPort}`;
    const err = await assertRejects(() => fetch(url));

    assert(err instanceof TypeError, `${err}`);
    assertStringIncludes(
      err.message,
      "error sending request from 127.0.0.1:",
      `${err.message}`,
    );
    assertStringIncludes(
      err.message,
      ` for http://localhost:${listenPort}/ (127.0.0.1:${listenPort}): client error (SendRequest): `,
      `${err.message}`,
    );

    await server;
  },
);
