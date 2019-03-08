// Based on https://github.com/golang/go/blob/891682/src/bufio/bufio_test.go
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

const { Buffer } = Deno;
type Reader = Deno.Reader;
type ReadResult = Deno.ReadResult;
import { test } from "../testing/mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { BufReader, BufWriter } from "./bufio.ts";
import * as iotest from "./iotest.ts";
import { charCode, copyBytes, stringsReader } from "./util.ts";

const encoder = new TextEncoder();

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

test(async function bufioReaderSimple() {
  const data = "hello world";
  const b = new BufReader(stringsReader(data));
  const s = await readBytes(b);
  assertEquals(s, data);
});

interface ReadMaker {
  name: string;
  fn: (r: Reader) => Reader;
}

const readMakers: ReadMaker[] = [
  { name: "full", fn: r => r },
  { name: "byte", fn: r => new iotest.OneByteReader(r) },
  { name: "half", fn: r => new iotest.HalfReader(r) }
  // TODO { name: "data+err", r => new iotest.DataErrReader(r) },
  // { name: "timeout", fn: r => new iotest.TimeoutReader(r) },
];

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

interface NamedBufReader {
  name: string;
  fn: (r: BufReader) => Promise<string>;
}

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
    str += String.fromCharCode((i % 26) + 97);
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
          assertEquals(s, text, debugStr);
        }
      }
    }
  }
});

test(async function bufioBufferFull() {
  const longString =
    "And now, hello, world! It is the time for all good men to come to the aid of their party";
  const buf = new BufReader(stringsReader(longString), MIN_READ_BUFFER_SIZE);
  let [line, err] = await buf.readSlice(charCode("!"));

  const decoder = new TextDecoder();
  let actual = decoder.decode(line);
  assertEquals(err, "BufferFull");
  assertEquals(actual, "And now, hello, ");

  [line, err] = await buf.readSlice(charCode("!"));
  actual = decoder.decode(line);
  assertEquals(actual, "world!");
  assert(err == null);
});

const testInput = encoder.encode(
  "012\n345\n678\n9ab\ncde\nfgh\nijk\nlmn\nopq\nrst\nuvw\nxy"
);
const testInputrn = encoder.encode(
  "012\r\n345\r\n678\r\n9ab\r\ncde\r\nfgh\r\nijk\r\nlmn\r\nopq\r\nrst\r\nuvw\r\nxy\r\n\n\r\n"
);
const testOutput = encoder.encode("0123456789abcdefghijklmnopqrstuvwxy");

// TestReader wraps a Uint8Array and returns reads of a specific length.
class TestReader implements Reader {
  constructor(private data: Uint8Array, private stride: number) {}

  async read(buf: Uint8Array): Promise<ReadResult> {
    let nread = this.stride;
    if (nread > this.data.byteLength) {
      nread = this.data.byteLength;
    }
    if (nread > buf.byteLength) {
      nread = buf.byteLength;
    }
    copyBytes(buf as Uint8Array, this.data);
    this.data = this.data.subarray(nread);
    let eof = false;
    if (this.data.byteLength == 0) {
      eof = true;
    }
    return { nread, eof };
  }
}

async function testReadLine(input: Uint8Array): Promise<void> {
  for (let stride = 1; stride < 2; stride++) {
    let done = 0;
    let reader = new TestReader(input, stride);
    let l = new BufReader(reader, input.byteLength + 1);
    while (true) {
      let [line, isPrefix, err] = await l.readLine();
      if (line.byteLength > 0 && err != null) {
        throw Error("readLine returned both data and error");
      }
      assertEquals(isPrefix, false);
      if (err == "EOF") {
        break;
      }
      // eslint-disable-next-line @typescript-eslint/restrict-plus-operands
      let want = testOutput.subarray(done, done + line.byteLength);
      assertEquals(
        line,
        want,
        `Bad line at stride ${stride}: want: ${want} got: ${line}`
      );
      done += line.byteLength;
    }
    assertEquals(
      done,
      testOutput.byteLength,
      `readLine didn't return everything: got: ${done}, ` +
        `want: ${testOutput} (stride: ${stride})`
    );
  }
}

test(async function bufioReadLine() {
  await testReadLine(testInput);
  await testReadLine(testInputrn);
});

