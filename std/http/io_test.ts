import {
  AssertionError,
  assertThrowsAsync,
  assertEquals,
  assert,
  assertNotEquals,
} from "../testing/asserts.ts";
import {
  bodyReader,
  writeTrailers,
  readTrailers,
  parseHTTPVersion,
  readRequest,
  writeResponse,
  parseKeepAlive,
  KeepAlive,
  writeRequest,
  readResponse,
} from "./io.ts";
import { encode, decode } from "../encoding/utf8.ts";
import { BufReader, ReadLineResult } from "../io/bufio.ts";
import { chunkedBodyReader } from "./io.ts";
import { ServerResponse, ServerRequest } from "./server.ts";
import { StringReader, stringReader, multiReader } from "../io/readers.ts";
import { mockConn } from "./testing.ts";
import { ClientRequest } from "./client.ts";
import { TimeoutError, deferred } from "../util/async.ts";
import { readUntilEOF } from "../io/ioutil.ts";
const { Buffer, test, readAll } = Deno;

const kBuf = new Uint8Array(1);
test("[http/io] bodyReader", async () => {
  const text = "Hello, Deno";
  const r = bodyReader(text.length, new BufReader(new Buffer(encode(text))));
  assertEquals(decode(await Deno.readAll(r)), text);
  assertEquals(await r.read(kBuf), null);
});
function chunkify(n: number, char: string): string {
  const v = Array.from({ length: n })
    .map(() => `${char}`)
    .join("");
  return `${n.toString(16)}\r\n${v}\r\n`;
}
test("[http/io] chunkedBodyReader", async () => {
  const body = [
    chunkify(3, "a"),
    chunkify(5, "b"),
    chunkify(11, "c"),
    chunkify(22, "d"),
    chunkify(0, ""),
  ].join("");
  const h = new Headers();
  const r = chunkedBodyReader(h, new BufReader(new Buffer(encode(body))));
  let result: number | null;
  // Use small buffer as some chunks exceed buffer size
  const buf = new Uint8Array(5);
  const dest = new Buffer();
  while ((result = await r.read(buf)) !== null) {
    const len = Math.min(buf.byteLength, result);
    await dest.write(buf.subarray(0, len));
  }
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(new TextDecoder().decode(dest.bytes()), exp);
  assertEquals(await r.read(kBuf), null);
});

test("[http/io] chunkedBodyReader with trailers", async () => {
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
  const r = chunkedBodyReader(h, new BufReader(new Buffer(encode(body))));
  assertEquals(h.has("trailer"), true);
  assertEquals(h.has("deno"), false);
  assertEquals(h.has("node"), false);
  const act = decode(await Deno.readAll(r));
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(act, exp);
  assertEquals(h.has("trailer"), false);
  assertEquals(h.get("deno"), "land");
  assertEquals(h.get("node"), "js");
  assertEquals(await r.read(kBuf), null);
});

test("[http/io] readTrailers", async () => {
  const h = new Headers({
    trailer: "deno,node",
  });
  const trailer = ["deno: land", "node: js", "", ""].join("\r\n");
  await readTrailers(h, new BufReader(new Buffer(encode(trailer))));
  assertEquals(h.has("trailer"), false);
  assertEquals(h.get("deno"), "land");
  assertEquals(h.get("node"), "js");
});

test("[http/io] readTrailer should throw if undeclared headers found in trailer", async () => {
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
        await readTrailers(h, new BufReader(new Buffer(encode(trailer))));
      },
      Error,
      "Undeclared trailer field"
    );
  }
});

test("[http/io] readTrailer should throw if trailer contains prohibited fields", async () => {
  for (const f of ["content-length", "trailer", "transfer-encoding"]) {
    const h = new Headers({
      trailer: f,
    });
    await assertThrowsAsync(
      async () => {
        await readTrailers(h, new BufReader(new Buffer()));
      },
      Error,
      "Prohibited field for trailer"
    );
  }
});

test("[http/io] writeTrailer", async () => {
  const w = new Buffer();
  await writeTrailers(
    w,
    new Headers({ "transfer-encoding": "chunked", trailer: "deno,node" }),
    new Headers({ deno: "land", node: "js" })
  );
  assertEquals(
    new TextDecoder().decode(w.bytes()),
    "deno: land\r\nnode: js\r\n\r\n"
  );
});

test("[http/io] writeTrailer should throw", async () => {
  const w = new Buffer();
  await assertThrowsAsync(
    () => {
      return writeTrailers(w, new Headers(), new Headers());
    },
    Error,
    'must have "trailer"'
  );
  await assertThrowsAsync(
    () => {
      return writeTrailers(w, new Headers({ trailer: "deno" }), new Headers());
    },
    Error,
    "only allowed"
  );
  for (const f of ["content-length", "trailer", "transfer-encoding"]) {
    await assertThrowsAsync(
      () => {
        return writeTrailers(
          w,
          new Headers({ "transfer-encoding": "chunked", trailer: f }),
          new Headers({ [f]: "1" })
        );
      },
      AssertionError,
      "prohibited"
    );
  }
  await assertThrowsAsync(
    () => {
      return writeTrailers(
        w,
        new Headers({ "transfer-encoding": "chunked", trailer: "deno" }),
        new Headers({ node: "js" })
      );
    },
    AssertionError,
    "Not trailer"
  );
});

