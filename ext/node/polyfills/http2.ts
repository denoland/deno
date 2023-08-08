// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

const core = globalThis.Deno.core;
import { notImplemented, warnNotImplemented } from "ext:deno_node/_utils.ts";
import { EventEmitter } from "node:events";
import { Buffer } from "node:buffer";
import { Server, Socket, TCP } from "node:net";
import { TypedArray } from "ext:deno_node/internal/util/types.ts";
import { setStreamTimeout } from "ext:deno_node/internal/stream_base_commons.ts";
import { FileHandle } from "node:fs/promises";
import { kStreamBaseField } from "ext:deno_node/internal_binding/stream_wrap.ts";
import { addTrailers, serveHttpOnConnection } from "ext:deno_http/00_serve.js";
import { type Deferred, deferred } from "ext:deno_node/_util/async.ts";
import { nextTick } from "ext:deno_node/_next_tick.ts";
import { TextEncoder } from "ext:deno_web/08_text_encoding.js";
import { Duplex } from "node:stream";

const ENCODER = new TextEncoder();
type Http2Headers = Record<string, string | string[]>;

export class Http2Session extends EventEmitter {
  constructor() {
    super();
    this._connecting = false;
    this._closed = false;
  }

  get alpnProtocol(): string | undefined {
    notImplemented("Http2Session.alpnProtocol");
    return undefined;
  }

  close(_callback?: () => void) {
    // warnNotImplemented("Http2Session.close");
    this._closed = true;
    core.tryClose(this._connRid);
    core.tryClose(this._clientRid);
  }

  get closed(): boolean {
    return this._closed;
  }

  get connecting(): boolean {
    return this._connecting;
  }

  destroy(_error?: Error, _code?: number) {
    notImplemented("Http2Session.destroy");
  }

  get destroyed(): boolean {
    return false;
  }

  get encrypted(): boolean {
    notImplemented("Http2Session.encrypted");
    return false;
  }

  goaway(
    _code: number,
    _lastStreamID: number,
    _opaqueData: Buffer | TypedArray | DataView,
  ) {
    notImplemented("Http2Session.goaway");
  }

  get localSettings(): Record<string, unknown> {
    notImplemented("Http2Session.localSettings");
    return {};
  }

  get originSet(): string[] | undefined {
    notImplemented("Http2Session.originSet");
    return undefined;
  }

  get pendingSettingsAck(): boolean {
    notImplemented("Http2Session.pendingSettingsAck");
    return false;
  }

  ping(
    _payload: Buffer | TypedArray | DataView,
    _callback: () => void,
  ): boolean {
    notImplemented("Http2Session.ping");
    return false;
  }

  ref() {
    warnNotImplemented("Http2Session.ref");
  }

  get remoteSettings(): Record<string, unknown> {
    notImplemented("Http2Session.remoteSettings");
    return {};
  }

  setLocalWindowSize(_windowSize: number) {
    notImplemented("Http2Session.setLocalWindowSize");
  }

  setTimeout(msecs: number, callback?: () => void) {
    setStreamTimeout(this, msecs, callback);
  }

  get socket(): Socket /*| TlsSocket*/ {
    console.error("NOT IMPLEMENTED: Http2Session.socket", new Error());
    return {};
  }

  get state(): Record<string, unknown> {
    return {};
  }

  settings(_settings: Record<string, unknown>, _callback: () => void) {
    notImplemented("Http2Session.settings");
  }

  get type(): number {
    notImplemented("Http2Session.type");
    return 0;
  }

  unref() {
    warnNotImplemented("Http2Session.unref");
  }
}

export class ServerHttp2Session extends Http2Session {
  constructor() {
    super();
  }

  altsvc(
    _alt: string,
    _originOrStream: number | string | URL | { origin: string },
  ) {
    notImplemented("ServerHttp2Session.altsvc");
  }

  origin(..._origins: (string | URL | { origin: string })[]) {
    notImplemented("ServerHttp2Session.origins");
  }
}

export class ClientHttp2Session extends Http2Session {
  #connectPromise: Promise<void>;
  _clientRid: number;

  constructor(
    authority: string | URL,
    options: Record<string, unknown>,
  ) {
    super();
    this.#connectPromise = (async () => {
      this._connecting = true;
      // console.log("before connect");
      const [clientRid, connRid] = await core.opAsync(
        "op_http2_connect",
        authority,
      );
      // console.log("after connect");
      this._connecting = false;
      this._clientRid = clientRid;
      this._connRid = connRid;
      // TODO(bartlomieju): save so it can be unrefed
      (async () => {
        try {
          await core.opAsync("op_http2_poll_client_connection", this._connRid);
        } catch (e) {
          console.error("Error in op_http2_poll_client_connection", e);
          this.emit("error", e);
        }
      })();
      this.emit("connect", this, {});
    })();
  }

  request(
    headers: Http2Headers,
    options?: Record<string, unknown>,
  ): ClientHttp2Stream {
    const reqHeaders: string[][] = [];
    let waitForTrailers = false;

    options ??= {};
    if (options.waitForTrailers) {
      waitForTrailers = true;
    }

    if (options.signal) {
      notImplemented("http2.ClientHttp2Session.request.options.signal");
    }

    const pseudoHeaders = {};

    for (const [name, value] of Object.entries(headers)) {
      if (name == constants.HTTP2_HEADER_PATH) {
        pseudoHeaders[constants.HTTP2_HEADER_PATH] = String(value);
      } else if (name == constants.HTTP2_HEADER_METHOD) {
        pseudoHeaders[constants.HTTP2_HEADER_METHOD] = String(value);
      } else if (name == constants.HTTP2_HEADER_AUTHORITY) {
        pseudoHeaders[constants.HTTP2_HEADER_AUTHORITY] = String(value);
      } else {
        reqHeaders.push([name, String(value)]);
      }
    }

    return new ClientHttp2Stream(
      options,
      this,
      this.#connectPromise,
      pseudoHeaders,
      reqHeaders,
    );
  }
}

