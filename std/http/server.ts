// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { encode } from "../encoding/utf8.ts";
import { BufReader, BufWriter } from "../io/bufio.ts";
import { assert } from "../_util/assert.ts";
import { Deferred, deferred, MuxAsyncIterator } from "../async/mod.ts";
import {
  bodyReader,
  chunkedBodyReader,
  emptyReader,
  readRequest,
  writeResponse,
} from "./_io.ts";

export class ServerRequest {
  url!: string;
  method!: string;
  proto!: string;
  protoMinor!: number;
  protoMajor!: number;
  headers!: Headers;
  conn!: Deno.Conn;
  r!: BufReader;
  w!: BufWriter;

  #done: Deferred<Error | undefined> = deferred();
  #contentLength?: number | null = undefined;
  #body?: Deno.Reader = undefined;
  #finalized = false;

  get done(): Promise<Error | undefined> {
    return this.#done.then((e) => e);
  }

  /**
   * Value of Content-Length header.
   * If null, then content length is invalid or not given (e.g. chunked encoding).
   */
  get contentLength(): number | null {
    // undefined means not cached.
    // null means invalid or not provided.
    if (this.#contentLength === undefined) {
      const cl = this.headers.get("content-length");
      if (cl) {
        this.#contentLength = parseInt(cl);
        // Convert NaN to null (as NaN harder to test)
        if (Number.isNaN(this.#contentLength)) {
          this.#contentLength = null;
        }
      } else {
        this.#contentLength = null;
      }
    }
    return this.#contentLength;
  }

  /**
   * Body of the request.  The easiest way to consume the body is:
   *
   *     const buf: Uint8Array = await Deno.readAll(req.body);
   */
  get body(): Deno.Reader {
    if (!this.#body) {
      if (this.contentLength != null) {
        this.#body = bodyReader(this.contentLength, this.r);
      } else {
        const transferEncoding = this.headers.get("transfer-encoding");
        if (transferEncoding != null) {
          const parts = transferEncoding
            .split(",")
            .map((e): string => e.trim().toLowerCase());
          assert(
            parts.includes("chunked"),
            'transfer-encoding must include "chunked" if content-length is not set',
          );
          this.#body = chunkedBodyReader(this.headers, this.r);
        } else {
          // Neither content-length nor transfer-encoding: chunked
          this.#body = emptyReader();
        }
      }
    }
    return this.#body;
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
      } catch {
        // Pass
      }
      err = e;
    }
    // Signal that this request has been processed and the next pipelined
    // request on the same connection can be accepted.
    this.#done.resolve(err);
    if (err) {
      // Error during responding, rethrow.
      throw err;
    }
  }

  async finalize(): Promise<void> {
    if (this.#finalized) return;
    // Consume unread body
    const body = this.body;
    const buf = new Uint8Array(1024);
    while ((await body.read(buf)) !== null) {
      // Pass
    }
    this.#finalized = true;
  }
}

export class Server implements AsyncIterable<ServerRequest> {
  #closing = false;
  #connections: Deno.Conn[] = [];

  constructor(public listener: Deno.Listener) {}

