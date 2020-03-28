// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { BufReader, BufWriter } from "../io/bufio.ts";
import { assert } from "../testing/asserts.ts";
import {
  deferred,
  Deferred,
  MuxAsyncIterator,
  timeoutReader
} from "../util/async.ts";
import {
  bodyReader,
  chunkedBodyReader,
  emptyReader,
  writeResponse,
  readRequest,
  KeepAlive,
  parseKeepAlive,
  parseHTTPVersion
} from "./io.ts";
import Listener = Deno.Listener;
import Conn = Deno.Conn;
import Reader = Deno.Reader;
import { TimeUnits } from "../datetime/mod.ts";
const { listen, listenTLS } = Deno;

export class ServerRequest {
  constructor({
    method,
    url,
    proto,
    headers,
    conn,
    r,
    w,
    timeout
  }: {
    url: string;
    method: string;
    proto: string;
    headers: Headers;
    conn: Deno.Conn;
    r: BufReader;
    w: BufWriter;
    timeout?: number;
  }) {
    this.conn = conn;
    this.r = r;
    this.w = w;
    this.method = method;
    this.url = url;
    this.proto = proto;
    this.timeout = timeout;
    [this.protoMinor, this.protoMajor] = parseHTTPVersion(this.proto);
    this.headers = headers;
  }
  url: string;
  method: string;
  proto: string;
  protoMinor: number;
  protoMajor: number;
  headers: Headers;
  conn: Conn;
  r: BufReader;
  w: BufWriter;
  timeout?: number;
  done: Deferred<Error | undefined> = deferred();

  private _contentLength: number | undefined | null = undefined;
  /**
   * Value of Content-Length header.
   * If null, then content length is invalid or not given (e.g. chunked encoding).
   */
  get contentLength(): number | null {
    // undefined means not cached.
    // null means invalid or not provided.
    if (this._contentLength === undefined) {
      const cl = this.headers.get("content-length");
      if (cl) {
        this._contentLength = parseInt(cl);
        // Convert NaN to null (as NaN harder to test)
        if (Number.isNaN(this._contentLength)) {
          this._contentLength = null;
        }
      } else {
        this._contentLength = null;
      }
    }
    return this._contentLength;
  }

  private _body: Deno.Reader | null = null;

  /**  Body of the request */
  get body(): Deno.Reader {
    if (!this._body) {
      if (this.contentLength != null) {
        this._body = bodyReader(this.contentLength, this.r);
      } else {
        const transferEncoding = this.headers.get("transfer-encoding");
        if (transferEncoding != null) {
          const parts = transferEncoding
            .split(",")
            .map((e): string => e.trim().toLowerCase());
          assert(
            parts.includes("chunked"),
            'transfer-encoding must include "chunked" if content-length is not set'
          );
          this._body = chunkedBodyReader(this.headers, this.r);
        } else {
          // Neither content-length nor transfer-encoding: chunked
          this._body = emptyReader();
        }
      }
      if (this.timeout != null) {
        this._body = timeoutReader(this._body, this.timeout);
      }
    }
    return this._body;
  }

  async respond(r: Response): Promise<void> {
    let err: Error | undefined;
    try {
      // Write our response!
      await writeResponse(this.w, r);
    } catch (e) {
      try {
        // Eagerly close on error.
        this.conn.close();
      } catch {}
      err = e;
    }
    // Signal that this request has been processed and the next pipelined
    // request on the same connection can be accepted.
    this.done.resolve(err);
    if (err) {
      // Error during responding, rethrow.
      throw err;
    }
  }

  private finalized = false;
  async finalize(): Promise<void> {
    if (this.finalized) return;
    // Consume unread body
    const body = this.body;
    const buf = new Uint8Array(1024);
    while ((await body.read(buf)) !== Deno.EOF) {}
    this.finalized = true;
  }
}

export class Server implements AsyncIterable<ServerRequest> {
  private closing = false;
  private connections: Conn[] = [];
  readTimeout?: number;

  constructor(public listener: Listener, opts?: ServeOptions) {
    if (typeof opts?.readTimeout === "number") {
      assert(opts.readTimeout > 0, "readTimeout must be greater than zero");
      this.readTimeout = opts.readTimeout;
    }
  }

  close(): void {
    this.closing = true;
    this.listener.close();
    for (const conn of this.connections) {
      try {
        conn.close();
      } catch (e) {
        // Connection might have been already closed
        if (!(e instanceof Deno.errors.BadResource)) {
          throw e;
        }
      }
    }
  }

  // Yields all HTTP requests on a single TCP connection.
  private async *iterateHttpRequests(
    conn: Conn
  ): AsyncIterableIterator<ServerRequest> {
    const r = new BufReader(conn);
    const w = new BufWriter(conn);
    let req: ServerRequest | Deno.EOF;
    let keepAlive: KeepAlive | undefined;
    while (!this.closing) {
      try {
        let timeout = this.readTimeout;
        if (keepAlive?.timeout) {
          timeout = keepAlive.timeout * TimeUnits.seconds;
        }
        req = await readRequest(conn, { w, r, timeout });
      } catch (e) {
        break;
      }

      if (req === Deno.EOF) {
        break;
      }

      const ka = req.headers.get("keep-alive");
      if (ka) {
        keepAlive = parseKeepAlive(ka);
      }

      yield req;

      // Wait for the request to be processed before we accept a new request on
      // this connection.
      const procError = await req.done;
      if (procError) {
        // Something bad happened during response.
        // (likely other side closed during pipelined req)
        // req.done implies this connection already closed, so we can just return.
        this.untrackConnection(req.conn);
        return;
      }
      // Consume unread body and trailers if receiver didn't consume those data
      await req.finalize();
    }

    this.untrackConnection(conn);
    try {
      conn.close();
    } catch (e) {
      // might have been already closed
    }
  }

