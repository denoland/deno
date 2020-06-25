// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { decode, encode } from "../encoding/utf8.ts";
import { hasOwnProperty } from "../_util/has_own_property.ts";
import { BufReader, BufWriter } from "../io/bufio.ts";
import { readLong, readShort, sliceLongToBytes } from "../io/ioutil.ts";
import { Sha1 } from "../hash/sha1.ts";
import { writeResponse } from "../http/_io.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { Deferred, deferred } from "../async/deferred.ts";
import { assert } from "../_util/assert.ts";
import { concat } from "../bytes/mod.ts";

export enum OpCode {
  Continue = 0x0,
  TextFrame = 0x1,
  BinaryFrame = 0x2,
  Close = 0x8,
  Ping = 0x9,
  Pong = 0xa,
}

export type WebSocketEvent =
  | string
  | Uint8Array
  | WebSocketCloseEvent // Received after closing connection finished.
  | WebSocketPingEvent // Received after pong frame responded.
  | WebSocketPongEvent;

export interface WebSocketCloseEvent {
  code: number;
  reason?: string;
}

export function isWebSocketCloseEvent(
  a: WebSocketEvent
): a is WebSocketCloseEvent {
  return hasOwnProperty(a, "code");
}

export type WebSocketPingEvent = ["ping", Uint8Array];

export function isWebSocketPingEvent(
  a: WebSocketEvent
): a is WebSocketPingEvent {
  return Array.isArray(a) && a[0] === "ping" && a[1] instanceof Uint8Array;
}

export type WebSocketPongEvent = ["pong", Uint8Array];

export function isWebSocketPongEvent(
  a: WebSocketEvent
): a is WebSocketPongEvent {
  return Array.isArray(a) && a[0] === "pong" && a[1] instanceof Uint8Array;
}

export type WebSocketMessage = string | Uint8Array;

export interface WebSocketFrame {
  isLastFrame: boolean;
  opcode: OpCode;
  mask?: Uint8Array;
  payload: Uint8Array;
}

export interface WebSocket extends AsyncIterable<WebSocketEvent> {
  readonly conn: Deno.Conn;
  readonly isClosed: boolean;

  [Symbol.asyncIterator](): AsyncIterableIterator<WebSocketEvent>;

  /**
   * @throws `Deno.errors.ConnectionReset`
   */
  send(data: WebSocketMessage): Promise<void>;

  /**
   * @param data
   * @throws `Deno.errors.ConnectionReset`
   */
  ping(data?: WebSocketMessage): Promise<void>;

  /** Close connection after sending close frame to peer.
   * This is canonical way of disconnection but it may hang because of peer's response delay.
   * Default close code is 1000 (Normal Closure)
   * @throws `Deno.errors.ConnectionReset`
   */
  close(): Promise<void>;
  close(code: number): Promise<void>;
  close(code: number, reason: string): Promise<void>;

  /** Close connection forcely without sending close frame to peer.
   *  This is basically undesirable way of disconnection. Use carefully. */
  closeForce(): void;
}

/** Unmask masked websocket payload */
export function unmask(payload: Uint8Array, mask?: Uint8Array): void {
  if (mask) {
    for (let i = 0, len = payload.length; i < len; i++) {
      payload[i] ^= mask[i & 3];
    }
  }
}

