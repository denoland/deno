// Ported to Deno from:
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import * as deno from "deno";
import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import { BufReader, BufState } from "./bufio.ts";
import { Buffer } from "./buffer.ts";
import * as iotest from "./iotest.ts";
import { charCode } from "./util.ts";

async function readBytes(buf: BufReader): Promise<string> {
  const b = new Uint8Array(1000);
  let nb = 0;
  while (true) {
    let c = await buf.readByte();
    if (c < 0) {
      break; // EOF
    }
    b[nb] = c;
    nb++;
  }
  const decoder = new TextDecoder();
  return decoder.decode(b.subarray(0, nb));
}

function stringsReader(s: string): deno.Reader {
  const encoder = new TextEncoder();
  const ui8 = encoder.encode(s);
  return new Buffer(ui8.buffer as ArrayBuffer);
}

test(async function bufioReaderSimple() {
  const data = "hello world";
  const b = new BufReader(stringsReader(data));
  const s = await readBytes(b);
  assertEqual(s, data);
});

type ReadMaker = { name: string; fn: (r: deno.Reader) => deno.Reader };

const readMakers: ReadMaker[] = [
  { name: "full", fn: r => r },
  { name: "byte", fn: r => new iotest.OneByteReader(r) },
  { name: "half", fn: r => new iotest.HalfReader(r) }
  // TODO { name: "data+err", r => new iotest.DataErrReader(r) },
  // { name: "timeout", fn: r => new iotest.TimeoutReader(r) },
];

function readLines(b: BufReader): string {
  let s = "";
  while (true) {
    let s1 = b.readString("\n");
    if (s1 == null) {
      break; // EOF
    }
    s += s1;
  }
  return s;
}

// Call read to accumulate the text of a file
async function reads(buf: BufReader, m: number): Promise<string> {
  const b = new Uint8Array(1000);
  let nb = 0;
  while (true) {
    const { nread, eof } = await buf.read(b.subarray(nb, nb + m));
    nb += nread;
    if (eof) {
      break;
    }
  }
  const decoder = new TextDecoder();
  return decoder.decode(b.subarray(0, nb));
}

type NamedBufReader = { name: string; fn: (r: BufReader) => Promise<string> };

const bufreaders: NamedBufReader[] = [
  { name: "1", fn: (b: BufReader) => reads(b, 1) },
  { name: "2", fn: (b: BufReader) => reads(b, 2) },
  { name: "3", fn: (b: BufReader) => reads(b, 3) },
  { name: "4", fn: (b: BufReader) => reads(b, 4) },
  { name: "5", fn: (b: BufReader) => reads(b, 5) },
  { name: "7", fn: (b: BufReader) => reads(b, 7) },
  { name: "bytes", fn: readBytes }
  // { name: "lines", fn: readLines },
];

const MIN_READ_BUFFER_SIZE = 16;
const bufsizes: number[] = [
  0,
  MIN_READ_BUFFER_SIZE,
  23,
  32,
  46,
  64,
  93,
  128,
  1024,
  4096
];

test(async function bufioBufReader() {
  const texts = new Array<string>(31);
  let str = "";
  let all = "";
  for (let i = 0; i < texts.length - 1; i++) {
    texts[i] = str + "\n";
    all += texts[i];
    str += String.fromCharCode(i % 26 + 97);
  }
  texts[texts.length - 1] = all;

  for (let text of texts) {
    for (let readmaker of readMakers) {
      for (let bufreader of bufreaders) {
        for (let bufsize of bufsizes) {
          const read = readmaker.fn(stringsReader(text));
          const buf = new BufReader(read, bufsize);
          const s = await bufreader.fn(buf);
          const debugStr =
            `reader=${readmaker.name} ` +
            `fn=${bufreader.name} bufsize=${bufsize} want=${text} got=${s}`;
          assertEqual(s, text, debugStr);
        }
      }
    }
  }
});

test(async function bufioBufferFull() {
  const longString =
    "And now, hello, world! It is the time for all good men to come to the aid of their party";
  const buf = new BufReader(stringsReader(longString), MIN_READ_BUFFER_SIZE);
  let [line, state] = await buf.readSlice(charCode("!"));

  const decoder = new TextDecoder();
  let actual = decoder.decode(line);
  assertEqual(state, BufState.BufferFull);
  assertEqual(actual, "And now, hello, ");

  [line, state] = await buf.readSlice(charCode("!"));
  actual = decoder.decode(line);
  assertEqual(actual, "world!");
  assertEqual(state, BufState.Ok);
});