test(async function bufioPeek() {
  const decoder = new TextDecoder();
  let p = new Uint8Array(10);
  // string is 16 (minReadBufferSize) long.
  let buf = new BufReader(
    stringsReader("abcdefghijklmnop"),
    MIN_READ_BUFFER_SIZE
  );

  let [actual, err] = await buf.peek(1);
  assertEquals(decoder.decode(actual), "a");
  assert(err == null);

  [actual, err] = await buf.peek(4);
  assertEquals(decoder.decode(actual), "abcd");
  assert(err == null);

  [actual, err] = await buf.peek(32);
  assertEquals(decoder.decode(actual), "abcdefghijklmnop");
  assertEquals(err, "BufferFull");

  await buf.read(p.subarray(0, 3));
  assertEquals(decoder.decode(p.subarray(0, 3)), "abc");

  [actual, err] = await buf.peek(1);
  assertEquals(decoder.decode(actual), "d");
  assert(err == null);

  [actual, err] = await buf.peek(1);
  assertEquals(decoder.decode(actual), "d");
  assert(err == null);

  [actual, err] = await buf.peek(1);
  assertEquals(decoder.decode(actual), "d");
  assert(err == null);

  [actual, err] = await buf.peek(2);
  assertEquals(decoder.decode(actual), "de");
  assert(err == null);

  let { eof } = await buf.read(p.subarray(0, 3));
  assertEquals(decoder.decode(p.subarray(0, 3)), "def");
  assert(!eof);
  assert(err == null);

  [actual, err] = await buf.peek(4);
  assertEquals(decoder.decode(actual), "ghij");
  assert(err == null);

  await buf.read(p);
  assertEquals(decoder.decode(p), "ghijklmnop");

  [actual, err] = await buf.peek(0);
  assertEquals(decoder.decode(actual), "");
  assert(err == null);

  [actual, err] = await buf.peek(1);
  assertEquals(decoder.decode(actual), "");
  assert(err == "EOF");
  /* TODO
	// Test for issue 3022, not exposing a reader's error on a successful Peek.
	buf = NewReaderSize(dataAndEOFReader("abcd"), 32)
	if s, err := buf.Peek(2); string(s) != "ab" || err != nil {
		t.Errorf(`Peek(2) on "abcd", EOF = %q, %v; want "ab", nil`, string(s), err)
	}
	if s, err := buf.Peek(4); string(s) != "abcd" || err != nil {
		t.Errorf(`Peek(4) on "abcd", EOF = %q, %v; want "abcd", nil`, string(s), err)
	}
	if n, err := buf.Read(p[0:5]); string(p[0:n]) != "abcd" || err != nil {
		t.Fatalf("Read after peek = %q, %v; want abcd, EOF", p[0:n], err)
	}
	if n, err := buf.Read(p[0:1]); string(p[0:n]) != "" || err != io.EOF {
		t.Fatalf(`second Read after peek = %q, %v; want "", EOF`, p[0:n], err)
	}
  */
});

test(async function bufioWriter() {
  const data = new Uint8Array(8192);

  for (let i = 0; i < data.byteLength; i++) {
    // eslint-disable-next-line @typescript-eslint/restrict-plus-operands
    data[i] = charCode(" ") + (i % (charCode("~") - charCode(" ")));
  }

  const w = new Buffer();
  for (let nwrite of bufsizes) {
    for (let bs of bufsizes) {
      // Write nwrite bytes using buffer size bs.
      // Check that the right amount makes it out
      // and that the data is correct.

      w.reset();
      const buf = new BufWriter(w, bs);

      const context = `nwrite=${nwrite} bufsize=${bs}`;
      const n = await buf.write(data.subarray(0, nwrite));
      assertEquals(n, nwrite, context);

      await buf.flush();

      const written = w.bytes();
      assertEquals(written.byteLength, nwrite);

      for (let l = 0; l < written.byteLength; l++) {
        assertEquals(written[l], data[l]);
      }
    }
  }
});

test(async function bufReaderReadFull() {
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const text = "Hello World";
  const data = new Buffer(enc.encode(text));
  const bufr = new BufReader(data, 3);
  {
    const buf = new Uint8Array(6);
    const [nread, err] = await bufr.readFull(buf);
    assertEquals(nread, 6);
    assert(!err);
    assertEquals(dec.decode(buf), "Hello ");
  }
  {
    const buf = new Uint8Array(6);
    const [nread, err] = await bufr.readFull(buf);
    assertEquals(nread, 5);
    assertEquals(err, "EOF");
    assertEquals(dec.decode(buf.subarray(0, 5)), "World");
  }
});