/** Write websocket frame to given writer */
export async function writeFrame(
  frame: WebSocketFrame,
  writer: Deno.Writer
): Promise<void> {
  const payloadLength = frame.payload.byteLength;
  let header: Uint8Array;
  const hasMask = frame.mask ? 0x80 : 0;
  if (frame.mask && frame.mask.byteLength !== 4) {
    throw new Error(
      "invalid mask. mask must be 4 bytes: length=" + frame.mask.byteLength
    );
  }
  if (payloadLength < 126) {
    header = new Uint8Array([0x80 | frame.opcode, hasMask | payloadLength]);
  } else if (payloadLength < 0xffff) {
    header = new Uint8Array([
      0x80 | frame.opcode,
      hasMask | 0b01111110,
      payloadLength >>> 8,
      payloadLength & 0x00ff,
    ]);
  } else {
    header = new Uint8Array([
      0x80 | frame.opcode,
      hasMask | 0b01111111,
      ...sliceLongToBytes(payloadLength),
    ]);
  }
  if (frame.mask) {
    header = concat(header, frame.mask);
  }
  unmask(frame.payload, frame.mask);
  header = concat(header, frame.payload);
  const w = BufWriter.create(writer);
  await w.write(header);
  await w.flush();
}

/** Read websocket frame from given BufReader
 * @throws `Deno.errors.UnexpectedEof` When peer closed connection without close frame
 * @throws `Error` Frame is invalid
 */
export async function readFrame(buf: BufReader): Promise<WebSocketFrame> {
  let b = await buf.readByte();
  assert(b !== null);
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
  assert(b !== null);
  const hasMask = b >>> 7;
  let payloadLength = b & 0b01111111;
  if (payloadLength === 126) {
    const l = await readShort(buf);
    assert(l !== null);
    payloadLength = l;
  } else if (payloadLength === 127) {
    const l = await readLong(buf);
    assert(l !== null);
    payloadLength = Number(l);
  }
  // mask
  let mask: Uint8Array | undefined;
  if (hasMask) {
    mask = new Uint8Array(4);
    assert((await buf.readFull(mask)) !== null);
  }
  // payload
  const payload = new Uint8Array(payloadLength);
  assert((await buf.readFull(payload)) !== null);
  return {
    isLastFrame,
    opcode,
    mask,
    payload,
  };
}

// Create client-to-server mask, random 32bit number
function createMask(): Uint8Array {
  return crypto.getRandomValues(new Uint8Array(4));
}

class WebSocketImpl implements WebSocket {
  readonly conn: Deno.Conn;
  private readonly mask?: Uint8Array;
  private readonly bufReader: BufReader;
  private readonly bufWriter: BufWriter;
  private sendQueue: Array<{
    frame: WebSocketFrame;
    d: Deferred<void>;
  }> = [];

  constructor({
    conn,
    bufReader,
    bufWriter,
    mask,
  }: {
    conn: Deno.Conn;
    bufReader?: BufReader;
    bufWriter?: BufWriter;
    mask?: Uint8Array;
  }) {
    this.conn = conn;
    this.mask = mask;
    this.bufReader = bufReader || new BufReader(conn);
    this.bufWriter = bufWriter || new BufWriter(conn);
  }

