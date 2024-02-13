// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This code has been ported almost directly from Go's src/bytes/buffer_test.go
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
import { assert, assertEquals, assertRejects, fail } from "../assert/mod.ts";
import { BufferFullError, BufReader, PartialReadError } from "./buf_reader.ts";
import { StringReader } from "./string_reader.ts";
import { bufsizes, MIN_READ_BUFFER_SIZE } from "./_test_common.ts";
import { Buffer } from "./buffer.ts";
import type { Reader } from "../types.d.ts";
import { copy } from "../bytes/copy.ts";

/** OneByteReader returns a Reader that implements
 * each non-empty Read by reading one byte from r.
 */
class OneByteReader implements Reader {
  constructor(readonly r: Reader) {}

  read(p: Uint8Array): Promise<number | null> {
    if (p.byteLength === 0) {
      return Promise.resolve(0);
    }
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    return Promise.resolve(this.r.read(p.subarray(0, 1)));
  }
}

/** HalfReader returns a Reader that implements Read
 * by reading half as many requested bytes from r.
 */
class HalfReader implements Reader {
  constructor(readonly r: Reader) {}

  read(p: Uint8Array): Promise<number | null> {
    if (!(p instanceof Uint8Array)) {
      throw Error("expected Uint8Array");
    }
    const half = Math.floor((p.byteLength + 1) / 2);
    return Promise.resolve(this.r.read(p.subarray(0, half)));
  }
}

async function readBytes(buf: BufReader): Promise<string> {
  const b = new Uint8Array(1000);
  let nb = 0;
  while (true) {
    const c = await buf.readByte();
    if (c === null) {
      break; // EOF
    }
    b[nb] = c;
    nb++;
  }
  const decoder = new TextDecoder();
  return decoder.decode(b.subarray(0, nb));
}

interface ReadMaker {
  name: string;
  fn: (r: Reader) => Reader;
}

const readMakers: ReadMaker[] = [
  { name: "full", fn: (r): Reader => r },
  {
    name: "byte",
    fn: (r): OneByteReader => new OneByteReader(r),
  },
  { name: "half", fn: (r): HalfReader => new HalfReader(r) },
  // TODO(bartlomieju): { name: "data+err", r => new DataErrReader(r) },
  // { name: "timeout", fn: r => new TimeoutReader(r) },
];

// Call read to accumulate the text of a file
async function reads(buf: BufReader, m: number): Promise<string> {
  const b = new Uint8Array(1000);
  let nb = 0;
  while (true) {
    const result = await buf.read(b.subarray(nb, nb + m));
    if (result === null) {
      break;
    }
    nb += result;
  }
  const decoder = new TextDecoder();
  return decoder.decode(b.subarray(0, nb));
}

interface NamedBufReader {
  name: string;
  fn: (r: BufReader) => Promise<string>;
}

const bufreaders: NamedBufReader[] = [
  { name: "1", fn: (b: BufReader): Promise<string> => reads(b, 1) },
  { name: "2", fn: (b: BufReader): Promise<string> => reads(b, 2) },
  { name: "3", fn: (b: BufReader): Promise<string> => reads(b, 3) },
  { name: "4", fn: (b: BufReader): Promise<string> => reads(b, 4) },
  { name: "5", fn: (b: BufReader): Promise<string> => reads(b, 5) },
  { name: "7", fn: (b: BufReader): Promise<string> => reads(b, 7) },
  { name: "bytes", fn: readBytes },
  // { name: "lines", fn: readLines },
];

Deno.test("bufioReaderSimple", async function () {
  const data = "hello world";
  const b = new BufReader(new StringReader(data));
  const s = await readBytes(b);
  assertEquals(s, data);
});

Deno.test("bufioBufReader", async function () {
  const texts = new Array<string>(31);
  let str = "";
  let all = "";
  for (let i = 0; i < texts.length - 1; i++) {
    texts[i] = str + "\n";
    all += texts[i];
    str += String.fromCharCode((i % 26) + 97);
  }
  texts[texts.length - 1] = all;

  for (const text of texts) {
    for (const readmaker of readMakers) {
      for (const bufreader of bufreaders) {
        for (const bufsize of bufsizes) {
          const read = readmaker.fn(new StringReader(text));
          const buf = new BufReader(read, bufsize);
          const s = await bufreader.fn(buf);
          const debugStr = `reader=${readmaker.name} ` +
            `fn=${bufreader.name} bufsize=${bufsize} want=${text} got=${s}`;
          assertEquals(s, text, debugStr);
        }
      }
    }
  }
});

