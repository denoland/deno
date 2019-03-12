// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { listen, toAsyncIterator, copy } = Deno;
type Conn = Deno.Conn;
type Reader = Deno.Reader;
type Writer = Deno.Writer;
import { BufReader, BufState, BufWriter } from "../io/bufio.ts";
import { TextProtoReader } from "../textproto/mod.ts";
import { STATUS_TEXT } from "./http_status.ts";
import { assert } from "../testing/asserts.ts";

interface Deferred {
  promise: Promise<{}>;
  resolve: () => void;
  reject: () => void;
}
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

  let out = `HTTP/${protoMajor}.${protoMinor} ${statusCode} ${statusText}\r\n`;

  setContentLength(r);

  if (r.headers) {
    for (const [key, value] of r.headers) {
      out += `${key}: ${value}\r\n`;
    }
  }
  out += "\r\n";

  const header = new TextEncoder().encode(out);
  let n = await writer.write(header);
  assert(header.byteLength == n);

  if (r.body) {
    if (r.body instanceof Uint8Array) {
      n = await writer.write(r.body);
      assert(r.body.byteLength == n);
    } else {
      if (r.headers.has("content-length")) {
        const bodyLength = parseInt(r.headers.get("content-length"));
        const n = await copy(writer, r.body);
        assert(n == bodyLength);
      } else {
        await writeChunkedBody(writer, r.body);
      }
    }
  }
  await writer.flush();
}
async function readAllIterator(
  it: AsyncIterableIterator<Uint8Array>
): Promise<Uint8Array> {
  const chunks = [];
  let len = 0;
  for await (const chunk of it) {
    chunks.push(chunk);
    len += chunk.length;
  }
  if (chunks.length === 0) {
    // No need for copy
    return chunks[0];
  }
  const collected = new Uint8Array(len);
  let offset = 0;
  for (let chunk of chunks) {
    collected.set(chunk, offset);
    offset += chunk.length;
  }
  return collected;
}

export class ServerRequest {
  url: string;
  method: string;
  proto: string;
  headers: Headers;
  conn: Conn;
  r: BufReader;
  w: BufWriter;

  public async *bodyStream(): AsyncIterableIterator<Uint8Array> {
    if (this.headers.has("content-length")) {
      const len = +this.headers.get("content-length");
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
          .get("transfer-encoding")
          .split(",")
          .map(e => e.trim().toLowerCase());
        if (transferEncodings.includes("chunked")) {
          // Based on https://tools.ietf.org/html/rfc2616#section-19.4.6
          const tp = new TextProtoReader(this.r);
          let [line] = await tp.readLine();
          // TODO: handle chunk extension
          let [chunkSizeString] = line.split(";");
          let chunkSize = parseInt(chunkSizeString, 16);
          if (Number.isNaN(chunkSize) || chunkSize < 0) {
            throw new Error("Invalid chunk size");
          }
          while (chunkSize > 0) {
            let data = new Uint8Array(chunkSize);
            let [nread] = await this.r.readFull(data);
            if (nread !== chunkSize) {
              throw new Error("Chunk data does not match size");
            }
            yield data;
            await this.r.readLine(); // Consume \r\n
            [line] = await tp.readLine();
            chunkSize = parseInt(line, 16);
          }
          const [entityHeaders, err] = await tp.readMIMEHeader();
          if (!err) {
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
    return readAllIterator(this.bodyStream());
  }

  async respond(r: Response): Promise<void> {
    return writeResponse(this.w, r);
  }
}

function deferred(): Deferred {
  let resolve, reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return {
    promise,
    resolve,
    reject
  };
}

interface ServeEnv {
  reqQueue: ServerRequest[];
  serveDeferred: Deferred;
}

/** Continuously read more requests from conn until EOF
 * Calls maybeHandleReq.
 * bufr is empty on a fresh TCP connection.
 * Would be passed around and reused for later request on same conn
 * TODO: make them async function after this change is done
 * https://github.com/tc39/ecma262/pull/1250
 * See https://v8.dev/blog/fast-async
 */
async function readRequest(
  c: Conn,
  bufr?: BufReader
): Promise<[ServerRequest, BufState]> {
  if (!bufr) {
    bufr = new BufReader(c);
  }
  const bufw = new BufWriter(c);
  const req = new ServerRequest();
  req.conn = c;
  req.r = bufr!;
  req.w = bufw;
  const tp = new TextProtoReader(bufr!);

  let s: string;
  let err: BufState;

  // First line: GET /index.html HTTP/1.0
  [s, err] = await tp.readLine();
  if (err) {
    return [null, err];
  }
  [req.method, req.url, req.proto] = s.split(" ", 3);

  [req.headers, err] = await tp.readMIMEHeader();

  return [req, err];
}

function maybeHandleReq(
  env: ServeEnv,
  conn: Conn,
  maybeReq: [ServerRequest, BufState]
): void {
  const [req, _err] = maybeReq;
  if (_err) {
    conn.close(); // assume EOF for now...
    return;
  }
  env.reqQueue.push(req); // push req to queue
  env.serveDeferred.resolve(); // signal while loop to process it
}

function serveConn(env: ServeEnv, conn: Conn, bufr?: BufReader): void {
  readRequest(conn, bufr).then(maybeHandleReq.bind(null, env, conn));
}

export async function* serve(
  addr: string
): AsyncIterableIterator<ServerRequest> {
  const listener = listen("tcp", addr);
  const env: ServeEnv = {
    reqQueue: [], // in case multiple promises are ready
    serveDeferred: deferred()
  };

  // Routine that keeps calling accept
  let handleConn = (_conn: Conn): void => {};
  let scheduleAccept = (): void => {};
  const acceptRoutine = (): void => {
    scheduleAccept = () => {
      listener.accept().then(handleConn);
    };
    handleConn = (conn: Conn) => {
      serveConn(env, conn); // don't block
      scheduleAccept(); // schedule next accept
    };

    scheduleAccept();
  };

  acceptRoutine();

  // Loop hack to allow yield (yield won't work in callbacks)
  while (true) {
    await env.serveDeferred.promise;
    env.serveDeferred = deferred(); // use a new deferred
    let queueToProcess = env.reqQueue;
    env.reqQueue = [];
    for (const result of queueToProcess) {
      yield result;
      // Continue read more from conn when user is done with the current req
      // Moving this here makes it easier to manage
      serveConn(env, result.conn, result.r);
    }
  }
  listener.close();
}

export async function listenAndServe(
  addr: string,
  handler: (req: ServerRequest) => void
): Promise<void> {
  const server = serve(addr);

  for await (const request of server) {
    await handler(request);
  }
}

export interface Response {
  status?: number;
  headers?: Headers;
  body?: Uint8Array | Reader;
}
