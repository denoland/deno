import {
  AssertionError,
  assertThrowsAsync,
  assertEquals,
  assert,
  assertNotEOF,
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
import { encode, decode } from "../strings/mod.ts";
import { BufReader, ReadLineResult } from "../io/bufio.ts";
import { chunkedBodyReader } from "./io.ts";
import { ServerResponse, ServerRequest } from "./server.ts";
import { StringReader } from "../io/readers.ts";
import { mockConn } from "./testing.ts";
import { ClientRequest } from "./client.ts";
const { Buffer, test } = Deno;

const kBuf = new Uint8Array(1);
test("[http/io] bodyReader", async () => {
  const text = "Hello, Deno";
  const r = bodyReader(text.length, new BufReader(new Buffer(encode(text))));
  assertEquals(decode(await Deno.readAll(r)), text);
  assertEquals(await r.read(kBuf), Deno.EOF);
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
  let result: number | Deno.EOF;
  // Use small buffer as some chunks exceed buffer size
  const buf = new Uint8Array(5);
  const dest = new Buffer();
  while ((result = await r.read(buf)) !== Deno.EOF) {
    const len = Math.min(buf.byteLength, result);
    await dest.write(buf.subarray(0, len));
  }
  const exp = "aaabbbbbcccccccccccdddddddddddddddddddddd";
  assertEquals(dest.toString(), exp);
  assertEquals(await r.read(kBuf), Deno.EOF);
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
  assertEquals(await r.read(kBuf), Deno.EOF);
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
  assertEquals(w.toString(), "deno: land\r\nnode: js\r\n\r\n");
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

  let r: ReadLineResult;
  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), "HTTP/1.1 200 OK");
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), `content-length: ${shortText.length}`);
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(r.line.byteLength, 0);
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), shortText);
  assertEquals(r.more, false);

  const eof = await reader.readLine();
  assertEquals(eof, Deno.EOF);
});

test("[http/io] writeStringResponse", async function (): Promise<void> {
  const body = "Hello";

  const res: ServerResponse = { body };

  const buf = new Deno.Buffer();
  await writeResponse(buf, res);

  const decoder = new TextDecoder("utf-8");
  const reader = new BufReader(buf);

  let r: ReadLineResult;
  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), "HTTP/1.1 200 OK");
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), `content-length: ${body.length}`);
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(r.line.byteLength, 0);
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), body);
  assertEquals(r.more, false);

  const eof = await reader.readLine();
  assertEquals(eof, Deno.EOF);
});

test("[http/io] writeStringReaderResponse", async function (): Promise<void> {
  const shortText = "Hello";

  const body = new StringReader(shortText);
  const res: ServerResponse = { body };

  const buf = new Deno.Buffer();
  await writeResponse(buf, res);

  const decoder = new TextDecoder("utf-8");
  const reader = new BufReader(buf);

  let r: ReadLineResult;
  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), "HTTP/1.1 200 OK");
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), "transfer-encoding: chunked");
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(r.line.byteLength, 0);
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), shortText.length.toString());
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
  assertEquals(decoder.decode(r.line), shortText);
  assertEquals(r.more, false);

  r = assertNotEOF(await reader.readLine());
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
  const ret = w.toString();
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

test("[http/io] readRequestError", async function (): Promise<void> {
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
    { in: "", err: Deno.EOF },
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
    let req: ServerRequest | Deno.EOF | undefined;
    try {
      req = await readRequest(mockConn(), { r: reader });
    } catch (e) {
      err = e;
    }
    if (test.err === Deno.EOF) {
      assertEquals(req, Deno.EOF);
    } else if (typeof test.err === "string") {
      assertEquals(err.message, test.err);
    } else if (test.err) {
      assert(err instanceof (test.err as typeof Deno.errors.UnexpectedEof));
    } else {
      assert(req instanceof ServerRequest);
      assert(test.headers);
      assertEquals(err, undefined);
      assertNotEquals(req, Deno.EOF);
      for (const h of test.headers) {
        assertEquals(req.headers.get(h.key), h.value);
      }
    }
  }
});

const writeRequestCases: Array<[string, ClientRequest]> = [
  [
    "request_get",
    {
      url: "https://deno.land/index.html?deno=land&msg=gogo",
      method: "GET",
      headers: new Headers({
        "content-type": "text/plain",
      }),
    },
  ],
  [
    "request_get_capital",
    {
      url: "https://deno.land/About/Index.html?deno=land&msg=gogo",
      method: "GET",
      headers: new Headers({
        "content-type": "text/plain",
      }),
    },
  ],
  // ["request_get_encoded", {
  //   url: "https://deno.land/でのくに?deno=🦕",
  //   method: "GET",
  //   headers: new Headers({
  //     "content-type": "text/plain"
  //   })
  // }],
  [
    "request_post",
    {
      url: "https://deno.land/index.html",
      method: "POST",
      headers: new Headers({
        "content-type": "text/plain",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
    },
  ],
  [
    "request_post_chunked",
    {
      url: "https://deno.land/index.html",
      method: "POST",
      headers: new Headers({
        "content-type": "text/plain",
        "transfer-encoding": "chunked",
      }),
      body:
        "A secure JavaScript/TypeScript runtime built with V8, Rust, and Tokio",
    },
  ],
  [
    "request_post_chunked_trailers",
    {
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
  ],
];

for (const [file, req] of writeRequestCases) {
  test({
    name: `[http/io] writeRequest ${file}`,
    async fn() {
      const dest = new Deno.Buffer();
      await writeRequest(dest, req);
      const exp = decode(await Deno.readFile(`http/testdata/${file}.txt`));
      assertEquals(dest.toString(), exp);
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
