// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { listen, copy, toAsyncIterator } = Deno;
type Listener = Deno.Listener;
type Conn = Deno.Conn;
type Reader = Deno.Reader;
type Writer = Deno.Writer;
import { BufReader, BufWriter, EOF, UnexpectedEOFError } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { STATUS_TEXT } from "./http_status.ts";
import { assert } from "../testing/asserts.ts";
import {
  collectUint8Arrays,
  deferred,
  Deferred,
  MuxAsyncIterator
} from "../util/async.ts";

function bufWriter(w: Writer): BufWriter {
  if (w instanceof BufWriter) {
    return w;
  } else {
    return new BufWriter(w);
  }
}

export function setContentLength(r: Response): void {
  if (!r.headers) {
    r.headers = new Headers();
  }

  if (r.body) {
    if (!r.headers.has("content-length")) {
      if (r.body instanceof Uint8Array) {
        const bodyLength = r.body.byteLength;
        r.headers.append("Content-Length", bodyLength.toString());
      } else {
        r.headers.append("Transfer-Encoding", "chunked");
      }
    }
  }
}

async function writeChunkedBody(w: Writer, r: Reader): Promise<void> {
  const writer = bufWriter(w);
  const encoder = new TextEncoder();

  for await (const chunk of toAsyncIterator(r)) {
    if (chunk.byteLength <= 0) continue;
    const start = encoder.encode(`${chunk.byteLength.toString(16)}\r\n`);
    const end = encoder.encode("\r\n");
    await writer.write(start);
    await writer.write(chunk);
    await writer.write(end);
  }

  const endChunk = encoder.encode("0\r\n\r\n");
  await writer.write(endChunk);
}

export async function writeResponse(w: Writer, r: Response): Promise<void> {
  const protoMajor = 1;
  const protoMinor = 1;
  const statusCode = r.status || 200;
  const statusText = STATUS_TEXT.get(statusCode);
  const writer = bufWriter(w);
  if (!statusText) {
    throw Error("bad status code");
  }
  if (!r.body) {
    r.body = new Uint8Array();
  }

  let out = `HTTP/${protoMajor}.${protoMinor} ${statusCode} ${statusText}\r\n`;

  setContentLength(r);
  const headers = r.headers!;

  for (const [key, value] of headers!) {
    out += `${key}: ${value}\r\n`;
  }
  out += "\r\n";

  const header = new TextEncoder().encode(out);
  const n = await writer.write(header);
  assert(n === header.byteLength);

  if (r.body instanceof Uint8Array) {
    const n = await writer.write(r.body);
    assert(n === r.body.byteLength);
  } else if (headers.has("content-length")) {
    const bodyLength = parseInt(headers.get("content-length")!);
    const n = await copy(writer, r.body);
    assert(n === bodyLength);
  } else {
    await writeChunkedBody(writer, r.body);
  }
  await writer.flush();
}

export class ServerRequest {
  url!: string;
  method!: string;
  proto!: string;
  protoMinor!: number;
  protoMajor!: number;
  headers!: Headers;
  r!: BufReader;
  w!: BufWriter;
  done: Deferred<void> = deferred();

  public async *bodyStream(): AsyncIterableIterator<Uint8Array> {
    if (this.headers.has("content-length")) {
      const len = +this.headers.get("content-length")!;
      if (Number.isNaN(len)) {
        return new Uint8Array(0);
      }
      let buf = new Uint8Array(1024);
      let rr = await this.r.read(buf);
      let nread = rr.nread;
      while (!rr.eof && nread < len) {
        yield buf.subarray(0, rr.nread);
        buf = new Uint8Array(1024);
        rr = await this.r.read(buf);
        nread += rr.nread;
      }
      yield buf.subarray(0, rr.nread);
    } else {
      if (this.headers.has("transfer-encoding")) {
        const transferEncodings = this.headers
          .get("transfer-encoding")!
          .split(",")
          .map((e): string => e.trim().toLowerCase());
        if (transferEncodings.includes("chunked")) {
          // Based on https://tools.ietf.org/html/rfc2616#section-19.4.6
          const tp = new TextProtoReader(this.r);
          let line = await tp.readLine();
          if (line === EOF) throw new UnexpectedEOFError();
          // TODO: handle chunk extension
          let [chunkSizeString] = line.split(";");
          let chunkSize = parseInt(chunkSizeString, 16);
          if (Number.isNaN(chunkSize) || chunkSize < 0) {
            throw new Error("Invalid chunk size");
          }
          while (chunkSize > 0) {
            const data = new Uint8Array(chunkSize);
            if ((await this.r.readFull(data)) === EOF) {
              throw new UnexpectedEOFError();
            }
            yield data;
            await this.r.readLine(); // Consume \r\n
            line = await tp.readLine();
            if (line === EOF) throw new UnexpectedEOFError();
            chunkSize = parseInt(line, 16);
          }
          const entityHeaders = await tp.readMIMEHeader();
          if (entityHeaders !== EOF) {
            for (let [k, v] of entityHeaders) {
              this.headers.set(k, v);
            }
          }
          /* Pseudo code from https://tools.ietf.org/html/rfc2616#section-19.4.6
          length := 0
          read chunk-size, chunk-extension (if any) and CRLF
          while (chunk-size > 0) {
            read chunk-data and CRLF
            append chunk-data to entity-body
            length := length + chunk-size
            read chunk-size and CRLF
          }
          read entity-header
          while (entity-header not empty) {
            append entity-header to existing header fields
            read entity-header
          }
          Content-Length := length
          Remove "chunked" from Transfer-Encoding
          */
          return; // Must return here to avoid fall through
        }
        // TODO: handle other transfer-encoding types
      }
      // Otherwise...
      yield new Uint8Array(0);
    }
  }