Deno.test("bufioBufferFull", async function () {
  const longString =
    "And now, hello, world! It is the time for all good men to come to the" +
    " aid of their party";
  const buf = new BufReader(new StringReader(longString), MIN_READ_BUFFER_SIZE);
  const decoder = new TextDecoder();

  try {
    await buf.readSlice("!".charCodeAt(0));
    fail("readSlice should throw");
  } catch (err) {
    assert(err instanceof BufferFullError);
    assert(err.partial instanceof Uint8Array);
    assertEquals(decoder.decode(err.partial), "And now, hello, ");
  }

  const line = await buf.readSlice("!".charCodeAt(0));
  assert(line !== null);
  const actual = decoder.decode(line);
  assertEquals(actual, "world!");
});

Deno.test("bufioReadString", async function () {
  const string = "And now, hello world!";
  const buf = new BufReader(new StringReader(string), MIN_READ_BUFFER_SIZE);

  const line = await buf.readString(",");
  assert(line !== null);
  assertEquals(line, "And now,");
  assertEquals(line.length, 8);

  const line2 = await buf.readString(",");
  assert(line2 !== null);
  assertEquals(line2, " hello world!");

  assertEquals(await buf.readString(","), null);

  try {
    await buf.readString("deno");

    fail("should throw");
  } catch (err) {
    assert(err instanceof Error);
    assert(err.message, "Delimiter should be a single character");
  }
});

Deno.test("bufReaderReadFull", async function () {
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const text = "Hello World";
  const data = new Buffer(enc.encode(text));
  const bufr = new BufReader(data, 3);
  {
    const buf = new Uint8Array(6);
    const r = await bufr.readFull(buf);
    assert(r !== null);
    assertEquals(r, buf);
    assertEquals(dec.decode(buf), "Hello ");
  }
  {
    const buf = new Uint8Array(6);
    try {
      await bufr.readFull(buf);
      fail("readFull() should throw PartialReadError");
    } catch (err) {
      assert(err instanceof PartialReadError);
      assert(err.partial instanceof Uint8Array);
      assertEquals(err.partial.length, 5);
      assertEquals(dec.decode(buf.subarray(0, 5)), "World");
    }
  }
});

Deno.test("bufioPeek", async function () {
  const decoder = new TextDecoder();
  const p = new Uint8Array(10);
  // string is 16 (minReadBufferSize) long.
  const buf = new BufReader(
    new StringReader("abcdefghijklmnop"),
    MIN_READ_BUFFER_SIZE,
  );

  let actual = await buf.peek(1);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "a");

  actual = await buf.peek(4);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "abcd");

  try {
    await buf.peek(32);
    fail("peek() should throw");
  } catch (err) {
    assert(err instanceof BufferFullError);
    assert(err.partial instanceof Uint8Array);
    assertEquals(decoder.decode(err.partial), "abcdefghijklmnop");
  }

  await buf.read(p.subarray(0, 3));
  assertEquals(decoder.decode(p.subarray(0, 3)), "abc");

  actual = await buf.peek(1);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "d");

  actual = await buf.peek(1);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "d");

  actual = await buf.peek(1);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "d");

  actual = await buf.peek(2);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "de");

  const res = await buf.read(p.subarray(0, 3));
  assertEquals(decoder.decode(p.subarray(0, 3)), "def");
  assert(res !== null);

  actual = await buf.peek(4);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "ghij");

  await buf.read(p);
  assertEquals(decoder.decode(p), "ghijklmnop");

  actual = await buf.peek(0);
  assert(actual !== null);
  assertEquals(decoder.decode(actual), "");

  const r = await buf.peek(1);
  assert(r === null);
  /* TODO
  Test for issue 3022, not exposing a reader's error on a successful Peek.
  buf = NewReaderSize(dataAndEOFReader("abcd"), 32)
  if s, err := buf.Peek(2); string(s) != "ab" || err != nil {
    t.Errorf(`Peek(2) on "abcd", EOF = %q, %v; want "ab", nil`, string(s), err)
  }
  if s, err := buf.Peek(4); string(s) != "abcd" || err != nil {
    t.Errorf(
      `Peek(4) on "abcd", EOF = %q, %v; want "abcd", nil`,
      string(s),
      err
    )
  }
  if n, err := buf.Read(p[0:5]); string(p[0:n]) != "abcd" || err != nil {
    t.Fatalf("Read after peek = %q, %v; want abcd, EOF", p[0:n], err)
  }
  if n, err := buf.Read(p[0:1]); string(p[0:n]) != "" || err != io.EOF {
    t.Fatalf(`second Read after peek = %q, %v; want "", EOF`, p[0:n], err)
  }
  */
});