  async *[Symbol.asyncIterator](): AsyncIterableIterator<WebSocketEvent> {
    let frames: WebSocketFrame[] = [];
    let payloadsLength = 0;
    while (!this._isClosed) {
      let frame: WebSocketFrame;
      try {
        frame = await readFrame(this.bufReader);
      } catch (e) {
        this.ensureSocketClosed();
        break;
      }
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
              yield decode(concat);
            } else {
              // binary
              yield concat;
            }
            frames = [];
            payloadsLength = 0;
          }
          break;
        case OpCode.Close: {
          // [0x12, 0x34] -> 0x1234
          const code = (frame.payload[0] << 8) | frame.payload[1];
          const reason = decode(
            frame.payload.subarray(2, frame.payload.length)
          );
          await this.close(code, reason);
          yield { code, reason };
          return;
        }
        case OpCode.Ping:
          await this.enqueue({
            opcode: OpCode.Pong,
            payload: frame.payload,
            isLastFrame: true,
          });
          yield ["ping", frame.payload] as WebSocketPingEvent;
          break;
        case OpCode.Pong:
          yield ["pong", frame.payload] as WebSocketPongEvent;
          break;
        default:
      }
    }
  }

  private dequeue(): void {
    const [entry] = this.sendQueue;
    if (!entry) return;
    if (this._isClosed) return;
    const { d, frame } = entry;
    writeFrame(frame, this.bufWriter)
      .then(() => d.resolve())
      .catch((e) => d.reject(e))
      .finally(() => {
        this.sendQueue.shift();
        this.dequeue();
      });
  }

  private enqueue(frame: WebSocketFrame): Promise<void> {
    if (this._isClosed) {
      throw new Deno.errors.ConnectionReset("Socket has already been closed");
    }
    const d = deferred<void>();
    this.sendQueue.push({ d, frame });
    if (this.sendQueue.length === 1) {
      this.dequeue();
    }
    return d;
  }

  send(data: WebSocketMessage): Promise<void> {
    const opcode =
      typeof data === "string" ? OpCode.TextFrame : OpCode.BinaryFrame;
    const payload = typeof data === "string" ? encode(data) : data;
    const isLastFrame = true;
    const frame = {
      isLastFrame,
      opcode,
      payload,
      mask: this.mask,
    };
    return this.enqueue(frame);
  }

  ping(data: WebSocketMessage = ""): Promise<void> {
    const payload = typeof data === "string" ? encode(data) : data;
    const frame = {
      isLastFrame: true,
      opcode: OpCode.Ping,
      mask: this.mask,
      payload,
    };
    return this.enqueue(frame);
  }

  private _isClosed = false;
  get isClosed(): boolean {
    return this._isClosed;
  }

  async close(code = 1000, reason?: string): Promise<void> {
    try {
      const header = [code >>> 8, code & 0x00ff];
      let payload: Uint8Array;
      if (reason) {
        const reasonBytes = encode(reason);
        payload = new Uint8Array(2 + reasonBytes.byteLength);
        payload.set(header);
        payload.set(reasonBytes, 2);
      } else {
        payload = new Uint8Array(header);
      }
      await this.enqueue({
        isLastFrame: true,
        opcode: OpCode.Close,
        mask: this.mask,
        payload,
      });
    } catch (e) {
      throw e;
    } finally {
      this.ensureSocketClosed();
    }
  }

  closeForce(): void {
    this.ensureSocketClosed();
  }

  private ensureSocketClosed(): void {
    if (this.isClosed) return;
    try {
      this.conn.close();
    } catch (e) {
      console.error(e);
    } finally {
      this._isClosed = true;
      const rest = this.sendQueue;
      this.sendQueue = [];
      rest.forEach((e) =>
        e.d.reject(
          new Deno.errors.ConnectionReset("Socket has already been closed")
        )
      );
    }
  }
}

/** Return whether given headers is acceptable for websocket  */
export function acceptable(req: { headers: Headers }): boolean {
  const upgrade = req.headers.get("upgrade");
  if (!upgrade || upgrade.toLowerCase() !== "websocket") {
    return false;
  }
  const secKey = req.headers.get("sec-websocket-key");
  return (
    req.headers.has("sec-websocket-key") &&
    typeof secKey === "string" &&
    secKey.length > 0
  );
}

const kGUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/** Create sec-websocket-accept header value with given nonce */
export function createSecAccept(nonce: string): string {
  const sha1 = new Sha1();
  sha1.update(nonce + kGUID);
  const bytes = sha1.digest();
  return btoa(String.fromCharCode(...bytes));
}

/** Upgrade given TCP connection into websocket connection */
export async function acceptWebSocket(req: {
  conn: Deno.Conn;
  bufWriter: BufWriter;
  bufReader: BufReader;
  headers: Headers;
}): Promise<WebSocket> {
  const { conn, headers, bufReader, bufWriter } = req;
  if (acceptable(req)) {
    const sock = new WebSocketImpl({ conn, bufReader, bufWriter });
    const secKey = headers.get("sec-websocket-key");
    if (typeof secKey !== "string") {
      throw new Error("sec-websocket-key is not provided");
    }
    const secAccept = createSecAccept(secKey);
    await writeResponse(bufWriter, {
      status: 101,
      headers: new Headers({
        Upgrade: "websocket",
        Connection: "Upgrade",
        "Sec-WebSocket-Accept": secAccept,
      }),
    });
    return sock;
  }
  throw new Error("request is not acceptable");
}

