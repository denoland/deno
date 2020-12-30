// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertNotEquals,
  assertThrowsAsync,
} from "../testing/asserts.ts";
import {
  bodyReader,
  chunkedBodyReader,
  parseHTTPVersion,
  readRequest,
  readTrailers,
  writeResponse,
  writeTrailers,
} from "./_io.ts";
import { decode, encode } from "../encoding/utf8.ts";
import { BufReader, ReadLineResult } from "../io/bufio.ts";
import { Response, ServerRequest } from "./server.ts";
import { StringReader } from "../io/readers.ts";
import { mockConn } from "./_mock_conn.ts";

Deno.test("bodyReader", async () => {
  const text = "Hello, Deno";
  const r = bodyReader(
    text.length,
    new BufReader(new Deno.Buffer(encode(text))),
  );
  assertEquals(decode(await Deno.readAll(r)), text);
});
function chunkify(n: number, char: string): string {
  const v = Array.from({ length: n })
    .map(() => `${char}`)
    .join("");
  return `${n.toString(16)}\r\n${v}\r\n`;
}
Deno.test("chunkedBodyReader", async () => {
  const body = [
    chunkify(3, "a"),
    chunkify(5, "b"),
    chunkify(11, "c"),
    chunkify(22, "d"),
    chunkify(0, ""),
  ].join("");
  const h = new Headers();
  const r = chunkedBodyReader(h, new BufReader(new Deno.Buffer(encode(body))));
  let result: number | null;
  // Use small buffer as some chunks exceed buffer size
  const buf = new Uint8Array(5);
  const dest = new Deno.Buffer();
  while ((result = await r.read(buf)) !== null) {
    const len = Math.min(buf.byteLength, result);
    await dest.write(buf.subarray(0, len));
  }
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(new TextDecoder().decode(dest.bytes()), exp);
});

Deno.test("chunkedBodyReader with trailers", async () => {
  const body = [
    chunkify(3, "a"),
    chunkify(5, "b"),
    chunkify(11, "c"),
    chunkify(22, "d"),
    chunkify(0, ""),
    "deno: land\r\n",
    "node: js\r\n",
    "\r\n",
  ].join("");
  const h = new Headers({
    trailer: "deno,node",
  });
  const r = chunkedBodyReader(h, new BufReader(new Deno.Buffer(encode(body))));
  assertEquals(h.has("trailer"), true);
  assertEquals(h.has("deno"), false);
  assertEquals(h.has("node"), false);
  const act = decode(await Deno.readAll(r));
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(act, exp);
  assertEquals(h.has("trailer"), false);
  assertEquals(h.get("deno"), "land");
  assertEquals(h.get("node"), "js");
});

Deno.test("readTrailers", async () => {
  const h = new Headers({
    trailer: "Deno, Node",
  });
  const trailer = ["deno: land", "node: js", "", ""].join("\r\n");
  await readTrailers(h, new BufReader(new Deno.Buffer(encode(trailer))));
  assertEquals(h.has("trailer"), false);
  assertEquals(h.get("deno"), "land");
  assertEquals(h.get("node"), "js");
});

Deno.test(
  "readTrailer should throw if undeclared headers found in trailer",
  async () => {
    const patterns = [
      ["deno,node", "deno: land\r\nnode: js\r\ngo: lang\r\n\r\n"],
      ["deno", "node: js\r\n\r\n"],
      ["deno", "node:js\r\ngo: lang\r\n\r\n"],
    ];
    for (const [header, trailer] of patterns) {
      const h = new Headers({
        trailer: header,
      });
      await assertThrowsAsync(
        async () => {
          await readTrailers(
            h,
            new BufReader(new Deno.Buffer(encode(trailer))),
          );
        },
        Deno.errors.InvalidData,
        `Undeclared trailers: [ "`,
      );
    }
  },
);

Deno.test(
  "readTrailer should throw if trailer contains prohibited fields",
  async () => {
    for (const f of ["Content-Length", "Trailer", "Transfer-Encoding"]) {
      const h = new Headers({
        trailer: f,
      });
      await assertThrowsAsync(
        async () => {
          await readTrailers(h, new BufReader(new Deno.Buffer()));
        },
        Deno.errors.InvalidData,
        `Prohibited trailer names: [ "`,
      );
    }
  },
);

Deno.test("writeTrailer", async () => {
  const w = new Deno.Buffer();
  await writeTrailers(
    w,
    new Headers({ "transfer-encoding": "chunked", trailer: "deno,node" }),
    new Headers({ deno: "land", node: "js" }),
  );
  assertEquals(
    new TextDecoder().decode(w.bytes()),
    "deno: land\r\nnode: js\r\n\r\n",
  );
});

