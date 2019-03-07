// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./sha1_test.ts";

const { Buffer } = Deno;
import { BufReader } from "../io/bufio.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { test } from "../testing/mod.ts";
import {
  acceptable,
  createSecAccept,
  OpCode,
  readFrame,
  unmask
} from "./mod.ts";

test(async function testReadUnmaskedTextFrame() {
  // unmasked single text frame with payload "Hello"
  const buf = new BufReader(
    new Buffer(new Uint8Array([0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]))
  );
  const frame = await readFrame(buf);
  assertEquals(frame.opcode, OpCode.TextFrame);
  assertEquals(frame.mask, undefined);
  assertEquals(new Buffer(frame.payload).toString(), "Hello");
  assertEquals(frame.isLastFrame, true);
});

test(async function testReadMakedTextFrame() {
  //a masked single text frame with payload "Hello"
  const buf = new BufReader(
    new Buffer(
      new Uint8Array([
        0x81,
        0x85,
        0x37,
        0xfa,
        0x21,
        0x3d,
        0x7f,
        0x9f,
        0x4d,
        0x51,
        0x58
      ])
    )
  );
  const frame = await readFrame(buf);
  console.dir(frame);
  assertEquals(frame.opcode, OpCode.TextFrame);
  unmask(frame.payload, frame.mask);
  assertEquals(new Buffer(frame.payload).toString(), "Hello");
  assertEquals(frame.isLastFrame, true);
});

test(async function testReadUnmaskedSplittedTextFrames() {
  const buf1 = new BufReader(
    new Buffer(new Uint8Array([0x01, 0x03, 0x48, 0x65, 0x6c]))
  );
  const buf2 = new BufReader(
    new Buffer(new Uint8Array([0x80, 0x02, 0x6c, 0x6f]))
  );
  const [f1, f2] = await Promise.all([readFrame(buf1), readFrame(buf2)]);
  assertEquals(f1.isLastFrame, false);
  assertEquals(f1.mask, undefined);
  assertEquals(f1.opcode, OpCode.TextFrame);
  assertEquals(new Buffer(f1.payload).toString(), "Hel");

  assertEquals(f2.isLastFrame, true);
  assertEquals(f2.mask, undefined);
  assertEquals(f2.opcode, OpCode.Continue);
  assertEquals(new Buffer(f2.payload).toString(), "lo");
});

test(async function testReadUnmaksedPingPongFrame() {
  // unmasked ping with payload "Hello"
  const buf = new BufReader(
    new Buffer(new Uint8Array([0x89, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]))
  );
  const ping = await readFrame(buf);
  assertEquals(ping.opcode, OpCode.Ping);
  assertEquals(new Buffer(ping.payload).toString(), "Hello");

  const buf2 = new BufReader(
    new Buffer(
      new Uint8Array([
        0x8a,
        0x85,
        0x37,
        0xfa,
        0x21,
        0x3d,
        0x7f,
        0x9f,
        0x4d,
        0x51,
        0x58
      ])
    )
  );
  const pong = await readFrame(buf2);
  assertEquals(pong.opcode, OpCode.Pong);
  assert(pong.mask !== undefined);
  unmask(pong.payload, pong.mask);
  assertEquals(new Buffer(pong.payload).toString(), "Hello");
});

test(async function testReadUnmaksedBigBinaryFrame() {
  const a = [0x82, 0x7e, 0x01, 0x00];
  for (let i = 0; i < 256; i++) {
    a.push(i);
  }
  const buf = new BufReader(new Buffer(new Uint8Array(a)));
  const bin = await readFrame(buf);
  assertEquals(bin.opcode, OpCode.BinaryFrame);
  assertEquals(bin.isLastFrame, true);
  assertEquals(bin.mask, undefined);
  assertEquals(bin.payload.length, 256);
});

test(async function testReadUnmaskedBigBigBinaryFrame() {
  const a = [0x82, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00];
  for (let i = 0; i < 0xffff; i++) {
    a.push(i);
  }
  const buf = new BufReader(new Buffer(new Uint8Array(a)));
  const bin = await readFrame(buf);
  assertEquals(bin.opcode, OpCode.BinaryFrame);
  assertEquals(bin.isLastFrame, true);
  assertEquals(bin.mask, undefined);
  assertEquals(bin.payload.length, 0xffff + 1);
});

test(async function testCreateSecAccept() {
  const nonce = "dGhlIHNhbXBsZSBub25jZQ==";
  const d = createSecAccept(nonce);
  assertEquals(d, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
});

test(function testAcceptable() {
  const ret = acceptable({
    headers: new Headers({
      upgrade: "websocket",
      "sec-websocket-key": "aaa"
    })
  });
  assertEquals(ret, true);
});

const invalidHeaders = [
  { "sec-websocket-key": "aaa" },
  { upgrade: "websocket" },
  { upgrade: "invalid", "sec-websocket-key": "aaa" },
  { upgrade: "websocket", "sec-websocket-ky": "" }
];

test(function testAcceptableInvalid() {
  for (const pat of invalidHeaders) {
    const ret = acceptable({
      headers: new Headers(pat)
    });
    assertEquals(ret, false);
  }
});