export class Http2Stream extends EventEmitter {
  #session: Http2Session;
  #headers: Deferred<Http2Headers>;
  #controllerPromise: Deferred<ReadableStreamDefaultController<Uint8Array>>;
  #readerPromise: Deferred<ReadableStream<Uint8Array>>;
  #closed: boolean;
  _response: Response;

  constructor(
    session: Http2Session,
    headers: Promise<Http2Headers>,
    controllerPromise: Promise<ReadableStreamDefaultController<Uint8Array>>,
    readerPromise: Promise<ReadableStream<Uint8Array>>,
  ) {
    super();
    this.#session = session;
    this.#headers = headers;
    this.#controllerPromise = controllerPromise;
    this.#readerPromise = readerPromise;
    this.#closed = false;
    nextTick(() => {
      (async () => {
        const headers = await this.#headers;
        this.emit("headers", headers);
      })();
      (async () => {
        const reader = await this.#readerPromise;
        if (reader) {
          for await (const data of reader) {
            this.emit("data", new Buffer(data));
          }
        }
        this.emit("end");
      })();
    });
  }

  // TODO(mmastrac): Implement duplex
  end() {
    (async () => {
      const controller = await this.#controllerPromise;
      controller.close();
    })();
  }

  write(buffer, callback?: () => void) {
    (async () => {
      const controller = await this.#controllerPromise;
      if (typeof buffer === "string") {
        controller.enqueue(ENCODER.encode(buffer));
      } else {
        controller.enqueue(buffer);
      }
      callback?.();
    })();
  }

  setEncoding(_encoding) {}

  resume() {
  }

  pause() {
  }

  get aborted(): boolean {
    notImplemented("Http2Stream.aborted");
    return false;
  }

  get bufferSize(): number {
    notImplemented("Http2Stream.bufferSize");
    return 0;
  }

  close(_code: number, _callback: () => void) {
    this.#closed = true;
    this.emit("close");
  }

  get closed(): boolean {
    return this.#closed;
  }

  get destroyed(): boolean {
    return false;
  }

  get endAfterHeaders(): boolean {
    notImplemented("Http2Stream.endAfterHeaders");
    return false;
  }

  get id(): number | undefined {
    notImplemented("Http2Stream.id");
    return undefined;
  }

  get pending(): boolean {
    notImplemented("Http2Stream.pending");
    return false;
  }

  priority(_options: Record<string, unknown>) {
    notImplemented("Http2Stream.priority");
  }

  get rstCode(): number {
    // notImplemented("Http2Stream.rstCode");
    return 0;
  }

  get sentHeaders(): boolean {
    notImplemented("Http2Stream.sentHeaders");
    return false;
  }

  get sentInfoHeaders(): Record<string, unknown> {
    notImplemented("Http2Stream.sentInfoHeaders");
    return {};
  }

  get sentTrailers(): Record<string, unknown> {
    notImplemented("Http2Stream.sentTrailers");
    return {};
  }

  get session(): Http2Session {
    return this.#session;
  }

  setTimeout(msecs: number, callback?: () => void) {
    setStreamTimeout(this, msecs, callback);
  }

  get state(): Record<string, unknown> {
    notImplemented("Http2Stream.state");
    return {};
  }

  sendTrailers(_headers: Record<string, unknown>) {
    addTrailers(this._response, [["grpc-status", "0"], ["grpc-message", "OK"]]);
  }
}

export class ClientHttp2Stream extends Duplex {
  #session: Http2Session;
  #requestPromise: Promise<[number, number]>;
  #responsePromise: Promise<void>;
  #closed: boolean = false;
  #rid: number | undefined = undefined;
  #id: number | undefined = undefined;
  #bodyRid = 0;
  #sentTrailers = Object.create(null);
  #waitForTrailers = false;
  #hasTrailersToRead = false;
  #encoding = "utf8";