  // Read the body of the request into a single Uint8Array
  public async body(): Promise<Uint8Array> {
    return collectUint8Arrays(this.bodyStream());
  }

  async respond(r: Response): Promise<void> {
    // Write our response!
    await writeResponse(this.w, r);
    // Signal that this request has been processed and the next pipelined
    // request on the same connection can be accepted.
    this.done.resolve();
  }
}

function fixLength(req: ServerRequest): void {
  const contentLength = req.headers.get("Content-Length");
  if (contentLength) {
    const arrClen = contentLength.split(",");
    if (arrClen.length > 1) {
      const distinct = [...new Set(arrClen.map((e): string => e.trim()))];
      if (distinct.length > 1) {
        throw Error("cannot contain multiple Content-Length headers");
      } else {
        req.headers.set("Content-Length", distinct[0]);
      }
    }
    const c = req.headers.get("Content-Length");
    if (req.method === "HEAD" && c && c !== "0") {
      throw Error("http: method cannot contain a Content-Length");
    }
    if (c && req.headers.has("transfer-encoding")) {
      // A sender MUST NOT send a Content-Length header field in any message
      // that contains a Transfer-Encoding header field.
      // rfc: https://tools.ietf.org/html/rfc7230#section-3.3.2
      throw new Error(
        "http: Transfer-Encoding and Content-Length cannot be send together"
      );
    }
  }
}

// ParseHTTPVersion parses a HTTP version string.
// "HTTP/1.0" returns (1, 0, true).
// Ported from https://github.com/golang/go/blob/f5c43b9/src/net/http/request.go#L766-L792
export function parseHTTPVersion(vers: string): [number, number] {
  switch (vers) {
    case "HTTP/1.1":
      return [1, 1];

    case "HTTP/1.0":
      return [1, 0];

    default: {
      const Big = 1000000; // arbitrary upper bound
      const digitReg = /^\d+$/; // test if string is only digit
      let major: number;
      let minor: number;

      if (!vers.startsWith("HTTP/")) {
        break;
      }

      const dot = vers.indexOf(".");
      if (dot < 0) {
        break;
      }

      let majorStr = vers.substring(vers.indexOf("/") + 1, dot);
      major = parseInt(majorStr);
      if (
        !digitReg.test(majorStr) ||
        isNaN(major) ||
        major < 0 ||
        major > Big
      ) {
        break;
      }

      let minorStr = vers.substring(dot + 1);
      minor = parseInt(minorStr);
      if (
        !digitReg.test(minorStr) ||
        isNaN(minor) ||
        minor < 0 ||
        minor > Big
      ) {
        break;
      }

      return [major, minor];
    }
  }

  throw new Error(`malformed HTTP version ${vers}`);
}

export async function readRequest(
  bufr: BufReader
): Promise<ServerRequest | EOF> {
  const tp = new TextProtoReader(bufr);
  const firstLine = await tp.readLine(); // e.g. GET /index.html HTTP/1.0
  if (firstLine === EOF) return EOF;
  const headers = await tp.readMIMEHeader();
  if (headers === EOF) throw new UnexpectedEOFError();

  const req = new ServerRequest();
  req.r = bufr;
  [req.method, req.url, req.proto] = firstLine.split(" ", 3);
  [req.protoMinor, req.protoMajor] = parseHTTPVersion(req.proto);
  req.headers = headers;
  fixLength(req);
  return req;
}

export class Server implements AsyncIterable<ServerRequest> {
  private closing = false;

  constructor(public listener: Listener) {}

  close(): void {
    this.closing = true;
    this.listener.close();
  }

  // Yields all HTTP requests on a single TCP connection.
  private async *iterateHttpRequests(
    conn: Conn
  ): AsyncIterableIterator<ServerRequest> {
    const bufr = new BufReader(conn);
    const w = new BufWriter(conn);
    let req: ServerRequest | EOF;
    let err: Error | undefined;

    while (!this.closing) {
      try {
        req = await readRequest(bufr);
      } catch (e) {
        err = e;
        break;
      }
      if (req === EOF) {
        break;
      }

      req.w = w;
      yield req;

      // Wait for the request to be processed before we accept a new request on
      // this connection.
      await req!.done;
    }

    if (req! === EOF) {
      // The connection was gracefully closed.
    } else if (err) {
      // An error was thrown while parsing request headers.
      await writeResponse(req!.w, {
        status: 400,
        body: new TextEncoder().encode(`${err.message}\r\n\r\n`)
      });
    } else if (this.closing) {
      // There are more requests incoming but the server is closing.
      // TODO(ry): send a back a HTTP 503 Service Unavailable status.
    }

    conn.close();
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
    const conn = await this.listener.accept();
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

export function serve(addr: string): Server {
  const listener = listen("tcp", addr);
  return new Server(listener);
}

export async function listenAndServe(
  addr: string,
  handler: (req: ServerRequest) => void
): Promise<void> {
  const server = serve(addr);

  for await (const request of server) {
    handler(request);
  }
}

export interface Response {
  status?: number;
  headers?: Headers;
  body?: Uint8Array | Reader;
}