  close(): void {
    this.#closing = true;
    this.listener.close();
    for (const conn of this.#connections) {
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
    conn: Deno.Conn,
  ): AsyncIterableIterator<ServerRequest> {
    const reader = new BufReader(conn);
    const writer = new BufWriter(conn);

    while (!this.#closing) {
      let request: ServerRequest | null;
      try {
        request = await readRequest(conn, reader);
      } catch (error) {
        if (
          error instanceof Deno.errors.InvalidData ||
          error instanceof Deno.errors.UnexpectedEof
        ) {
          // An error was thrown while parsing request headers.
          // Try to send the "400 Bad Request" before closing the connection.
          try {
            await writeResponse(writer, {
              status: 400,
              body: encode(`${error.message}\r\n\r\n`),
            });
          } catch (error) {
            // The connection is broken.
          }
        }
        break;
      }
      if (request === null) {
        break;
      }

      request.w = writer;
      yield request;

      // Wait for the request to be processed before we accept a new request on
      // this connection.
      const responseError = await request.done;
      if (responseError) {
        // Something bad happened during response.
        // (likely other side closed during pipelined req)
        // req.done implies this connection already closed, so we can just return.
        this.untrackConnection(request.conn);
        return;
      }

      try {
        // Consume unread body and trailers if receiver didn't consume those data
        await request.finalize();
      } catch (error) {
        // Invalid data was received or the connection was closed.
        break;
      }
    }

    this.untrackConnection(conn);
    try {
      conn.close();
    } catch (e) {
      // might have been already closed
    }
  }

  private trackConnection(conn: Deno.Conn): void {
    this.#connections.push(conn);
  }

  private untrackConnection(conn: Deno.Conn): void {
    const index = this.#connections.indexOf(conn);
    if (index !== -1) {
      this.#connections.splice(index, 1);
    }
  }

  // Accepts a new TCP connection and yields all HTTP requests that arrive on
  // it. When a connection is accepted, it also creates a new iterator of the
  // same kind and adds it to the request multiplexer so that another TCP
  // connection can be accepted.
  private async *acceptConnAndIterateHttpRequests(
    mux: MuxAsyncIterator<ServerRequest>,
  ): AsyncIterableIterator<ServerRequest> {
    if (this.#closing) return;
    // Wait for a new connection.
    let conn: Deno.Conn;
    try {
      conn = await this.listener.accept();
    } catch (error) {
      if (
        // The listener is closed:
        error instanceof Deno.errors.BadResource ||
        // TLS handshake errors:
        error instanceof Deno.errors.InvalidData ||
        error instanceof Deno.errors.UnexpectedEof ||
        error instanceof Deno.errors.ConnectionReset
      ) {
        return mux.add(this.acceptConnAndIterateHttpRequests(mux));
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

/** Options for creating an HTTP server. */
export type HTTPOptions = Omit<Deno.ListenOptions, "transport">;

/**
 * Parse addr from string
 *
 *     const addr = "::1:8000";
 *     parseAddrFromString(addr);
 *
 * @param addr Address string
 */
export function _parseAddrFromStr(addr: string): HTTPOptions {
  let url: URL;
  try {
    const host = addr.startsWith(":") ? `0.0.0.0${addr}` : addr;
    url = new URL(`http://${host}`);
  } catch {
    throw new TypeError("Invalid address.");
  }
  if (
    url.username ||
    url.password ||
    url.pathname != "/" ||
    url.search ||
    url.hash
  ) {
    throw new TypeError("Invalid address.");
  }

  return {
    hostname: url.hostname,
    port: url.port === "" ? 80 : Number(url.port),
  };
}

/**
 * Create a HTTP server
 *
 *     import { serve } from "https://deno.land/std/http/server.ts";
 *     const body = "Hello World\n";
 *     const server = serve({ port: 8000 });
 *     for await (const req of server) {
 *       req.respond({ body });
 *     }
 */
export function serve(addr: string | HTTPOptions): Server {
  if (typeof addr === "string") {
    addr = _parseAddrFromStr(addr);
  }

  const listener = Deno.listen(addr);
  return new Server(listener);
}

/**
 * Start an HTTP server with given options and request handler
 *
 *     const body = "Hello World\n";
 *     const options = { port: 8000 };
 *     listenAndServe(options, (req) => {
 *       req.respond({ body });
 *     });
 *
 * @param options Server configuration
 * @param handler Request handler
 */
export async function listenAndServe(
  addr: string | HTTPOptions,
  handler: (req: ServerRequest) => void,
): Promise<void> {
  const server = serve(addr);

  for await (const request of server) {
    handler(request);
  }
}

/** Options for creating an HTTPS server. */
export type HTTPSOptions = Omit<Deno.ListenTlsOptions, "transport">;

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
  const tlsOptions: Deno.ListenTlsOptions = {
    ...options,
    transport: "tcp",
  };
  const listener = Deno.listenTls(tlsOptions);
  return new Server(listener);
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
export async function listenAndServeTLS(
  options: HTTPSOptions,
  handler: (req: ServerRequest) => void,
): Promise<void> {
  const server = serveTLS(options);

  for await (const request of server) {
    handler(request);
  }
}

/**
 * Interface of HTTP server response.
 * If body is a Reader, response would be chunked.
 * If body is a string, it would be UTF-8 encoded by default.
 */
export interface Response {
  status?: number;
  headers?: Headers;
  body?: Uint8Array | Deno.Reader | string;
  trailers?: () => Promise<Headers> | Headers;
}