const kSecChars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-.~_";

/** Create WebSocket-Sec-Key. Base64 encoded 16 bytes string */
export function createSecKey(): string {
  let key = "";
  for (let i = 0; i < 16; i++) {
    const j = Math.floor(Math.random() * kSecChars.length);
    key += kSecChars[j];
  }
  return btoa(key);
}

export async function handshake(
  url: URL,
  headers: Headers,
  bufReader: BufReader,
  bufWriter: BufWriter
): Promise<void> {
  const { hostname, pathname, search } = url;
  const key = createSecKey();

  if (!headers.has("host")) {
    headers.set("host", hostname);
  }
  headers.set("upgrade", "websocket");
  headers.set("connection", "upgrade");
  headers.set("sec-websocket-key", key);
  headers.set("sec-websocket-version", "13");

  let headerStr = `GET ${pathname}${search} HTTP/1.1\r\n`;
  for (const [key, value] of headers) {
    headerStr += `${key}: ${value}\r\n`;
  }
  headerStr += "\r\n";

  await bufWriter.write(encode(headerStr));
  await bufWriter.flush();

  const tpReader = new TextProtoReader(bufReader);
  const statusLine = await tpReader.readLine();
  if (statusLine === null) {
    throw new Deno.errors.UnexpectedEof();
  }
  const m = statusLine.match(/^(?<version>\S+) (?<statusCode>\S+) /);
  if (!m) {
    throw new Error("ws: invalid status line: " + statusLine);
  }

  assert(m.groups);
  const { version, statusCode } = m.groups;
  if (version !== "HTTP/1.1" || statusCode !== "101") {
    throw new Error(
      `ws: server didn't accept handshake: ` +
        `version=${version}, statusCode=${statusCode}`
    );
  }

  const responseHeaders = await tpReader.readMIMEHeader();
  if (responseHeaders === null) {
    throw new Deno.errors.UnexpectedEof();
  }

  const expectedSecAccept = createSecAccept(key);
  const secAccept = responseHeaders.get("sec-websocket-accept");
  if (secAccept !== expectedSecAccept) {
    throw new Error(
      `ws: unexpected sec-websocket-accept header: ` +
        `expected=${expectedSecAccept}, actual=${secAccept}`
    );
  }
}

/**
 * Connect to given websocket endpoint url.
 * Endpoint must be acceptable for URL.
 */
export async function connectWebSocket(
  endpoint: string,
  headers: Headers = new Headers()
): Promise<WebSocket> {
  const url = new URL(endpoint);
  const { hostname } = url;
  let conn: Deno.Conn;
  if (url.protocol === "http:" || url.protocol === "ws:") {
    const port = parseInt(url.port || "80");
    conn = await Deno.connect({ hostname, port });
  } else if (url.protocol === "https:" || url.protocol === "wss:") {
    const port = parseInt(url.port || "443");
    conn = await Deno.connectTls({ hostname, port });
  } else {
    throw new Error("ws: unsupported protocol: " + url.protocol);
  }
  const bufWriter = new BufWriter(conn);
  const bufReader = new BufReader(conn);
  try {
    await handshake(url, headers, bufReader, bufWriter);
  } catch (err) {
    conn.close();
    throw err;
  }
  return new WebSocketImpl({
    conn,
    bufWriter,
    bufReader,
    mask: createMask(),
  });
}

export function createWebSocket(params: {
  conn: Deno.Conn;
  bufWriter?: BufWriter;
  bufReader?: BufReader;
  mask?: Uint8Array;
}): WebSocket {
  return new WebSocketImpl(params);
}
