// Copyright 2010 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

// Ported from
// https://github.com/golang/go/blob/master/src/net/http/responsewrite_test.go

import {
  test,
  assert,
  assertEqual
} from "https://deno.land/x/testing/testing.ts";

import {
  listenAndServe,
  ServerRequest,
  setContentLength,
  Response
} from "./http";
import { Buffer } from "deno";
import { BufWriter, BufReader } from "./bufio";

interface ResponseTest {
  response: Response;
  raw: string;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

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
    request.w = bufw;

    await request.respond(testCase.response);
    assertEqual(buf.toString(), testCase.raw);
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
    assertEqual(body, "Hello");
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
    assertEqual(body, longText);
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
    assertEqual(body, shortText);
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
    assertEqual(body, longText);
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
      assertEqual(shortText.substr(offset, s.length), s);
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
      assertEqual(longText.substr(offset, s.length), s);
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
      assertEqual(shortText.substr(offset, s.length), s);
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
      assertEqual(longText.substr(offset, s.length), s);
      offset += s.length;
    }
  }
});
