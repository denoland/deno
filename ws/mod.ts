// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { Buffer } = Deno;
import { Writer, Conn } from "deno";
import { BufReader, BufWriter } from "../io/bufio.ts";
import { readLong, readShort, sliceLongToBytes } from "../io/ioutil.ts";
import { Sha1 } from "./sha1.ts";
import { writeResponse } from "../http/server.ts";

export enum OpCode {
  Continue = 0x0,
  TextFrame = 0x1,
  BinaryFrame = 0x2,
  Close = 0x8,
  Ping = 0x9,
  Pong = 0xa
}

export type WebSocketEvent =
  | string
  | Uint8Array
  | WebSocketCloseEvent
  | WebSocketPingEvent
  | WebSocketPongEvent;

export type WebSocketCloseEvent = {
  code: number;
  reason?: string;
};

export function isWebSocketCloseEvent(a): a is WebSocketCloseEvent {
  return a && typeof a["code"] === "number";
}

export type WebSocketPingEvent = ["ping", Uint8Array];

export function isWebSocketPingEvent(a): a is WebSocketPingEvent {
  return Array.isArray(a) && a[0] === "ping" && a[1] instanceof Uint8Array;
}

export type WebSocketPongEvent = ["pong", Uint8Array];

export function isWebSocketPongEvent(a): a is WebSocketPongEvent {
  return Array.isArray(a) && a[0] === "pong" && a[1] instanceof Uint8Array;
}

export type WebSocketMessage = string | Uint8Array;

// TODO move this to common/util module
export function append(a: Uint8Array, b: Uint8Array) {
  if (a == null || !a.length) {
    return b;
  }
  if (b == null || !b.length) {
    return a;
  }
  const output = new Uint8Array(a.length + b.length);
  output.set(a, 0);
  output.set(b, a.length);
  return output;
}

export class SocketClosedError extends Error {}

export type WebSocketFrame = {
  isLastFrame: boolean;
  opcode: OpCode;
  mask?: Uint8Array;
  payload: Uint8Array;
};

export type WebSocket = {
  readonly isClosed: boolean;
  receive(): AsyncIterableIterator<WebSocketEvent>;
  send(data: WebSocketMessage): Promise<void>;
  ping(data?: WebSocketMessage): Promise<void>;
  close(code: number, reason?: string): Promise<void>;
};

class WebSocketImpl implements WebSocket {
  encoder = new TextEncoder();

  constructor(private conn: Conn, private mask?: Uint8Array) {}

  async *receive(): AsyncIterableIterator<WebSocketEvent> {
    let frames: WebSocketFrame[] = [];
    let payloadsLength = 0;
    for await (const frame of receiveFrame(this.conn)) {
      unmask(frame.payload, frame.mask);
      switch (frame.opcode) {
        case OpCode.TextFrame:
        case OpCode.BinaryFrame:
        case OpCode.Continue:
          frames.push(frame);
          payloadsLength += frame.payload.length;
          if (frame.isLastFrame) {
            const concat = new Uint8Array(payloadsLength);
            let offs = 0;
            for (const frame of frames) {
              concat.set(frame.payload, offs);
              offs += frame.payload.length;
            }
            if (frames[0].opcode === OpCode.TextFrame) {
              // text
              yield new Buffer(concat).toString();
            } else {
              // binary
              yield concat;
            }
            frames = [];
            payloadsLength = 0;
          }
          break;
        case OpCode.Close:
          const code = (frame.payload[0] << 16) | frame.payload[1];
          const reason = new Buffer(
            frame.payload.subarray(2, frame.payload.length)
          ).toString();
          this._isClosed = true;
          yield { code, reason };
          return;
        case OpCode.Ping:
          yield ["ping", frame.payload] as WebSocketPingEvent;
          break;
        case OpCode.Pong:
          yield ["pong", frame.payload] as WebSocketPongEvent;
          break;
        default:
      }
    }
  }

  async send(data: WebSocketMessage): Promise<void> {
    if (this.isClosed) {
      throw new SocketClosedError("socket has been closed");
    }
    const opcode =
      typeof data === "string" ? OpCode.TextFrame : OpCode.BinaryFrame;
    const payload = typeof data === "string" ? this.encoder.encode(data) : data;
    const isLastFrame = true;
    await writeFrame(
      {
        isLastFrame,
        opcode,
        payload,
        mask: this.mask
      },
      this.conn
    );
  }

  async ping(data: WebSocketMessage): Promise<void> {
    const payload = typeof data === "string" ? this.encoder.encode(data) : data;
    await writeFrame(
      {
        isLastFrame: true,
        opcode: OpCode.Close,
        mask: this.mask,
        payload
      },
      this.conn
    );
  }

  private _isClosed = false;
  get isClosed() {
    return this._isClosed;
  }

