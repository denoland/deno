// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { BufReader } from "../io/bufio.ts";
import { assert, assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
import { runIfMain, test } from "../testing/mod.ts";
import {
  acceptable,
  connectWebSocket,
  createSecAccept,
  OpCode,
  readFrame,
  unmask,
  writeFrame
} from "./mod.ts";
import { encode } from "../strings/mod.ts";

const { Buffer } = Deno;

test(async function wsReadUnmaskedTextFrame(): Promise<void> {
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

test(async function wsReadMaskedTextFrame(): Promise<void> {
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
  assertEquals(frame.opcode, OpCode.TextFrame);
  unmask(frame.payload, frame.mask);
  assertEquals(new Buffer(frame.payload).toString(), "Hello");
  assertEquals(frame.isLastFrame, true);
});

test(async function wsReadUnmaskedSplitTextFrames(): Promise<void> {
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

test(async function wsReadUnmaskedPingPongFrame(): Promise<void> {
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

test(async function wsReadUnmaskedBigBinaryFrame(): Promise<void> {
  const payloadLength = 0x100;
  const a = [0x82, 0x7e, 0x01, 0x00];
  for (let i = 0; i < payloadLength; i++) {
    a.push(i);
  }
  const buf = new BufReader(new Buffer(new Uint8Array(a)));
  const bin = await readFrame(buf);
  assertEquals(bin.opcode, OpCode.BinaryFrame);
  assertEquals(bin.isLastFrame, true);
  assertEquals(bin.mask, undefined);
  assertEquals(bin.payload.length, payloadLength);
});

test(async function wsReadUnmaskedBigBigBinaryFrame(): Promise<void> {
  const payloadLength = 0x10000;
  const a = [0x82, 0x7f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00];
  for (let i = 0; i < payloadLength; i++) {
    a.push(i);
  }
  const buf = new BufReader(new Buffer(new Uint8Array(a)));
  const bin = await readFrame(buf);
  assertEquals(bin.opcode, OpCode.BinaryFrame);
  assertEquals(bin.isLastFrame, true);
  assertEquals(bin.mask, undefined);
  assertEquals(bin.payload.length, payloadLength);
});

test(async function wsCreateSecAccept(): Promise<void> {
  const nonce = "dGhlIHNhbXBsZSBub25jZQ==";
  const d = createSecAccept(nonce);
  assertEquals(d, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
});

test(function wsAcceptable(): void {
  const ret = acceptable({
    headers: new Headers({
      upgrade: "websocket",
      "sec-websocket-key": "aaa"
    })
  });
  assertEquals(ret, true);

  assert(
    acceptable({
      headers: new Headers([
        ["connection", "Upgrade"],
        ["host", "127.0.0.1:9229"],
        [
          "sec-websocket-extensions",
          "permessage-deflate; client_max_window_bits"
        ],
        ["sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="],
        ["sec-websocket-version", "13"],
        ["upgrade", "WebSocket"]
      ])
    })
  );
});

test(function wsAcceptableInvalid(): void {
  assertEquals(
    acceptable({
      headers: new Headers({ "sec-websocket-key": "aaa" })
    }),
    false
  );
  assertEquals(
    acceptable({
      headers: new Headers({ upgrade: "websocket" })
    }),
    false
  );
  assertEquals(
    acceptable({
      headers: new Headers({ upgrade: "invalid", "sec-websocket-key": "aaa" })
    }),
    false
  );
  assertEquals(
    acceptable({
      headers: new Headers({ upgrade: "websocket", "sec-websocket-ky": "" })
    }),
    false
  );
});

test("connectWebSocket should throw invalid scheme of url", async (): Promise<
  void
> => {
  await assertThrowsAsync(
    async (): Promise<void> => {
      await connectWebSocket("file://hoge/hoge");
    }
  );
});

test(async function wsWriteReadMaskedFrame(): Promise<void> {
  const mask = new Uint8Array([0, 1, 2, 3]);
  const msg = "hello";
  const buf = new Buffer();
  const r = new BufReader(buf);
  await writeFrame(
    {
      isLastFrame: true,
      mask,
      opcode: OpCode.TextFrame,
      payload: encode(msg)
    },
    buf
  );
  const frame = await readFrame(r);
  assertEquals(frame.opcode, OpCode.TextFrame);
  assertEquals(frame.isLastFrame, true);
  assertEquals(frame.mask, mask);
  unmask(frame.payload, frame.mask);
  assertEquals(frame.payload, encode(msg));
});

runIfMain(import.meta);