const encoder = new TextEncoder();

const testInput = encoder.encode(
  "012\n345\n678\n9ab\ncde\nfgh\nijk\nlmn\nopq\nrst\nuvw\nxy",
);
const testInputrn = encoder.encode(
  "012\r\n345\r\n678\r\n9ab\r\ncde\r\nfgh\r\nijk\r\nlmn\r\nopq\r\nrst\r\n" +
    "uvw\r\nxy\r\n\n\r\n",
);
const testOutput = encoder.encode("0123456789abcdefghijklmnopqrstuvwxy");

// TestReader wraps a Uint8Array and returns reads of a specific length.
class TestReader implements Reader {
  constructor(private data: Uint8Array, private stride: number) {}

  read(buf: Uint8Array): Promise<number | null> {
    let nread = this.stride;
    if (nread > this.data.byteLength) {
      nread = this.data.byteLength;
    }
    if (nread > buf.byteLength) {
      nread = buf.byteLength;
    }
    if (nread === 0) {
      return Promise.resolve(null);
    }
    copy(this.data, buf as Uint8Array);
    this.data = this.data.subarray(nread);
    return Promise.resolve(nread);
  }
}

async function testReadLine(input: Uint8Array) {
  for (let stride = 1; stride < 2; stride++) {
    let done = 0;
    const reader = new TestReader(input, stride);
    const l = new BufReader(reader, input.byteLength + 1);
    while (true) {
      const r = await l.readLine();
      if (r === null) {
        break;
      }
      const { line, more } = r;
      assertEquals(more, false);
      const want = testOutput.subarray(done, done + line.byteLength);
      assertEquals(
        line,
        want,
        `Bad line at stride ${stride}: want: ${want} got: ${line}`,
      );
      done += line.byteLength;
    }
    assertEquals(
      done,
      testOutput.byteLength,
      `readLine didn't return everything: got: ${done}, ` +
        `want: ${testOutput} (stride: ${stride})`,
    );
  }
}

Deno.test("bufioReadLine", async function () {
  await testReadLine(testInput);
  await testReadLine(testInputrn);
});

Deno.test("bufioReadLineBadResource", async () => {
  const file = await Deno.open("README.md");
  const bufReader = new BufReader(file);
  file.close();
  await assertRejects(async () => {
    await bufReader.readLine();
  }, Deno.errors.BadResource);
});

Deno.test("bufioReadLineBufferFullError", async () => {
  const input = "@".repeat(5000) + "\n";
  const bufReader = new BufReader(new StringReader(input));
  const r = await bufReader.readLine();

  assert(r !== null);

  const { line, more } = r;
  assertEquals(more, true);
  assertEquals(line, encoder.encode("@".repeat(4096)));
});

/* TODO(kt3k): Enable this test
Deno.test(
  "bufReaderShouldNotShareArrayBufferAcrossReads",
  async function () {
    const decoder = new TextDecoder();
    const data = "abcdefghijklmnopqrstuvwxyz";
    const bufSize = 25;
    const b = new BufReader(new StringReader(data), bufSize);

    const r1 = (await b.readLine()) as ReadLineResult;
    assert(r1 !== null);
    assertEquals(decoder.decode(r1.line), "abcdefghijklmnopqrstuvwxy");

    const r2 = (await b.readLine()) as ReadLineResult;
    assert(r2 !== null);
    assertEquals(decoder.decode(r2.line), "z");
    assert(
      r1.line.buffer !== r2.line.buffer,
      "array buffer should not be shared across reads",
    );
  },
);
*/
