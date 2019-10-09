// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

// Ported from
// https://github.com/golang/go/blob/master/src/net/http/responsewrite_test.go

const { Buffer } = Deno;
import { TextProtoReader } from "../textproto/mod.ts";
import { test, runIfMain } from "../testing/mod.ts";
import { assert, assertEquals, assertNotEquals } from "../testing/asserts.ts";
import {
  Response,
  ServerRequest,
  writeResponse,
  readRequest,
  parseHTTPVersion
} from "./server.ts";
import { delay } from "../util/async.ts";
import {
  BufReader,
  BufWriter,
  ReadLineResult,
  UnexpectedEOFError
} from "../io/bufio.ts";
import { StringReader } from "../io/readers.ts";

function assertNotEOF<T extends {}>(val: T | Deno.EOF): T {
  assertNotEquals(val, Deno.EOF);
  return val as T;
}

interface ResponseTest {
  response: Response;
  raw: string;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

type Handler = () => void;

const responseTests: ResponseTest[] = [
  // Default response
  {
    response: {},
    raw: "HTTP/1.1 200 OK\r\n" + "content-length: 0" + "\r\n\r\n"
  },
  // Empty body with status
  {
    response: {
      status: 404
    },
    raw: "HTTP/1.1 404 Not Found\r\n" + "content-length: 0" + "\r\n\r\n"
  },
  // HTTP/1.1, chunked coding; empty trailer; close
  {
    response: {
      status: 200,
      body: new Buffer(new TextEncoder().encode("abcdef"))
    },

    raw:
      "HTTP/1.1 200 OK\r\n" +
      "transfer-encoding: chunked\r\n\r\n" +
      "6\r\nabcdef\r\n0\r\n\r\n"
  }
];

test(async function responseWrite(): Promise<void> {
  for (const testCase of responseTests) {
    const buf = new Buffer();
    const bufw = new BufWriter(buf);
    const request = new ServerRequest();
    request.w = bufw;

    request.conn = {
      localAddr: "",
      remoteAddr: "",
      rid: -1,
      closeRead: (): void => {},
      closeWrite: (): void => {},
      read: async (): Promise<number | Deno.EOF> => {
        return 0;
      },
      write: async (): Promise<number> => {
        return -1;
      },
      close: (): void => {}
    };

    await request.respond(testCase.response);
    assertEquals(buf.toString(), testCase.raw);
    await request.done;
  }
});

test(async function requestBodyWithContentLength(): Promise<void> {
  {
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("content-length", "5");
    const buf = new Buffer(enc.encode("Hello"));
    req.r = new BufReader(buf);
    const body = dec.decode(await req.body());
    assertEquals(body, "Hello");
  }

  // Larger than internal buf
  {
    const longText = "1234\n".repeat(1000);
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("Content-Length", "5000");
    const buf = new Buffer(enc.encode(longText));
    req.r = new BufReader(buf);
    const body = dec.decode(await req.body());
    assertEquals(body, longText);
  }
});

test(async function requestBodyWithTransferEncoding(): Promise<void> {
  {
    const shortText = "Hello";
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("transfer-encoding", "chunked");
    let chunksData = "";
    let chunkOffset = 0;
    const maxChunkSize = 70;
    while (chunkOffset < shortText.length) {
      const chunkSize = Math.min(maxChunkSize, shortText.length - chunkOffset);
      chunksData += `${chunkSize.toString(16)}\r\n${shortText.substr(
        chunkOffset,
        chunkSize
      )}\r\n`;
      chunkOffset += chunkSize;
    }
    chunksData += "0\r\n\r\n";
    const buf = new Buffer(enc.encode(chunksData));
    req.r = new BufReader(buf);
    const body = dec.decode(await req.body());
    assertEquals(body, shortText);
  }

  // Larger than internal buf
  {
    const longText = "1234\n".repeat(1000);
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("transfer-encoding", "chunked");
    let chunksData = "";
    let chunkOffset = 0;
    const maxChunkSize = 70;
    while (chunkOffset < longText.length) {
      const chunkSize = Math.min(maxChunkSize, longText.length - chunkOffset);
      chunksData += `${chunkSize.toString(16)}\r\n${longText.substr(
        chunkOffset,
        chunkSize
      )}\r\n`;
      chunkOffset += chunkSize;
    }
    chunksData += "0\r\n\r\n";
    const buf = new Buffer(enc.encode(chunksData));
    req.r = new BufReader(buf);
    const body = dec.decode(await req.body());
    assertEquals(body, longText);
  }
});

test(async function requestBodyStreamWithContentLength(): Promise<void> {
  {
    const shortText = "Hello";
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("content-length", "" + shortText.length);
    const buf = new Buffer(enc.encode(shortText));
    req.r = new BufReader(buf);
    const it = await req.bodyStream();
    let offset = 0;
    for await (const chunk of it) {
      const s = dec.decode(chunk);
      assertEquals(shortText.substr(offset, s.length), s);
      offset += s.length;
    }
  }

  // Larger than internal buf
  {
    const longText = "1234\n".repeat(1000);
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("Content-Length", "5000");
    const buf = new Buffer(enc.encode(longText));
    req.r = new BufReader(buf);
    const it = await req.bodyStream();
    let offset = 0;
    for await (const chunk of it) {
      const s = dec.decode(chunk);
      assertEquals(longText.substr(offset, s.length), s);
      offset += s.length;
    }
  }
});

test(async function requestBodyStreamWithTransferEncoding(): Promise<void> {
  {
    const shortText = "Hello";
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("transfer-encoding", "chunked");
    let chunksData = "";
    let chunkOffset = 0;
    const maxChunkSize = 70;
    while (chunkOffset < shortText.length) {
      const chunkSize = Math.min(maxChunkSize, shortText.length - chunkOffset);
      chunksData += `${chunkSize.toString(16)}\r\n${shortText.substr(
        chunkOffset,
        chunkSize
      )}\r\n`;
      chunkOffset += chunkSize;
    }
    chunksData += "0\r\n\r\n";
    const buf = new Buffer(enc.encode(chunksData));
    req.r = new BufReader(buf);
    const it = await req.bodyStream();
    let offset = 0;
    for await (const chunk of it) {
      const s = dec.decode(chunk);
      assertEquals(shortText.substr(offset, s.length), s);
      offset += s.length;
    }
  }

  // Larger than internal buf
  {
    const longText = "1234\n".repeat(1000);
    const req = new ServerRequest();
    req.headers = new Headers();
    req.headers.set("transfer-encoding", "chunked");
    let chunksData = "";
    let chunkOffset = 0;
    const maxChunkSize = 70;
    while (chunkOffset < longText.length) {
      const chunkSize = Math.min(maxChunkSize, longText.length - chunkOffset);
      chunksData += `${chunkSize.toString(16)}\r\n${longText.substr(
        chunkOffset,
        chunkSize
      )}\r\n`;
      chunkOffset += chunkSize;
    }
    chunksData += "0\r\n\r\n";
    const buf = new Buffer(enc.encode(chunksData));
    req.r = new BufReader(buf);
    const it = await req.bodyStream();
    let offset = 0;
    for await (const chunk of it) {
      const s = dec.decode(chunk);
      assertEquals(longText.substr(offset, s.length), s);
      offset += s.length;
    }
  }
});

test(async function writeUint8ArrayResponse(): Promise<void> {
  const shortText = "Hello";

  const body = new TextEncoder().encode(shortText);
  const res: Response = { body };

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

test(async function writeStringReaderResponse(): Promise<void> {
  const shortText = "Hello";

  const body = new StringReader(shortText);
  const res: Response = { body };

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

const mockConn = {
  localAddr: "",
  remoteAddr: "",
  rid: -1,
  closeRead: (): void => {},
  closeWrite: (): void => {},
  read: async (): Promise<number | Deno.EOF> => {
    return 0;
  },
  write: async (): Promise<number> => {
    return -1;
  },
  close: (): void => {}
};

test(async function readRequestError(): Promise<void> {
  const input = `GET / HTTP/1.1
malformedHeader
`;
  const reader = new BufReader(new StringReader(input));
  let err;
  try {
    await readRequest(mockConn, reader);
  } catch (e) {
    err = e;
  }
  assert(err instanceof Error);
  assertEquals(err.message, "malformed MIME header line: malformedHeader");
});

// Ported from Go
// https://github.com/golang/go/blob/go1.12.5/src/net/http/request_test.go#L377-L443
// TODO(zekth) fix tests
test(async function testReadRequestError(): Promise<void> {
  const testCases = [
    {
      in: "GET / HTTP/1.1\r\nheader: foo\r\n\r\n",
      headers: [{ key: "header", value: "foo" }]
    },
    {
      in: "GET / HTTP/1.1\r\nheader:foo\r\n",
      err: UnexpectedEOFError
    },
    { in: "", err: Deno.EOF },
    {
      in: "HEAD / HTTP/1.1\r\nContent-Length:4\r\n\r\n",
      err: "http: method cannot contain a Content-Length"
    },
    {
      in: "HEAD / HTTP/1.1\r\n\r\n",
      headers: []
    },
    // Multiple Content-Length values should either be
    // deduplicated if same or reject otherwise
    // See Issue 16490.
    {
      in:
        "POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 0\r\n\r\n" +
        "Gopher hey\r\n",
      err: "cannot contain multiple Content-Length headers"
    },
    {
      in:
        "POST / HTTP/1.1\r\nContent-Length: 10\r\nContent-Length: 6\r\n\r\n" +
        "Gopher\r\n",
      err: "cannot contain multiple Content-Length headers"
    },
    {
      in:
        "PUT / HTTP/1.1\r\nContent-Length: 6 \r\nContent-Length: 6\r\n" +
        "Content-Length:6\r\n\r\nGopher\r\n",
      headers: [{ key: "Content-Length", value: "6" }]
    },
    {
      in: "PUT / HTTP/1.1\r\nContent-Length: 1\r\nContent-Length: 6 \r\n\r\n",
      err: "cannot contain multiple Content-Length headers"
    },
    // Setting an empty header is swallowed by textproto
    // see: readMIMEHeader()
    // {
    //   in: "POST / HTTP/1.1\r\nContent-Length:\r\nContent-Length: 3\r\n\r\n",
    //   err: "cannot contain multiple Content-Length headers"
    // },
    {
      in: "HEAD / HTTP/1.1\r\nContent-Length:0\r\nContent-Length: 0\r\n\r\n",
      headers: [{ key: "Content-Length", value: "0" }]
    },
    {
      in:
        "POST / HTTP/1.1\r\nContent-Length:0\r\ntransfer-encoding: " +
        "chunked\r\n\r\n",
      headers: [],
      err: "http: Transfer-Encoding and Content-Length cannot be send together"
    }
  ];
  for (const test of testCases) {
    const reader = new BufReader(new StringReader(test.in));
    let err;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    let req: any;
    try {
      req = await readRequest(mockConn, reader);
    } catch (e) {
      err = e;
    }
    if (test.err === Deno.EOF) {
      assertEquals(req, Deno.EOF);
    } else if (typeof test.err === "string") {
      assertEquals(err.message, test.err);
    } else if (test.err) {
      assert(err instanceof (test.err as typeof UnexpectedEOFError));
    } else {
      assertEquals(err, undefined);
      assertNotEquals(req, Deno.EOF);
      for (const h of test.headers!) {
        assertEquals((req! as ServerRequest).headers.get(h.key), h.value);
      }
    }
  }
});

// Ported from https://github.com/golang/go/blob/f5c43b9/src/net/http/request_test.go#L535-L565
test({
  name: "[http] parseHttpVersion",
  fn(): void {
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
      { in: "HTTP/1,0", err: true }
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
  }
});

test({
  name: "[http] destroyed connection",
  async fn(): Promise<void> {
    // Runs a simple server as another process
    const p = Deno.run({
      args: [Deno.execPath(), "http/testdata/simple_server.ts", "--allow-net"],
      stdout: "piped"
    });

    try {
      const r = new TextProtoReader(new BufReader(p.stdout!));
      const s = await r.readLine();
      assert(s !== Deno.EOF && s.includes("server listening"));

      let serverIsRunning = true;
      p.status()
        .then((): void => {
          serverIsRunning = false;
        })
        .catch((_): void => {}); // Ignores the error when closing the process.

      await delay(100);

      // Reqeusts to the server and immediately closes the connection
      const conn = await Deno.dial({ port: 4502 });
      await conn.write(new TextEncoder().encode("GET / HTTP/1.0\n\n"));
      conn.close();

      // Waits for the server to handle the above (broken) request
      await delay(100);

      assert(serverIsRunning);
    } finally {
      // Stops the sever.
      p.close();
    }
  }
});

runIfMain(import.meta);