  constructor(
    options: Record<string, unknown>,
    session: Http2Session,
    sessionConnectPromise: Promise<void>,
    pseudoHeaders: Record<string, string>,
    reqHeaders: string[][],
  ) {
    options.allowHalfOpen = true;
    options.decodeString = false;
    options.autoDestroy = false;
    super(options);
    this.cork();

    this.#requestPromise = (async () => {
      // console.log("waiting for connect promise");
      await sessionConnectPromise;
      // console.log("waited for connect promise", !!options.waitForTrailers);
      return await core.opAsync(
        "op_http2_client_request",
        session._clientRid,
        pseudoHeaders,
        reqHeaders,
        // TODO: waitForTrailers should be handled more gracefully
        false,
      );
    })();
    this.#session = session;
    this.#waitForTrailers = !!options.waitForTrailers;
    // console.log("created clienthttp2stream");
    this.#responsePromise = (async () => {
      // console.log("before request promise");
      const [streamRid, streamId] = await this.#requestPromise;
      this.#rid = streamRid;
      this.#id = streamId;
      console.log("after request promise", this.#rid);
      const response = await core.opAsync(
        "op_http2_client_get_response",
        this.#rid,
      );
      console.log("after get response", response);
      this.#bodyRid = response.bodyRid;
      const headers = {
        ":status": response.statusCode,
        ...Object.fromEntries(response.headers),
      };
      console.log("emitting response", headers);
      this.emit("response", headers, 0);
      this.__response = response;
      // this.uncork();
      this.emit("ready");
    })();
  }

  // TODO(mmastrac): Implement duplex
  // end(...args) {
  //   console.log("stream end", args);
  //   // console.log(this.#bodyRid, this.#rid, this.#session._connRid);
  //   // core.tryClose(this.#bodyRid);
  //   // core.tryClose(this.#rid);
  //   // core.tryClose(this.#session._connRid);
  // }

  _read() {
    if (!this.__response) {
      this.once("ready", this._read);
      return;
    }

    console.log("read");
    if (this.#hasTrailersToRead) {
      (async () => {
        // console.log("before trailers to read");
        const trailerList = await core.opAsync(
          "op_http2_client_get_response_trailers",
          response.bodyRid,
        );
        // console.log("after trailers to read");
        // console.log("trailers", trailerList);
        const trailers = Object.fromEntries(trailerList);
        // core.close(response.bodyRid);
        // core.close(clientRid);
        // core.tryClose(connRid);
        this.emit("trailers", trailers);
      })();

      // this.#closed = true;
      // // console.log(this.#bodyRid, this.#rid);
      // core.tryClose(this.#bodyRid);
      // core.tryClose(this.#rid);
      // this.emit("end");
    }

    (async () => {
      const [chunk, finished] = await core.opAsync(
        "op_http2_client_get_response_body_chunk",
        this.__response.bodyRid,
      );

      // console.log("chunk", chunk, finished);
      if (chunk === null) {
        if (finished) {
          this.#hasTrailersToRead = true;
          this.push(null);
          this.emit("end");
          return;
        }
        const trailerList = await core.opAsync(
          "op_http2_client_get_response_trailers",
          this.__response.bodyRid,
        );
        // console.log("after trailers to read");
        // console.log("trailers", trailerList);
        const trailers = Object.fromEntries(trailerList);
        // core.close(response.bodyRid);
        // core.close(clientRid);
        // core.tryClose(connRid);
        console.log("emitting trailers", trailers);
        this.push(null);
        this.emit("trailers", trailers);
        this.emit("end");
        return;
      }

      if (this.#encoding === "utf8") {
        this.push(new TextDecoder().decode(new Uint8Array(chunk)));
      } else {
        this.push(new Uint8Array(chunk));
      }
    })();
  }

  write(chunk, encoding, callback) {
    this._write(chunk, encoding, callback);
  }

  _write(chunk, encoding, callback?: () => void) {
    if (typeof encoding === "function") {
      callback = encoding;
      encoding = "utf8";
    }
    console.log("buffer", this.#rid, chunk, encoding, callback);
    (async () => {
      await this.#requestPromise;
      console.log("buffer", this.#rid, chunk, callback);

      let data;
      if (typeof encoding === "string") {
        data = ENCODER.encode(chunk);
      } else {
        data = chunk.buffer;
      }

      try {
        await core.opAsync(
          "op_http2_client_send_data",
          this.#rid,
          data,
        );
      } catch (e) {
        callback?.(e);
        return;
      }

      // console.log("after write");
      callback?.();
    })();
  }

  _writev(chunks, callback?: () => {}) {
    notImplemented("ClientHttp2Stream._writev");
  }

  _destroy(err, callback) {
    // TODO: handle error
    console.log("ClientHttp2Stream._destroy", err, callback);
    (async () => {
      await core.opAsync(
        "op_http2_client_reset_stream",
        this.#rid,
      );
      callback?.();
    })();
  }

  // end(...args) {
  //   console.log("end called", args);
  // }

  _final(cb) {
    console.error("Not implemented: ClientHttp2Stream._final");
    // notImplemented("ClientHttp2Stream._final");
    if (this.#waitForTrailers) {
      if (!this.emit("wantTrailers")) {
        this.sendTrailers({});
      }
    }
    (async () => {
      await core.opAsync(
        "op_http2_client_end_stream",
        this.#rid,
      );
      cb();
      // this._destroy();
    })();
  }

  get aborted(): boolean {
    notImplemented("Http2Stream.aborted");
    return false;
  }

  get bufferSize(): number {
    notImplemented("Http2Stream.bufferSize");
    return 0;
  }

  close(_code: number, _callback: () => void) {
    console.error("Stream close");
    // notImplemented("ClientHttp2Stream.end");
    this.#closed = true;
    this.emit("close");
  }

  get closed(): boolean {
    return this.#closed;
  }

  get destroyed(): boolean {
    return false;
  }

  get endAfterHeaders(): boolean {
    notImplemented("Http2Stream.endAfterHeaders");
    return false;
  }

  get id(): number | undefined {
    return this.#id;
  }

  get pending(): boolean {
    return this.#id === undefined;
  }

  priority(_options: Record<string, unknown>) {
    notImplemented("Http2Stream.priority");
  }

  get rstCode(): number {
    console.log("rstCode", new Error());
    // notImplemented("Http2Stream.rstCode");
    return 0;
  }

  get sentHeaders(): boolean {
    notImplemented("Http2Stream.sentHeaders");
    return false;
  }

  get sentInfoHeaders(): Record<string, unknown> {
    notImplemented("Http2Stream.sentInfoHeaders");
    return {};
  }

  get sentTrailers(): Record<string, unknown> {
    return this.#sentTrailers;
  }

  get session(): Http2Session {
    return this.#session;
  }

  setTimeout(msecs: number, callback?: () => void) {
    // setStreamTimeout(this, msecs, callback);
    notImplemented("ClientHttp2Stream.setTimeout");
  }

  get state(): Record<string, unknown> {
    notImplemented("Http2Stream.state");
    return {};
  }

  sendTrailers(trailers: Record<string, unknown>) {
    // TODO: handle __proto__ null
    this.#sentTrailers = { ...trailers, __proto__: null };
    const trailerList = [];
    for (const [key, value] of Object.entries(trailers)) {
      trailerList.push([key, value]);
    }

    (async () => {
      await core.opAsync(
        "op_http2_client_send_trailers",
        this.#rid,
        trailerList,
      );
    })();
  }
}

export class ServerHttp2Stream extends Http2Stream {
  _promise: Deferred<Response>;
  #body: ReadableStream<Uint8Array>;
  #waitForTrailers: boolean;
  #headersSent: boolean;

  constructor(
    session: Http2Session,
    headers: Promise<Http2Headers>,
    controllerPromise: Promise<ReadableStreamDefaultController<Uint8Array>>,
    reader: ReadableStream<Uint8Array>,
    body: ReadableStream<Uint8Array>,
  ) {
    super(session, headers, controllerPromise, Promise.resolve(reader));
    this._promise = new deferred();
    this.#body = body;
  }

  additionalHeaders(_headers: Record<string, unknown>) {
    notImplemented("ServerHttp2Stream.additionalHeaders");
  }

  end(): void {
    super.end();
    if (this.#waitForTrailers) {
      this.emit("wantTrailers");
    }
  }

  get headersSent(): boolean {
    return this.#headersSent;
  }

  get pushAllowed(): boolean {
    notImplemented("ServerHttp2Stream.pushAllowed");
    return false;
  }

  pushStream(
    _headers: Record<string, unknown>,
    _options: Record<string, unknown>,
    _callback: () => unknown,
  ) {
    notImplemented("ServerHttp2Stream.pushStream");
  }

  respond(
    headers: Http2Headers,
    options: Record<string, unknown>,
  ) {
    this.#headersSent = true;
    const response: ResponseInit = {};
    if (headers) {
      for (const [name, value] of Object.entries(headers)) {
        if (name == constants.HTTP2_HEADER_STATUS) {
          response.status = Number(value);
        }
      }
    }
    if (options?.endStream) {
      this._promise.resolve(this._response = new Response("", response));
    } else {
      this.#waitForTrailers = options?.waitForTrailers;
      this._promise.resolve(
        this._response = new Response(this.#body, response),
      );
    }
  }

  respondWithFD(
    _fd: number | FileHandle,
    _headers: Record<string, unknown>,
    _options: Record<string, unknown>,
  ) {
    notImplemented("ServerHttp2Stream.respondWithFD");
  }

  respondWithFile(
    _path: string | Buffer | URL,
    _headers: Record<string, unknown>,
    _options: Record<string, unknown>,
  ) {
    notImplemented("ServerHttp2Stream.respondWithFile");
  }
}

export class Http2Server extends Server {
  #options: Record<string, unknown> = {};
  #abortController;
  #server;
  timeout = 0;

  constructor(
    options: Record<string, unknown>,
    requestListener: () => unknown,
  ) {
    super(options);
    this.#abortController = new AbortController();
    this.on(
      "connection",
      (conn: Deno.Conn) => {
        try {
          const session = new ServerHttp2Session();
          this.emit("session", session);
          this.#server = serveHttpOnConnection(
            conn,
            this.#abortController.signal,
            async (req: Request) => {
              try {
                const controllerPromise: Deferred<
                  ReadableStreamDefaultController<Uint8Array>
                > = deferred();
                const body = new ReadableStream({
                  start(controller) {
                    controllerPromise.resolve(controller);
                  },
                });
                const headers: Http2Headers = {};
                for (const [name, value] of req.headers) {
                  headers[name] = value;
                }
                headers[constants.HTTP2_HEADER_PATH] =
                  new URL(req.url).pathname;
                const stream = new ServerHttp2Stream(
                  session,
                  Promise.resolve(headers),
                  controllerPromise,
                  req.body,
                  body,
                );
                session.emit("stream", stream, headers);
                this.emit("stream", stream, headers);
                return await stream._promise;
              } catch (e) {
                console.log("Error in serveHttpOnConnection", e);
              }
              return new Response("");
            },
            () => {
              console.log("error");
            },
            () => {},
          );
        } catch (e) {
          console.log("Error in Http2Server", e);
        }
      },
    );
    this.on(
      "newListener",
      (event) => console.log(`Event in newListener: ${event}`),
    );
    this.#options = options;
    if (typeof requestListener === "function") {
      this.on("request", requestListener);
    }
  }

  // Prevent the TCP server from wrapping this in a socket, since we need it to serve HTTP
  _createSocket(clientHandle: TCP) {
    return clientHandle[kStreamBaseField];
  }

  close(callback?: () => unknown) {
    if (callback) {
      this.on("close", callback);
    }
    this.#abortController.abort();
    super.close();
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.timeout = msecs;
    if (callback !== undefined) {
      this.on("timeout", callback);
    }
  }

  updateSettings(settings: Record<string, unknown>) {
    this.#options.settings = { ...this.#options.settings, ...settings };
  }
}

export class Http2SecureServer extends Server {
  #options: Record<string, unknown> = {};
  timeout = 0;

  constructor(
    options: Record<string, unknown>,
    requestListener: () => unknown,
  ) {
    super(options, function () {
      notImplemented("connectionListener");
    });
    this.#options = options;
    if (typeof requestListener === "function") {
      this.on("request", requestListener);
    }
  }

  close(_callback?: () => unknown) {
    notImplemented("Http2SecureServer.close");
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.timeout = msecs;
    if (callback !== undefined) {
      this.on("timeout", callback);
    }
  }

  updateSettings(settings: Record<string, unknown>) {
    this.#options.settings = { ...this.#options.settings, ...settings };
  }
}

export function createServer(
  options: Record<string, unknown>,
  onRequestHandler: () => unknown,
): Http2Server {
  if (typeof options === "function") {
    onRequestHandler = options;
    options = {};
  }
  return new Http2Server(options, onRequestHandler);
}

export function createSecureServer(
  _options: Record<string, unknown>,
  _onRequestHandler: () => unknown,
): Http2SecureServer {
  notImplemented("http2.createSecureServer");
  return new Http2SecureServer();
}

export function connect(
  authority: string | URL,
  options: Record<string, unknown>,
  callback: (session: ClientHttp2Session) => void,
): ClientHttp2Session {
  console.log("http2.connect", options);

  if (typeof options === "function") {
    callback = options;
    options = undefined;
  }

  options = { ...options };

  // TODO: handle defaults
  if (typeof options.createConnection === "function") {
    console.error("Not implemented: http2.connect.options.createConnection");
    // notImplemented("http2.connect.options.createConnection");
  }

  const session = new ClientHttp2Session(authority, options);

  if (typeof callback === "function") {
    session.once("connect", callback);
  }
  return session;
}

export const constants = {
  NGHTTP2_ERR_FRAME_SIZE_ERROR: -522,
  NGHTTP2_SESSION_SERVER: 0,
  NGHTTP2_SESSION_CLIENT: 1,
  NGHTTP2_STREAM_STATE_IDLE: 1,
  NGHTTP2_STREAM_STATE_OPEN: 2,
  NGHTTP2_STREAM_STATE_RESERVED_LOCAL: 3,
  NGHTTP2_STREAM_STATE_RESERVED_REMOTE: 4,
  NGHTTP2_STREAM_STATE_HALF_CLOSED_LOCAL: 5,
  NGHTTP2_STREAM_STATE_HALF_CLOSED_REMOTE: 6,
  NGHTTP2_STREAM_STATE_CLOSED: 7,
  NGHTTP2_FLAG_NONE: 0,
  NGHTTP2_FLAG_END_STREAM: 1,
  NGHTTP2_FLAG_END_HEADERS: 4,
  NGHTTP2_FLAG_ACK: 1,
  NGHTTP2_FLAG_PADDED: 8,
  NGHTTP2_FLAG_PRIORITY: 32,
  DEFAULT_SETTINGS_HEADER_TABLE_SIZE: 4096,
  DEFAULT_SETTINGS_ENABLE_PUSH: 1,
  DEFAULT_SETTINGS_MAX_CONCURRENT_STREAMS: 4294967295,
  DEFAULT_SETTINGS_INITIAL_WINDOW_SIZE: 65535,
  DEFAULT_SETTINGS_MAX_FRAME_SIZE: 16384,
  DEFAULT_SETTINGS_MAX_HEADER_LIST_SIZE: 65535,
  DEFAULT_SETTINGS_ENABLE_CONNECT_PROTOCOL: 0,
  MAX_MAX_FRAME_SIZE: 16777215,
  MIN_MAX_FRAME_SIZE: 16384,
  MAX_INITIAL_WINDOW_SIZE: 2147483647,
  NGHTTP2_SETTINGS_HEADER_TABLE_SIZE: 1,
  NGHTTP2_SETTINGS_ENABLE_PUSH: 2,
  NGHTTP2_SETTINGS_MAX_CONCURRENT_STREAMS: 3,
  NGHTTP2_SETTINGS_INITIAL_WINDOW_SIZE: 4,
  NGHTTP2_SETTINGS_MAX_FRAME_SIZE: 5,
  NGHTTP2_SETTINGS_MAX_HEADER_LIST_SIZE: 6,
  NGHTTP2_SETTINGS_ENABLE_CONNECT_PROTOCOL: 8,
  PADDING_STRATEGY_NONE: 0,
  PADDING_STRATEGY_ALIGNED: 1,
  PADDING_STRATEGY_MAX: 2,
  PADDING_STRATEGY_CALLBACK: 1,
  NGHTTP2_NO_ERROR: 0,
  NGHTTP2_PROTOCOL_ERROR: 1,
  NGHTTP2_INTERNAL_ERROR: 2,
  NGHTTP2_FLOW_CONTROL_ERROR: 3,
  NGHTTP2_SETTINGS_TIMEOUT: 4,
  NGHTTP2_STREAM_CLOSED: 5,
  NGHTTP2_FRAME_SIZE_ERROR: 6,
  NGHTTP2_REFUSED_STREAM: 7,
  NGHTTP2_CANCEL: 8,
  NGHTTP2_COMPRESSION_ERROR: 9,
  NGHTTP2_CONNECT_ERROR: 10,
  NGHTTP2_ENHANCE_YOUR_CALM: 11,
  NGHTTP2_INADEQUATE_SECURITY: 12,
  NGHTTP2_HTTP_1_1_REQUIRED: 13,
  NGHTTP2_DEFAULT_WEIGHT: 16,
  HTTP2_HEADER_STATUS: ":status",
  HTTP2_HEADER_METHOD: ":method",
  HTTP2_HEADER_AUTHORITY: ":authority",
  HTTP2_HEADER_SCHEME: ":scheme",
  HTTP2_HEADER_PATH: ":path",
  HTTP2_HEADER_PROTOCOL: ":protocol",
  HTTP2_HEADER_ACCEPT_ENCODING: "accept-encoding",
  HTTP2_HEADER_ACCEPT_LANGUAGE: "accept-language",
  HTTP2_HEADER_ACCEPT_RANGES: "accept-ranges",
  HTTP2_HEADER_ACCEPT: "accept",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_CREDENTIALS:
    "access-control-allow-credentials",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_HEADERS: "access-control-allow-headers",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_METHODS: "access-control-allow-methods",
  HTTP2_HEADER_ACCESS_CONTROL_ALLOW_ORIGIN: "access-control-allow-origin",
  HTTP2_HEADER_ACCESS_CONTROL_EXPOSE_HEADERS: "access-control-expose-headers",
  HTTP2_HEADER_ACCESS_CONTROL_REQUEST_HEADERS: "access-control-request-headers",
  HTTP2_HEADER_ACCESS_CONTROL_REQUEST_METHOD: "access-control-request-method",
  HTTP2_HEADER_AGE: "age",
  HTTP2_HEADER_AUTHORIZATION: "authorization",
  HTTP2_HEADER_CACHE_CONTROL: "cache-control",
  HTTP2_HEADER_CONNECTION: "connection",
  HTTP2_HEADER_CONTENT_DISPOSITION: "content-disposition",
  HTTP2_HEADER_CONTENT_ENCODING: "content-encoding",
  HTTP2_HEADER_CONTENT_LENGTH: "content-length",
  HTTP2_HEADER_CONTENT_TYPE: "content-type",
  HTTP2_HEADER_COOKIE: "cookie",
  HTTP2_HEADER_DATE: "date",
  HTTP2_HEADER_ETAG: "etag",
  HTTP2_HEADER_FORWARDED: "forwarded",
  HTTP2_HEADER_HOST: "host",
  HTTP2_HEADER_IF_MODIFIED_SINCE: "if-modified-since",
  HTTP2_HEADER_IF_NONE_MATCH: "if-none-match",
  HTTP2_HEADER_IF_RANGE: "if-range",
  HTTP2_HEADER_LAST_MODIFIED: "last-modified",
  HTTP2_HEADER_LINK: "link",
  HTTP2_HEADER_LOCATION: "location",
  HTTP2_HEADER_RANGE: "range",
  HTTP2_HEADER_REFERER: "referer",
  HTTP2_HEADER_SERVER: "server",
  HTTP2_HEADER_SET_COOKIE: "set-cookie",
  HTTP2_HEADER_STRICT_TRANSPORT_SECURITY: "strict-transport-security",
  HTTP2_HEADER_TRANSFER_ENCODING: "transfer-encoding",
  HTTP2_HEADER_TE: "te",
  HTTP2_HEADER_UPGRADE_INSECURE_REQUESTS: "upgrade-insecure-requests",
  HTTP2_HEADER_UPGRADE: "upgrade",
  HTTP2_HEADER_USER_AGENT: "user-agent",
  HTTP2_HEADER_VARY: "vary",
  HTTP2_HEADER_X_CONTENT_TYPE_OPTIONS: "x-content-type-options",
  HTTP2_HEADER_X_FRAME_OPTIONS: "x-frame-options",
  HTTP2_HEADER_KEEP_ALIVE: "keep-alive",
  HTTP2_HEADER_PROXY_CONNECTION: "proxy-connection",
  HTTP2_HEADER_X_XSS_PROTECTION: "x-xss-protection",
  HTTP2_HEADER_ALT_SVC: "alt-svc",
  HTTP2_HEADER_CONTENT_SECURITY_POLICY: "content-security-policy",
  HTTP2_HEADER_EARLY_DATA: "early-data",
  HTTP2_HEADER_EXPECT_CT: "expect-ct",
  HTTP2_HEADER_ORIGIN: "origin",
  HTTP2_HEADER_PURPOSE: "purpose",
  HTTP2_HEADER_TIMING_ALLOW_ORIGIN: "timing-allow-origin",
  HTTP2_HEADER_X_FORWARDED_FOR: "x-forwarded-for",
  HTTP2_HEADER_PRIORITY: "priority",
  HTTP2_HEADER_ACCEPT_CHARSET: "accept-charset",
  HTTP2_HEADER_ACCESS_CONTROL_MAX_AGE: "access-control-max-age",
  HTTP2_HEADER_ALLOW: "allow",
  HTTP2_HEADER_CONTENT_LANGUAGE: "content-language",
  HTTP2_HEADER_CONTENT_LOCATION: "content-location",
  HTTP2_HEADER_CONTENT_MD5: "content-md5",
  HTTP2_HEADER_CONTENT_RANGE: "content-range",
  HTTP2_HEADER_DNT: "dnt",
  HTTP2_HEADER_EXPECT: "expect",
  HTTP2_HEADER_EXPIRES: "expires",
  HTTP2_HEADER_FROM: "from",
  HTTP2_HEADER_IF_MATCH: "if-match",
  HTTP2_HEADER_IF_UNMODIFIED_SINCE: "if-unmodified-since",
  HTTP2_HEADER_MAX_FORWARDS: "max-forwards",
  HTTP2_HEADER_PREFER: "prefer",
  HTTP2_HEADER_PROXY_AUTHENTICATE: "proxy-authenticate",
  HTTP2_HEADER_PROXY_AUTHORIZATION: "proxy-authorization",
  HTTP2_HEADER_REFRESH: "refresh",
  HTTP2_HEADER_RETRY_AFTER: "retry-after",
  HTTP2_HEADER_TRAILER: "trailer",
  HTTP2_HEADER_TK: "tk",
  HTTP2_HEADER_VIA: "via",
  HTTP2_HEADER_WARNING: "warning",
  HTTP2_HEADER_WWW_AUTHENTICATE: "www-authenticate",
  HTTP2_HEADER_HTTP2_SETTINGS: "http2-settings",
  HTTP2_METHOD_ACL: "ACL",
  HTTP2_METHOD_BASELINE_CONTROL: "BASELINE-CONTROL",
  HTTP2_METHOD_BIND: "BIND",
  HTTP2_METHOD_CHECKIN: "CHECKIN",
  HTTP2_METHOD_CHECKOUT: "CHECKOUT",
  HTTP2_METHOD_CONNECT: "CONNECT",
  HTTP2_METHOD_COPY: "COPY",
  HTTP2_METHOD_DELETE: "DELETE",
  HTTP2_METHOD_GET: "GET",
  HTTP2_METHOD_HEAD: "HEAD",
  HTTP2_METHOD_LABEL: "LABEL",
  HTTP2_METHOD_LINK: "LINK",
  HTTP2_METHOD_LOCK: "LOCK",
  HTTP2_METHOD_MERGE: "MERGE",
  HTTP2_METHOD_MKACTIVITY: "MKACTIVITY",
  HTTP2_METHOD_MKCALENDAR: "MKCALENDAR",
  HTTP2_METHOD_MKCOL: "MKCOL",
  HTTP2_METHOD_MKREDIRECTREF: "MKREDIRECTREF",
  HTTP2_METHOD_MKWORKSPACE: "MKWORKSPACE",
  HTTP2_METHOD_MOVE: "MOVE",
  HTTP2_METHOD_OPTIONS: "OPTIONS",
  HTTP2_METHOD_ORDERPATCH: "ORDERPATCH",
  HTTP2_METHOD_PATCH: "PATCH",
  HTTP2_METHOD_POST: "POST",
  HTTP2_METHOD_PRI: "PRI",
  HTTP2_METHOD_PROPFIND: "PROPFIND",
  HTTP2_METHOD_PROPPATCH: "PROPPATCH",
  HTTP2_METHOD_PUT: "PUT",
  HTTP2_METHOD_REBIND: "REBIND",
  HTTP2_METHOD_REPORT: "REPORT",
  HTTP2_METHOD_SEARCH: "SEARCH",
  HTTP2_METHOD_TRACE: "TRACE",
  HTTP2_METHOD_UNBIND: "UNBIND",
  HTTP2_METHOD_UNCHECKOUT: "UNCHECKOUT",
  HTTP2_METHOD_UNLINK: "UNLINK",
  HTTP2_METHOD_UNLOCK: "UNLOCK",
  HTTP2_METHOD_UPDATE: "UPDATE",
  HTTP2_METHOD_UPDATEREDIRECTREF: "UPDATEREDIRECTREF",
  HTTP2_METHOD_VERSION_CONTROL: "VERSION-CONTROL",
  HTTP_STATUS_CONTINUE: 100,
  HTTP_STATUS_SWITCHING_PROTOCOLS: 101,
  HTTP_STATUS_PROCESSING: 102,
  HTTP_STATUS_EARLY_HINTS: 103,
  HTTP_STATUS_OK: 200,
  HTTP_STATUS_CREATED: 201,
  HTTP_STATUS_ACCEPTED: 202,
  HTTP_STATUS_NON_AUTHORITATIVE_INFORMATION: 203,
  HTTP_STATUS_NO_CONTENT: 204,
  HTTP_STATUS_RESET_CONTENT: 205,
  HTTP_STATUS_PARTIAL_CONTENT: 206,
  HTTP_STATUS_MULTI_STATUS: 207,
  HTTP_STATUS_ALREADY_REPORTED: 208,
  HTTP_STATUS_IM_USED: 226,
  HTTP_STATUS_MULTIPLE_CHOICES: 300,
  HTTP_STATUS_MOVED_PERMANENTLY: 301,
  HTTP_STATUS_FOUND: 302,
  HTTP_STATUS_SEE_OTHER: 303,
  HTTP_STATUS_NOT_MODIFIED: 304,
  HTTP_STATUS_USE_PROXY: 305,
  HTTP_STATUS_TEMPORARY_REDIRECT: 307,
  HTTP_STATUS_PERMANENT_REDIRECT: 308,
  HTTP_STATUS_BAD_REQUEST: 400,
  HTTP_STATUS_UNAUTHORIZED: 401,
  HTTP_STATUS_PAYMENT_REQUIRED: 402,
  HTTP_STATUS_FORBIDDEN: 403,
  HTTP_STATUS_NOT_FOUND: 404,
  HTTP_STATUS_METHOD_NOT_ALLOWED: 405,
  HTTP_STATUS_NOT_ACCEPTABLE: 406,
  HTTP_STATUS_PROXY_AUTHENTICATION_REQUIRED: 407,
  HTTP_STATUS_REQUEST_TIMEOUT: 408,
  HTTP_STATUS_CONFLICT: 409,
  HTTP_STATUS_GONE: 410,
  HTTP_STATUS_LENGTH_REQUIRED: 411,
  HTTP_STATUS_PRECONDITION_FAILED: 412,
  HTTP_STATUS_PAYLOAD_TOO_LARGE: 413,
  HTTP_STATUS_URI_TOO_LONG: 414,
  HTTP_STATUS_UNSUPPORTED_MEDIA_TYPE: 415,
  HTTP_STATUS_RANGE_NOT_SATISFIABLE: 416,
  HTTP_STATUS_EXPECTATION_FAILED: 417,
  HTTP_STATUS_TEAPOT: 418,
  HTTP_STATUS_MISDIRECTED_REQUEST: 421,
  HTTP_STATUS_UNPROCESSABLE_ENTITY: 422,
  HTTP_STATUS_LOCKED: 423,
  HTTP_STATUS_FAILED_DEPENDENCY: 424,
  HTTP_STATUS_TOO_EARLY: 425,
  HTTP_STATUS_UPGRADE_REQUIRED: 426,
  HTTP_STATUS_PRECONDITION_REQUIRED: 428,
  HTTP_STATUS_TOO_MANY_REQUESTS: 429,
  HTTP_STATUS_REQUEST_HEADER_FIELDS_TOO_LARGE: 431,
  HTTP_STATUS_UNAVAILABLE_FOR_LEGAL_REASONS: 451,
  HTTP_STATUS_INTERNAL_SERVER_ERROR: 500,
  HTTP_STATUS_NOT_IMPLEMENTED: 501,
  HTTP_STATUS_BAD_GATEWAY: 502,
  HTTP_STATUS_SERVICE_UNAVAILABLE: 503,
  HTTP_STATUS_GATEWAY_TIMEOUT: 504,
  HTTP_STATUS_HTTP_VERSION_NOT_SUPPORTED: 505,
  HTTP_STATUS_VARIANT_ALSO_NEGOTIATES: 506,
  HTTP_STATUS_INSUFFICIENT_STORAGE: 507,
  HTTP_STATUS_LOOP_DETECTED: 508,
  HTTP_STATUS_BANDWIDTH_LIMIT_EXCEEDED: 509,
  HTTP_STATUS_NOT_EXTENDED: 510,
  HTTP_STATUS_NETWORK_AUTHENTICATION_REQUIRED: 511,
};

export function getDefaultSettings(): Record<string, unknown> {
  notImplemented("http2.getDefaultSettings");
  return {};
}

export function getPackedSettings(_settings: Record<string, unknown>): Buffer {
  notImplemented("http2.getPackedSettings");
  return {};
}

export function getUnpackedSettings(
  _buffer: Buffer | TypedArray,
): Record<string, unknown> {
  notImplemented("http2.getUnpackedSettings");
  return {};
}

export const sensitiveHeaders = Symbol("nodejs.http2.sensitiveHeaders");

export class Http2ServerRequest {
  constructor() {
  }

  get aborted(): boolean {
    notImplemented("Http2ServerRequest.aborted");
    return false;
  }

  get authority(): string {
    notImplemented("Http2ServerRequest.authority");
    return "";
  }

  get complete(): boolean {
    notImplemented("Http2ServerRequest.complete");
    return false;
  }

  get connection(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerRequest.connection");
    return {};
  }

  destroy(_error: Error) {
    notImplemented("Http2ServerRequest.destroy");
  }

  get headers(): Record<string, unknown> {
    notImplemented("Http2ServerRequest.headers");
    return {};
  }

  get httpVersion(): string {
    notImplemented("Http2ServerRequest.httpVersion");
    return "";
  }

  get method(): string {
    notImplemented("Http2ServerRequest.method");
    return "";
  }

  get rawHeaders(): string[] {
    notImplemented("Http2ServerRequest.rawHeaders");
    return [];
  }

  get rawTrailers(): string[] {
    notImplemented("Http2ServerRequest.rawTrailers");
    return [];
  }

  get scheme(): string {
    notImplemented("Http2ServerRequest.scheme");
    return "";
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.stream.setTimeout(callback, msecs);
  }

  get socket(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerRequest.socket");
    return {};
  }

  get stream(): Http2Stream {
    notImplemented("Http2ServerRequest.stream");
    return new Http2Stream();
  }

  get trailers(): Record<string, unknown> {
    notImplemented("Http2ServerRequest.trailers");
    return {};
  }

  get url(): string {
    notImplemented("Http2ServerRequest.url");
    return "";
  }
}

export class Http2ServerResponse {
  constructor() {
  }

  addTrailers(_headers: Record<string, unknown>) {
    notImplemented("Http2ServerResponse.addTrailers");
  }

  get connection(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerResponse.connection");
    return {};
  }

  createPushResponse(
    _headers: Record<string, unknown>,
    _callback: () => unknown,
  ) {
    notImplemented("Http2ServerResponse.createPushResponse");
  }

  end(
    _data: string | Buffer | Uint8Array,
    _encoding: string,
    _callback: () => unknown,
  ) {
    notImplemented("Http2ServerResponse.end");
  }

  get finished(): boolean {
    notImplemented("Http2ServerResponse.finished");
    return false;
  }

  getHeader(_name: string): string {
    notImplemented("Http2ServerResponse.getHeader");
    return "";
  }

  getHeaderNames(): string[] {
    notImplemented("Http2ServerResponse.getHeaderNames");
    return [];
  }

  getHeaders(): Record<string, unknown> {
    notImplemented("Http2ServerResponse.getHeaders");
    return {};
  }

  hasHeader(_name: string) {
    notImplemented("Http2ServerResponse.hasHeader");
  }

  get headersSent(): boolean {
    notImplemented("Http2ServerResponse.headersSent");
    return false;
  }

  removeHeader(_name: string) {
    notImplemented("Http2ServerResponse.removeHeader");
  }

  get req(): Http2ServerRequest {
    notImplemented("Http2ServerResponse.req");
    return new Http2ServerRequest();
  }

  get sendDate(): boolean {
    notImplemented("Http2ServerResponse.sendDate");
    return false;
  }

  setHeader(_name: string, _value: string | string[]) {
    notImplemented("Http2ServerResponse.setHeader");
  }

  setTimeout(msecs: number, callback?: () => unknown) {
    this.stream.setTimeout(msecs, callback);
  }

  get socket(): Socket /*| TlsSocket*/ {
    notImplemented("Http2ServerResponse.socket");
    return {};
  }

  get statusCode(): number {
    notImplemented("Http2ServerResponse.statusCode");
    return 0;
  }

  get statusMessage(): string {
    notImplemented("Http2ServerResponse.statusMessage");
    return "";
  }

  get stream(): Http2Stream {
    notImplemented("Http2ServerResponse.stream");
    return new Http2Stream();
  }

  get writableEnded(): boolean {
    notImplemented("Http2ServerResponse.writableEnded");
    return false;
  }

  write(
    _chunk: string | Buffer | Uint8Array,
    _encoding: string,
    _callback: () => unknown,
  ) {
    notImplemented("Http2ServerResponse.write");
    return this.write;
  }

  writeContinue() {
    notImplemented("Http2ServerResponse.writeContinue");
  }

  writeEarlyHints(_hints: Record<string, unknown>) {
    notImplemented("Http2ServerResponse.writeEarlyHints");
  }

  writeHead(
    _statusCode: number,
    _statusMessage: string,
    _headers: Record<string, unknown>,
  ) {
    notImplemented("Http2ServerResponse.writeHead");
  }
}

export default {
  createServer,
  createSecureServer,
  connect,
  constants,
  getDefaultSettings,
  getPackedSettings,
  getUnpackedSettings,
  sensitiveHeaders,
  Http2ServerRequest,
  Http2ServerResponse,
};