Deno.test("writeTrailer should throw", async () => {
  const w = new Deno.Buffer();
  await assertThrowsAsync(
    () => {
      return writeTrailers(w, new Headers(), new Headers());
    },
    TypeError,
    "Missing trailer header.",
  );
  await assertThrowsAsync(
    () => {
      return writeTrailers(w, new Headers({ trailer: "deno" }), new Headers());
    },
    TypeError,
    `Trailers are only allowed for "transfer-encoding: chunked", got "transfer-encoding: null".`,
  );
  for (const f of ["content-length", "trailer", "transfer-encoding"]) {
    await assertThrowsAsync(
      () => {
        return writeTrailers(
          w,
          new Headers({ "transfer-encoding": "chunked", trailer: f }),
          new Headers({ [f]: "1" }),
        );
      },
      TypeError,
      `Prohibited trailer names: [ "`,
    );
  }
  await assertThrowsAsync(
    () => {
      return writeTrailers(
        w,
        new Headers({ "transfer-encoding": "chunked", trailer: "deno" }),
        new Headers({ node: "js" }),
      );
    },
    TypeError,
    `Undeclared trailers: [ "node" ].`,
  );
});

// Ported from https://github.com/golang/go/blob/f5c43b9/src/net/http/request_test.go#L535-L565
Deno.test("parseHttpVersion", (): void => {
  const testCases = [
    { in: "HTTP/0.9", want: [0, 9] },
    { in: "HTTP/1.0", want: [1, 0] },
    { in: "HTTP/1.1", want: [1, 1] },
    { in: "HTTP/3.14", want: [3, 14] },
    { in: "HTTP", err: true },
    { in: "HTTP/one.one", err: true },
    { in: "HTTP/1.1/", err: true },
    { in: "HTTP/-1.0", err: true },
    { in: "HTTP/0.-1", err: true },
    { in: "HTTP/", err: true },
    { in: "HTTP/1,0", err: true },
    { in: "HTTP/1.1000001", err: true },
  ];
  for (const t of testCases) {
    let r, err;
    try {
      r = parseHTTPVersion(t.in);
    } catch (e) {
      err = e;
    }
    if (t.err) {
      assert(err instanceof Error, t.in);
    } else {
      assertEquals(err, undefined);
      assertEquals(r, t.want, t.in);
    }
  }
});

Deno.test("writeUint8ArrayResponse", async function (): Promise<void> {
  const shortText = "Hello";

  const body = new TextEncoder().encode(shortText);
  const res: Response = { body };

  const buf = new Deno.Buffer();
  await writeResponse(buf, res);

  const decoder = new TextDecoder("utf-8");
  const reader = new BufReader(buf);

  let r: ReadLineResult | null = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), "HTTP/1.1 200 OK");
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), `content-length: ${shortText.length}`);
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(r.line.byteLength, 0);
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), shortText);
  assertEquals(r.more, false);

  const eof = await reader.readLine();
  assertEquals(eof, null);
});

Deno.test("writeStringResponse", async function (): Promise<void> {
  const body = "Hello";

  const res: Response = { body };

  const buf = new Deno.Buffer();
  await writeResponse(buf, res);

  const decoder = new TextDecoder("utf-8");
  const reader = new BufReader(buf);

  let r: ReadLineResult | null = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), "HTTP/1.1 200 OK");
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), `content-length: ${body.length}`);
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(r.line.byteLength, 0);
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), body);
  assertEquals(r.more, false);

  const eof = await reader.readLine();
  assertEquals(eof, null);
});

Deno.test("writeStringReaderResponse", async function (): Promise<void> {
  const shortText = "Hello";

  const body = new StringReader(shortText);
  const res: Response = { body };

  const buf = new Deno.Buffer();
  await writeResponse(buf, res);

  const decoder = new TextDecoder("utf-8");
  const reader = new BufReader(buf);

  let r: ReadLineResult | null = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), "HTTP/1.1 200 OK");
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), "transfer-encoding: chunked");
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(r.line.byteLength, 0);
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), shortText.length.toString());
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), shortText);
  assertEquals(r.more, false);

  r = await reader.readLine();
  assert(r !== null);
  assertEquals(decoder.decode(r.line), "0");
  assertEquals(r.more, false);
});

Deno.test("writeResponse with trailer", async () => {
  const w = new Deno.Buffer();
  const body = new StringReader("Hello");
  await writeResponse(w, {
    status: 200,
    headers: new Headers({
      "transfer-encoding": "chunked",
      trailer: "deno,node",
    }),
    body,
    trailers: () => new Headers({ deno: "land", node: "js" }),
  });
  const ret = new TextDecoder().decode(w.bytes());
  const exp = [
    "HTTP/1.1 200 OK",
    "transfer-encoding: chunked",
    "trailer: deno,node",
    "",
    "5",
    "Hello",
    "0",
    "",
    "deno: land",
    "node: js",
    "",
    "",
  ].join("\r\n");
  assertEquals(ret, exp);
});