  private trackConnection(conn: Conn): void {
    this.connections.push(conn);
  }

  private untrackConnection(conn: Conn): void {
    const index = this.connections.indexOf(conn);
    if (index !== -1) {
      this.connections.splice(index, 1);
    }
  }

  // Accepts a new TCP connection and yields all HTTP requests that arrive on
  // it. When a connection is accepted, it also creates a new iterator of the
  // same kind and adds it to the request multiplexer so that another TCP
  // connection can be accepted.
  private async *acceptConnAndIterateHttpRequests(
    mux: MuxAsyncIterator<ServerRequest>
  ): AsyncIterableIterator<ServerRequest> {
    if (this.closing) return;
    // Wait for a new connection.
    let conn: Conn;
    try {
      conn = await this.listener.accept();
    } catch (error) {
      if (error instanceof Deno.errors.BadResource) {
        return;
      }
      throw error;
    }
    this.trackConnection(conn);
    // Try to accept another connection and add it to the multiplexer.
    mux.add(this.acceptConnAndIterateHttpRequests(mux));
    // Yield the requests that arrive on the just-accepted connection.
    yield* this.iterateHttpRequests(conn);
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<ServerRequest> {
    const mux: MuxAsyncIterator<ServerRequest> = new MuxAsyncIterator();
    mux.add(this.acceptConnAndIterateHttpRequests(mux));
    return mux.iterate();
  }
}

export type ServeOptions = {
  readTimeout?: number;
};
/** Options for creating an HTTP server. */
export type HTTPOptions = Omit<Deno.ListenOptions, "transport"> & ServeOptions;

/**
 * Create a HTTP server
 *
 *     import { serve } from "https://deno.land/std/http/server.ts";
 *     const body = "Hello World\n";
 *     const s = serve({ port: 8000 });
 *     for await (const req of s) {
 *       req.respond({ body });
 *     }
 */
export function serve(addr: string | HTTPOptions): Server {
  const serveOpts: ServeOptions = {};
  if (typeof addr === "string") {
    const [hostname, port] = addr.split(":");
    addr = { hostname, port: Number(port) };
  } else {
    serveOpts.readTimeout = addr.readTimeout;
  }
  const listener = listen(addr);
  return new Server(listener, serveOpts);
}

/**
 * Start an HTTP server with given options and request handler
 *
 *     const body = "Hello World\n";
 *     const options = { port: 8000 };
 *     listenAndServeTLS(options, (req) => {
 *       req.respond({ body });
 *     });
 *
 * @param options Server configuration
 * @param handler Request handler
 */
export function listenAndServe(
  addr: string | HTTPOptions,
  handler: (req: ServerRequest) => void
): [Server, Promise<void>] {
  const server = serve(addr);
  const p = (async (): Promise<void> => {
    for await (const req of server) {
      handler(req);
    }
  })();
  return [server, p];
}

/** Options for creating an HTTPS server. */
export type HTTPSOptions = Omit<Deno.ListenTLSOptions, "transport"> &
  ServeOptions;

/**
 * Create an HTTPS server with given options
 *
 *     const body = "Hello HTTPS";
 *     const options = {
 *       hostname: "localhost",
 *       port: 443,
 *       certFile: "./path/to/localhost.crt",
 *       keyFile: "./path/to/localhost.key",
 *     };
 *     for await (const req of serveTLS(options)) {
 *       req.respond({ body });
 *     }
 *
 * @param options Server configuration
 * @return Async iterable server instance for incoming requests
 */
export function serveTLS(options: HTTPSOptions): Server {
  const tlsOptions: Deno.ListenTLSOptions = {
    ...options,
    transport: "tcp",
  };
  const listener = listenTLS(tlsOptions);
  const serveOpts: ServeOptions = {
    readTimeout: options.readTimeout
  };
  return new Server(listener, serveOpts);
}

/**
 * Start an HTTPS server with given options and request handler
 *
 *     const body = "Hello HTTPS";
 *     const options = {
 *       hostname: "localhost",
 *       port: 443,
 *       certFile: "./path/to/localhost.crt",
 *       keyFile: "./path/to/localhost.key",
 *     };
 *     listenAndServeTLS(options, (req) => {
 *       req.respond({ body });
 *     });
 *
 * @param options Server configuration
 * @param handler Request handler
 */
export function listenAndServeTLS(
  options: HTTPSOptions,
  handler: (req: ServerRequest) => void
): [Server, Promise<void>] {
  const server = serve(options);
  const p = (async (): Promise<void> => {
    for await (const req of server) {
      handler(req);
    }
  })();
  return [server, p];
}

/**
 * Interface of HTTP server response.
 * If body is a Reader, response would be chunked.
 * If body is a string, it would be UTF-8 encoded by default.
 */
export interface Response {
  status?: number;
  headers?: Headers;
  body?: Uint8Array | Reader | string;
  trailers?: () => Promise<Headers> | Headers;
}
