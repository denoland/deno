// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { BufReader, BufWriter } from "../io/bufio.ts";
import { assert, assertEquals, assertThrowsAsync } from "../testing/asserts.ts";
const { test } = Deno;
import { TextProtoReader } from "../textproto/mod.ts";
import * as bytes from "../bytes/mod.ts";
import {
  acceptable,
  connectWebSocket,
  createSecAccept,
  createSecKey,
  handshake,
  OpCode,
  readFrame,
  unmask,
  writeFrame,
  createWebSocket,
} from "./mod.ts";
import { encode, decode } from "../strings/mod.ts";
import Writer = Deno.Writer;
import Reader = Deno.Reader;
import Conn = Deno.Conn;
import Buffer = Deno.Buffer;
import { delay } from "../util/async.ts";

test("[ws] read unmasked text frame", async () => {
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

test("[ws] read masked text frame", async () => {
  // a masked single text frame with payload "Hello"
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
        0x58,
      ])
    )
  );
  const frame = await readFrame(buf);
  assertEquals(frame.opcode, OpCode.TextFrame);
  unmask(frame.payload, frame.mask);
  assertEquals(new Buffer(frame.payload).toString(), "Hello");
  assertEquals(frame.isLastFrame, true);
});

test("[ws] read unmasked split text frames", async () => {
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

test("[ws] read unmasked ping / pong frame", async () => {
  // unmasked ping with payload "Hello"
  const buf = new BufReader(
    new Buffer(new Uint8Array([0x89, 0x05, 0x48, 0x65, 0x6c, 0x6c, 0x6f]))
  );
  const ping = await readFrame(buf);
  assertEquals(ping.opcode, OpCode.Ping);
  assertEquals(new Buffer(ping.payload).toString(), "Hello");
  // prettier-ignore
  const pongFrame= [0x8a, 0x85, 0x37, 0xfa, 0x21, 0x3d, 0x7f, 0x9f, 0x4d, 0x51, 0x58]
  const buf2 = new BufReader(new Buffer(new Uint8Array(pongFrame)));
  const pong = await readFrame(buf2);
  assertEquals(pong.opcode, OpCode.Pong);
  assert(pong.mask !== undefined);
  unmask(pong.payload, pong.mask);
  assertEquals(new Buffer(pong.payload).toString(), "Hello");
});

test("[ws] read unmasked big binary frame", async () => {
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

test("[ws] read unmasked bigger binary frame", async () => {
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

test("[ws] createSecAccept", () => {
  const nonce = "dGhlIHNhbXBsZSBub25jZQ==";
  const d = createSecAccept(nonce);
  assertEquals(d, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
});

test("[ws] acceptable", () => {
  const ret = acceptable({
    headers: new Headers({
      upgrade: "websocket",
      "sec-websocket-key": "aaa",
    }),
  });
  assertEquals(ret, true);

  assert(
    acceptable({
      headers: new Headers([
        ["connection", "Upgrade"],
        ["host", "127.0.0.1:9229"],
        [
          "sec-websocket-extensions",
          "permessage-deflate; client_max_window_bits",
        ],
        ["sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="],
        ["sec-websocket-version", "13"],
        ["upgrade", "WebSocket"],
      ]),
    })
  );
});

test("[ws] acceptable should return false when headers invalid", () => {
  assertEquals(
    acceptable({
      headers: new Headers({ "sec-websocket-key": "aaa" }),
    }),
    false
  );
  assertEquals(
    acceptable({
      headers: new Headers({ upgrade: "websocket" }),
    }),
    false
  );
  assertEquals(
    acceptable({
      headers: new Headers({ upgrade: "invalid", "sec-websocket-key": "aaa" }),
    }),
    false
  );
  assertEquals(
    acceptable({
      headers: new Headers({ upgrade: "websocket", "sec-websocket-ky": "" }),
    }),
    false
  );
});

test("[ws] connectWebSocket should throw invalid scheme of url", async (): Promise<
  void
> => {
  await assertThrowsAsync(
    async (): Promise<void> => {
      await connectWebSocket("file://hoge/hoge");
    }
  );
});

test("[ws] write and read masked frame", async () => {
  const mask = new Uint8Array([0, 1, 2, 3]);
  const msg = "hello";
  const buf = new Buffer();
  const r = new BufReader(buf);
  await writeFrame(
    {
      isLastFrame: true,
      mask,
      opcode: OpCode.TextFrame,
      payload: encode(msg),
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

test("[ws] handshake should not send search when it's empty", async () => {
  const writer = new Buffer();
  const reader = new Buffer(encode("HTTP/1.1 400\r\n"));

  await assertThrowsAsync(
    async (): Promise<void> => {
      await handshake(
        new URL("ws://example.com"),
        new Headers(),
        new BufReader(reader),
        new BufWriter(writer)
      );
    }
  );

  const tpReader = new TextProtoReader(new BufReader(writer));
  const statusLine = await tpReader.readLine();

  assertEquals(statusLine, "GET / HTTP/1.1");
});

test("[ws] handshake should send search correctly", async function wsHandshakeWithSearch(): Promise<
  void
> {
  const writer = new Buffer();
  const reader = new Buffer(encode("HTTP/1.1 400\r\n"));

  await assertThrowsAsync(
    async (): Promise<void> => {
      await handshake(
        new URL("ws://example.com?a=1"),
        new Headers(),
        new BufReader(reader),
        new BufWriter(writer)
      );
    }
  );

  const tpReader = new TextProtoReader(new BufReader(writer));
  const statusLine = await tpReader.readLine();

  assertEquals(statusLine, "GET /?a=1 HTTP/1.1");
});

test("[ws] ws.close() should use 1000 as close code", async () => {
  const buf = new Buffer();
  const bufr = new BufReader(buf);
  const conn = dummyConn(buf, buf);
  const ws = createWebSocket({ conn });
  await ws.close();
  const frame = await readFrame(bufr);
  assertEquals(frame.opcode, OpCode.Close);
  const code = (frame.payload[0] << 8) | frame.payload[1];
  assertEquals(code, 1000);
});

function dummyConn(r: Reader, w: Writer): Conn {
  return {
    rid: -1,
    closeRead: (): void => {},
    closeWrite: (): void => {},
    read: (x): Promise<number | Deno.EOF> => r.read(x),
    write: (x): Promise<number> => w.write(x),
    close: (): void => {},
    localAddr: { transport: "tcp", hostname: "0.0.0.0", port: 0 },
    remoteAddr: { transport: "tcp", hostname: "0.0.0.0", port: 0 },
  };
}

function delayedWriter(ms: number, dest: Writer): Writer {
  return {
    write(p: Uint8Array): Promise<number> {
      return new Promise<number>((resolve) => {
        setTimeout(async (): Promise<void> => {
          resolve(await dest.write(p));
        }, ms);
      });
    },
  };
}
test({
  name: "[ws] WebSocket.send(), WebSocket.ping() should be exclusive",
  fn: async (): Promise<void> => {
    const buf = new Buffer();
    const conn = dummyConn(new Buffer(), delayedWriter(1, buf));
    const sock = createWebSocket({ conn });
    // Ensure send call
    await Promise.all([
      sock.send("first"),
      sock.send("second"),
      sock.ping(),
      sock.send(new Uint8Array([3])),
    ]);
    const bufr = new BufReader(buf);
    const first = await readFrame(bufr);
    const second = await readFrame(bufr);
    const ping = await readFrame(bufr);
    const third = await readFrame(bufr);
    assertEquals(first.opcode, OpCode.TextFrame);
    assertEquals(decode(first.payload), "first");
    assertEquals(first.opcode, OpCode.TextFrame);
    assertEquals(decode(second.payload), "second");
    assertEquals(ping.opcode, OpCode.Ping);
    assertEquals(third.opcode, OpCode.BinaryFrame);
    assertEquals(bytes.equal(third.payload, new Uint8Array([3])), true);
  },
});

test("[ws] createSecKeyHasCorrectLength", () => {
  // Note: relies on --seed=86 being passed to deno to reproduce failure in
  // #4063.
  const secKey = createSecKey();
  assertEquals(atob(secKey).length, 16);
});

test("[ws] WebSocket should throw `Deno.errors.ConnectionReset` when peer closed connection without close frame", async () => {
  const buf = new Buffer();
  const eofReader: Deno.Reader = {
    read(_: Uint8Array): Promise<number | Deno.EOF> {
      return Promise.resolve(Deno.EOF);
    },
  };
  const conn = dummyConn(eofReader, buf);
  const sock = createWebSocket({ conn });
  sock.closeForce();
  await assertThrowsAsync(
    () => sock.send("hello"),
    Deno.errors.ConnectionReset
  );
  await assertThrowsAsync(() => sock.ping(), Deno.errors.ConnectionReset);
  await assertThrowsAsync(() => sock.close(0), Deno.errors.ConnectionReset);
});

test("[ws] WebSocket shouldn't throw `Deno.errors.UnexpectedEof` on recive()", async () => {
  const buf = new Buffer();
  const eofReader: Deno.Reader = {
    read(_: Uint8Array): Promise<number | Deno.EOF> {
      return Promise.resolve(Deno.EOF);
    },
  };
  const conn = dummyConn(eofReader, buf);
  const sock = createWebSocket({ conn });
  const it = sock.receive();
  const { value, done } = await it.next();
  assertEquals(value, undefined);
  assertEquals(done, true);
});

test({
  name:
    "[ws] WebSocket should reject sending promise when connection reset forcely",
  fn: async () => {
    const buf = new Buffer();
    let timer: number | undefined;
    const lazyWriter: Deno.Writer = {
      write(_: Uint8Array): Promise<number> {
        return new Promise((resolve) => {
          timer = setTimeout(() => resolve(0), 1000);
        });
      },
    };
    const conn = dummyConn(buf, lazyWriter);
    const sock = createWebSocket({ conn });
    const onError = (e: unknown): unknown => e;
    const p = Promise.all([
      sock.send("hello").catch(onError),
      sock.send(new Uint8Array([1, 2])).catch(onError),
      sock.ping().catch(onError),
    ]);
    sock.closeForce();
    assertEquals(sock.isClosed, true);
    const [a, b, c] = await p;
    assert(a instanceof Deno.errors.ConnectionReset);
    assert(b instanceof Deno.errors.ConnectionReset);
    assert(c instanceof Deno.errors.ConnectionReset);
    clearTimeout(timer);
    // Wait for another event loop turn for `timeout` op promise
    // to resolve, otherwise we'll get "op leak".
    await delay(10);
  },
});