Deno.test("writeResponseShouldNotModifyOriginHeaders", async () => {
  const headers = new Headers();
  const buf = new Deno.Buffer();

  await writeResponse(buf, { body: "foo", headers });
  assert(decode(await Deno.readAll(buf)).includes("content-length: 3"));

  await writeResponse(buf, { body: "hello", headers });
  assert(decode(await Deno.readAll(buf)).includes("content-length: 5"));
});

Deno.test("readRequestError", async function (): Promise<void> {
  const input = `GET / HTTP/1.1
malformedHeader
`;
  const reader = new BufReader(new StringReader(input));
  let err;
  try {
    await readRequest(mockConn(), reader);
  } catch (e) {
    err = e;
  }
  assert(err instanceof Error);
  assertEquals(err.message, "malformed MIME header line: malformedHeader");
});

// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/net/http/request_test.go#L377-L443
// TODO(zekth) fix tests
Deno.test("testReadRequestError", async function (): Promise<void> {
  const testCases = [
    {
      in: "GET / HTTP/1.1\r\nheader: foo\r\n\r\n",
      headers: [{ key: "header", value: "foo" }],
    },
    {
      in: "GET / HTTP/1.1\r\nheader:foo\r\n",
      err: Deno.errors.UnexpectedEof,
    },
    {
      in: "POST / HTTP/1.0\r\n\r\n",
      headers: [],
      version: true,
    },
    { in: "", eof: true },
    {
      in: "HEAD / HTTP/1.1\r\nContent-Length:4\r\n\r\n",
      err: "http: method cannot contain a Content-Length",
    },
    {
      in: "HEAD / HTTP/1.1\r\n\r\n",
      headers: [],
    },
    // Multiple Content-Length values should either be
    // deduplicated if same or reject otherwise
    // See Issue 16490.
    {
      in: "POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 0\r\n\r\n" +
        "Gopher hey\r\n",
      err: "cannot contain multiple Content-Length headers",
    },
    {
      in: "POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 6\r\n\r\n" +
        "Gopher\r\n",
      err: "cannot contain multiple Content-Length headers",
    },
    {
      in: "PUT / HTTP/1.1\r\nContent-Length: 6 \r\nContent-Length: 6\r\n" +
        "Content-Length:6\r\n\r\nGopher\r\n",
      headers: [{ key: "Content-Length", value: "6" }],
    },
    {
      in: "PUT / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 6 \r\n\r\n",
      err: "cannot contain multiple Content-Length headers",
    },
    // Setting an empty header is swallowed by textproto
    // see: readMIMEHeader()
    // {
    //   in: "POST / HTTP/1.1\r\nContent-Length:\r\nContent-Length: 3\r\n\r\n",
    //   err: "cannot contain multiple Content-Length headers"
    // },
    {
      in: "HEAD / HTTP/1.1\r\nContent-Length:0\r\nContent-Length: 0\r\n\r\n",
      headers: [{ key: "Content-Length", value: "0" }],
    },
    {
      in: "POST / HTTP/1.1\r\nContent-Length:0\r\ntransfer-encoding: " +
        "chunked\r\n\r\n",
      headers: [],
      err: "http: Transfer-Encoding and Content-Length cannot be send together",
    },
  ];
  for (const test of testCases) {
    const reader = new BufReader(new StringReader(test.in));
    let err;
    let req: ServerRequest | null = null;
    try {
      req = await readRequest(mockConn(), reader);
    } catch (e) {
      err = e;
    }
    if (test.eof) {
      assertEquals(req, null);
    } else if (typeof test.err === "string") {
      assertEquals(err.message, test.err);
    } else if (test.err) {
      assert(err instanceof (test.err as typeof Deno.errors.UnexpectedEof));
    } else {
      assert(req instanceof ServerRequest);
      if (test.version) {
        // return value order of parseHTTPVersion() function have to match with [req.protoMajor, req.protoMinor];
        const version = parseHTTPVersion(test.in.split(" ", 3)[2]);
        assertEquals(req.protoMajor, version[0]);
        assertEquals(req.protoMinor, version[1]);
      }
      assert(test.headers);
      assertEquals(err, undefined);
      assertNotEquals(req, null);
      for (const h of test.headers) {
        assertEquals(req.headers.get(h.key), h.value);
      }
    }
  }
});
