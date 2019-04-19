// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

// Ported from
// https://github.com/golang/go/blob/master/src/net/http/responsewrite_test.go

const { Buffer } = Deno;
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { Response, ServerRequest } from "./server.ts";
import { BufReader, BufWriter } from "../io/bufio.ts";

interface ResponseTest {
  response: Response;
  raw: string;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

type Handler = () => void;

interface Deferred {
  promise: Promise<{}>;
  resolve: Handler;
  reject: Handler;
}

function deferred(isResolved = false): Deferred {
  let resolve: Handler = () => void 0;
  let reject: Handler = () => void 0;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  if (isResolved) {
    resolve();
  }
  return {
    promise,
    resolve,
    reject
  };
}

const responseTests: ResponseTest[] = [
  // Default response
  {
    response: {},
    raw: "HTTP/1.1 200 OK\r\n" + "\r\n"
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

test(async function responseWrite() {
  for (const testCase of responseTests) {
    const buf = new Buffer();
    const bufw = new BufWriter(buf);
    const request = new ServerRequest();
    request.pipelineId = 1;
    request.w = bufw;
    request.conn = {
      localAddr: "",
      remoteAddr: "",
      rid: -1,
      closeRead: () => {},
      closeWrite: () => {},
      read: async () => {
        return { eof: true, nread: 0 };
      },
      write: async () => {
        return -1;
      },
      close: () => {},
      lastPipelineId: 0,
      pendingDeferredMap: new Map([[0, deferred(true)], [1, deferred()]])
    };

    await request.respond(testCase.response);
    assertEquals(buf.toString(), testCase.raw);
  }
});

test(async function requestBodyWithContentLength() {
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

test(async function requestBodyWithTransferEncoding() {
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

test(async function requestBodyStreamWithContentLength() {
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

test(async function requestBodyStreamWithTransferEncoding() {
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