// Ported from https://github.com/golang/go/blob/f5c43b9/src/net/http/request_test.go#L535-L565
test("[http/io] parseHttpVersion", (): void => {
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

test("[http/io] writeUint8ArrayResponse", async function (): Promise<void> {
  const shortText = "Hello";

  const body = new TextEncoder().encode(shortText);
  const res: ServerResponse = { body };

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

test("[http/io] writeStringResponse", async function (): Promise<void> {
  const body = "Hello";

  const res: ServerResponse = { body };

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

test("[http/io] writeStringReaderResponse", async function (): Promise<void> {
  const shortText = "Hello";

  const body = new StringReader(shortText);
  const res: ServerResponse = { body };

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

test("[http/io] writeResponse with trailer", async () => {
  const w = new Buffer();
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

test("writeResponseShouldNotModifyOriginHeaders", async () => {
  const headers = new Headers();
  const buf = new Deno.Buffer();

  await writeResponse(buf, { body: "foo", headers });
  assert(decode(await readAll(buf)).includes("content-length: 3"));

  await writeResponse(buf, { body: "hello", headers });
  assert(decode(await readAll(buf)).includes("content-length: 5"));
});

test("readRequestError", async function (): Promise<void> {
  const input = `GET / HTTP/1.1
malformedHeader
`;
  const reader = new BufReader(new StringReader(input));
  let err;
  try {
    await readRequest(mockConn(), { r: reader });
  } catch (e) {
    err = e;
  }
  assert(err instanceof Error);
  assertEquals(err.message, "malformed MIME header line: malformedHeader");
});

// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/net/http/request_test.go#L377-L443
// TODO(zekth) fix tests
test("[http/io] testReadRequestError", async function (): Promise<void> {
  const testCases = [
    {
      in: "GET / HTTP/1.1\r\nheader: foo\r\n\r\n",
      headers: [{ key: "header", value: "foo" }],
    },
    {
      in: "GET / HTTP/1.1\r\nheader:foo\r\n",
      err: Deno.errors.UnexpectedEof,
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
      in:
        "POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 0\r\n\r\n" +
        "Gopher hey\r\n",
      err: "cannot contain multiple Content-Length headers",
    },
    {
      in:
        "POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 6\r\n\r\n" +
        "Gopher\r\n",
      err: "cannot contain multiple Content-Length headers",
    },
    {
      in:
        "PUT / HTTP/1.1\r\nContent-Length: 6 \r\nContent-Length: 6\r\n" +
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
      in:
        "POST / HTTP/1.1\r\nContent-Length:0\r\ntransfer-encoding: " +
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
      req = await readRequest(mockConn(), { r: reader });
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
      assert(test.headers);
      assertEquals(err, undefined);
      assertNotEquals(req, null);
      for (const h of test.headers) {
        assertEquals(req.headers.get(h.key), h.value);
      }
    }
  }
});

test({
  name: "[http/io] readRequest read header timeout",
  async fn() {
    const conn = mockConn();
    const d = deferred();
    conn.read = async (_: Uint8Array): Promise<number | null> => {
      await d;
      return null;
    };
    await assertThrowsAsync(async () => {
      await readRequest(conn, { timeout: 100 });
    }, TimeoutError);
    d.resolve();
  },
});

test({
  name: "[http/io] ServerRequest body timeout",
  async fn() {
    const d = deferred();
    const body = {
      async read(_: Uint8Array): Promise<number | null> {
        await d;
        return null;
      },
    };
    const conn = mockConn();
    const head = [
      "POST / HTTP/1.1",
      "host: deno.land",
      "content-length: 20",
      "\r\n",
    ].join("\r\n");
    const r = multiReader(stringReader(head), body);
    conn.read = r.read;
    const req = await readRequest(conn, { timeout: 100 });
    assert(req != null);
    assertEquals(req.headers.get("content-length"), "20");
    assert(req.body != null);
    await assertThrowsAsync(async () => {
      await readUntilEOF(req.body);
    }, TimeoutError);
    d.resolve();
  },
});

const writeRequestCases: Array<{
  title: string;
  exp: string[];
  req: ClientRequest;
  ignore?: boolean;
}> = [
  {
    title: "request_get",
    exp: [
      "GET /index.html?deno=land&msg=gogo HTTP/1.1",
      "content-type: text/plain",
      "host: deno.land",
      "\r\n",
    ],
    req: {
      url: "https://deno.land/index.html?deno=land&msg=gogo",
      method: "GET",
      headers: new Headers({
        "content-type": "text/plain",
      }),
    },
  },
  // FIXME (keroxp)
  {
    title: "request_get_encoded",
    exp: [
      "GET /%F0%9F%A6%96?q=%F0%9F%8E%89 HTTP/1.1",
      "content-type: text/plain",
      "host: deno.land",
      "\r\n",
    ],
    req: {
      url: "https://deno.land/🦖?q=🎉",
      method: "GET",
      headers: new Headers({
        "content-type": "text/plain",
      }),
    },
    // FIXME(keroxp)
    ignore: true,
  },
  {
    title: "request_post",
    exp: [
      "POST /index.html HTTP/1.1",
      "content-type: text/plain",
      "host: deno.land",
      "content-length: 69",
      "",
      "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
    ],
    req: {
      url: "https://deno.land/index.html",
      method: "POST",
      headers: new Headers({
        "content-type": "text/plain",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
    },
  },
  {
    title: "request_post_chunked",
    exp: [
      "POST /index.html HTTP/1.1",
      "content-type: text/plain",
      "transfer-encoding: chunked",
      "host: deno.land",
      "",
      "45",
      "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
      "0",
      "\r\n",
    ],
    req: {
      url: "https://deno.land/index.html",
      method: "POST",
      headers: new Headers({
        "content-type": "text/plain",
        "transfer-encoding": "chunked",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
    },
  },
  {
    title: "request_post_chunked_trailers",
    exp: [
      "POST /index.html HTTP/1.1",
      "content-type: text/plain",
      "transfer-encoding: chunked",
      "trailer: x-deno, x-node",
      "host: deno.land",
      "",
      "45",
      "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
      "0",
      "",
      "x-deno: land",
      "x-node: js",
      "\r\n",
    ],
    req: {
      url: "https://deno.land/index.html",
      method: "POST",
      headers: new Headers({
        "content-type": "text/plain",
        "transfer-encoding": "chunked",
        trailer: "x-deno, x-node",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
      trailers: (): Headers =>
        new Headers({
          "x-deno": "land",
          "x-node": "js",
        }),
    },
  },
];

for (const { title, exp, req, ignore } of writeRequestCases) {
  test({
    name: `[http/io] writeRequest ${title}`,
    ignore,
    async fn() {
      const dest = new Deno.Buffer();
      await writeRequest(dest, req);
      assertEquals(dest.toString(), exp.join("\r\n"));
    },
  });
}

const readResponseCases: Array<[
  string,
  { status: number; headers: Headers; body: string; trailers: Headers }
]> = [
  [
    "response",
    {
      status: 200,
      headers: new Headers({
        "content-type": "text/plain",
        "content-length": "69",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
      trailers: new Headers(),
    },
  ],
  [
    "response_chunked",
    {
      status: 200,
      headers: new Headers({
        "content-type": "text/plain",
        "transfer-encoding": "chunked",
        trailer: "x-deno, x-node",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
      trailers: new Headers({
        "x-deno": "land",
        "x-node": "js",
      }),
    },
  ],
];

for (const [filepath, resp] of readResponseCases) {
  test({
    name: `[http/io] readReponse ${filepath}`,
    async fn() {
      const file = await Deno.open(`http/testdata/${filepath}.txt`);
      const act = await readResponse(file);
      assertEquals(act.status, resp.status);
      for (const [k, v] of resp.headers) {
        assertEquals(act.headers.get(k), v);
      }
      assertEquals(decode(await Deno.readAll(act.body)), resp.body);
      // await act.finalize();
      // console.log([...act.headers.entries()])
      for (const [k, v] of resp.trailers) {
        assertEquals(act.headers.get(k), v);
      }
      file.close();
    },
  });
}

test({
  name: `[http/io] readResponse body timeout`,
  async fn() {
    const head = ["HTTP/1.1 200 OK", "content-length: 20", "\r\n"].join("\r\n");
    const d = deferred();
    const body = {
      async read(_: Uint8Array): Promise<number | null> {
        await d;
        return null;
      },
    };
    const r = multiReader(stringReader(head), body);
    const resp = await readResponse(r, { timeout: 100 });
    assertEquals(resp.headers.get("content-length"), "20");
    await assertThrowsAsync(async () => {
      await readUntilEOF(resp.body);
    }, TimeoutError);
    d.resolve();
  },
});

test({
  name: "[http/io] parseKeepAlive",
  fn() {
    const cases: Array<[string, KeepAlive]> = [
      ["timeout=1, max=1", { timeout: 1, max: 1 }],
      ["timeout=1", { timeout: 1 }],
      ["max=1", { max: 1 }],
      ["", {}],
    ];
    for (const [value, result] of cases) {
      assertEquals(parseKeepAlive(value), result);
    }
  },
});
