// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import "./sha1_test.ts";

import { Buffer } from "deno";
import { BufReader } from "../io/bufio.ts";
import { test, assert, assertEqual } from "../testing/mod.ts";
import { createSecAccept, OpCode, readFrame, unmask } from "./mod.ts";
import { serve } from "../http/server.ts";

test(async function testReadUnmaskedTextFrame() {
  // unmasked single text frame with payload "Hello"
  const buf = new BufReader(
    new Buffer(new Uint8Array([0x81, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]))
  );
  const frame = await readFrame(buf);
  assertEqual(frame.opcode, OpCode.TextFrame);
  assertEqual(frame.mask, undefined);
  assertEqual(new Buffer(frame.payload).toString(), "Hello");
  assertEqual(frame.isLastFrame, true);
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
  assertEqual(frame.opcode, OpCode.TextFrame);
  unmask(frame.payload, frame.mask);
  assertEqual(new Buffer(frame.payload).toString(), "Hello");
  assertEqual(frame.isLastFrame, true);
});

test(async function testReadUnmaskedSplittedTextFrames() {
  const buf1 = new BufReader(
    new Buffer(new Uint8Array([0x01, 0x03, 0x48, 0x65, 0x6c]))
  );
  const buf2 = new BufReader(
    new Buffer(new Uint8Array([0x80, 0x02, 0x6c, 0x6f]))
  );
  const [f1, f2] = await Promise.all([readFrame(buf1), readFrame(buf2)]);
  assertEqual(f1.isLastFrame, false);
  assertEqual(f1.mask, undefined);
  assertEqual(f1.opcode, OpCode.TextFrame);
  assertEqual(new Buffer(f1.payload).toString(), "Hel");

  assertEqual(f2.isLastFrame, true);
  assertEqual(f2.mask, undefined);
  assertEqual(f2.opcode, OpCode.Continue);
  assertEqual(new Buffer(f2.payload).toString(), "lo");
});

test(async function testReadUnmaksedPingPongFrame() {
  // unmasked ping with payload "Hello"
  const buf = new BufReader(
    new Buffer(new Uint8Array([0x89, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]))
  );
  const ping = await readFrame(buf);
  assertEqual(ping.opcode, OpCode.Ping);
  assertEqual(new Buffer(ping.payload).toString(), "Hello");

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
  assertEqual(pong.opcode, OpCode.Pong);
  assert(pong.mask !== undefined);
  unmask(pong.payload, pong.mask);
  assertEqual(new Buffer(pong.payload).toString(), "Hello");
});

test(async function testReadUnmaksedBigBinaryFrame() {
  let a = [0x82, 0x7e, 0x01, 0x00];
  for (let i = 0; i < 256; i++) {
    a.push(i);
  }
  const buf = new BufReader(new Buffer(new Uint8Array(a)));
  const bin = await readFrame(buf);
  assertEqual(bin.opcode, OpCode.BinaryFrame);
  assertEqual(bin.isLastFrame, true);
  assertEqual(bin.mask, undefined);
  assertEqual(bin.payload.length, 256);
});

test(async function testReadUnmaskedBigBigBinaryFrame() {
  let a = [0x82, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00];
  for (let i = 0; i < 0xffff; i++) {
    a.push(i);
  }
  const buf = new BufReader(new Buffer(new Uint8Array(a)));
  const bin = await readFrame(buf);
  assertEqual(bin.opcode, OpCode.BinaryFrame);
  assertEqual(bin.isLastFrame, true);
  assertEqual(bin.mask, undefined);
  assertEqual(bin.payload.length, 0xffff + 1);
});

test(async function testCreateSecAccept() {
  const nonce = "dGhlIHNhbXBsZSBub25jZQ==";
  const d = createSecAccept(nonce);
  assertEqual(d, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
});