  async close(code: number, reason?: string): Promise<void> {
    try {
      const header = [code >>> 8, code & 0x00ff];
      let payload: Uint8Array;
      if (reason) {
        const reasonBytes = this.encoder.encode(reason);
        payload = new Uint8Array(2 + reasonBytes.byteLength);
        payload.set(header);
        payload.set(reasonBytes, 2);
      } else {
        payload = new Uint8Array(header);
      }
      await writeFrame(
        {
          isLastFrame: true,
          opcode: OpCode.Close,
          mask: this.mask,
          payload
        },
        this.conn
      );
    } catch (e) {
      throw e;
    } finally {
      this.ensureSocketClosed();
    }
  }

  private ensureSocketClosed(): Error {
    if (this.isClosed) {
      return;
    }

    try {
      this.conn.close();
    } catch (e) {
      console.error(e);
    } finally {
      this._isClosed = true;
    }
  }
}

export async function* receiveFrame(
  conn: Conn
): AsyncIterableIterator<WebSocketFrame> {
  let receiving = true;
  const isLastFrame = true;
  const reader = new BufReader(conn);
  while (receiving) {
    const frame = await readFrame(reader);
    const { opcode, payload } = frame;
    switch (opcode) {
      case OpCode.TextFrame:
      case OpCode.BinaryFrame:
      case OpCode.Continue:
        yield frame;
        break;
      case OpCode.Close:
        await writeFrame(
          {
            opcode,
            payload,
            isLastFrame
          },
          conn
        );
        conn.close();
        yield frame;
        receiving = false;
        break;
      case OpCode.Ping:
        await writeFrame(
          {
            payload,
            isLastFrame,
            opcode: OpCode.Pong
          },
          conn
        );
        yield frame;
        break;
      case OpCode.Pong:
        yield frame;
        break;
      default:
    }
  }
}

export async function writeFrame(frame: WebSocketFrame, writer: Writer) {
  const payloadLength = frame.payload.byteLength;
  let header: Uint8Array;
  const hasMask = frame.mask ? 0x80 : 0;
  if (payloadLength < 126) {
    header = new Uint8Array([0x80 | frame.opcode, hasMask | payloadLength]);
  } else if (payloadLength < 0xffff) {
    header = new Uint8Array([
      0x80 | frame.opcode,
      hasMask | 0b01111110,
      payloadLength >>> 8,
      payloadLength & 0x00ff
    ]);
  } else {
    header = new Uint8Array([
      0x80 | frame.opcode,
      hasMask | 0b01111111,
      ...sliceLongToBytes(payloadLength)
    ]);
  }
  unmask(frame.payload, frame.mask);
  const bytes = append(header, frame.payload);
  const w = new BufWriter(writer);
  await w.write(bytes);
  await w.flush();
}

export function unmask(payload: Uint8Array, mask?: Uint8Array) {
  if (mask) {
    for (let i = 0, len = payload.length; i < len; i++) {
      payload[i] ^= mask![i & 3];
    }
  }
}

export function acceptable(req: { headers: Headers }): boolean {
  return (
    req.headers.get("upgrade") === "websocket" &&
    req.headers.has("sec-websocket-key") &&
    req.headers.get("sec-websocket-key").length > 0
  );
}

export async function acceptWebSocket(req: {
  conn: Conn;
  headers: Headers;
}): Promise<WebSocket> {
  const { conn, headers } = req;
  if (acceptable(req)) {
    const sock = new WebSocketImpl(conn);
    const secKey = headers.get("sec-websocket-key");
    const secAccept = createSecAccept(secKey);
    await writeResponse(conn, {
      status: 101,
      headers: new Headers({
        Upgrade: "websocket",
        Connection: "Upgrade",
        "Sec-WebSocket-Accept": secAccept
      })
    });
    return sock;
  }
  throw new Error("request is not acceptable");
}

const kGUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

export function createSecAccept(nonce: string) {
  const sha1 = new Sha1();
  sha1.update(nonce + kGUID);
  const bytes = sha1.digest();
  return btoa(String.fromCharCode.apply(String, bytes));
}

export async function readFrame(buf: BufReader): Promise<WebSocketFrame> {
  let b = await buf.readByte();
  let isLastFrame = false;
  switch (b >>> 4) {
    case 0b1000:
      isLastFrame = true;
      break;
    case 0b0000:
      isLastFrame = false;
      break;
    default:
      throw new Error("invalid signature");
  }
  const opcode = b & 0x0f;
  // has_mask & payload
  b = await buf.readByte();
  const hasMask = b >>> 7;
  let payloadLength = b & 0b01111111;
  if (payloadLength === 126) {
    payloadLength = await readShort(buf);
  } else if (payloadLength === 127) {
    payloadLength = await readLong(buf);
  }
  // mask
  let mask;
  if (hasMask) {
    mask = new Uint8Array(4);
    await buf.readFull(mask);
  }
  // payload
  const payload = new Uint8Array(payloadLength);
  await buf.readFull(payload);
  return {
    isLastFrame,
    opcode,
    mask,
    payload
  };
}
