import * as deno from "deno";
import { test, assertEqual } from "http://deno.land/x/testing/testing.ts";
import * as bufio from "./bufio.ts";
import { Buffer } from "./buffer.ts";

async function readBytes(buf: bufio.Reader): Promise<string> {
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
  const b = new bufio.Reader(stringsReader(data));
  const s = await readBytes(b);
  assertEqual(s, data);
});
