import { listen, Conn, toAsyncIterator, Reader, copy } from "deno";
import { BufReader, BufState, BufWriter } from "./bufio.ts";
import { TextProtoReader } from "./textproto.ts";
import { STATUS_TEXT } from "./http_status";
import { assert } from "./util";

interface Deferred {
  promise: Promise<{}>;
  resolve: () => void;
  reject: () => void;
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

// Continuously read more requests from conn until EOF
// Mutually calling with maybeHandleReq
// TODO: make them async function after this change is done
// https://github.com/tc39/ecma262/pull/1250
// See https://v8.dev/blog/fast-async
export function serveConn(env: ServeEnv, conn: Conn) {
  readRequest(conn).then(maybeHandleReq.bind(null, env, conn));
}
function maybeHandleReq(env: ServeEnv, conn: Conn, maybeReq: any) {
  const [req, _err] = maybeReq;
  if (_err) {
    conn.close(); // assume EOF for now...
    return;
  }
  env.reqQueue.push(req); // push req to queue
  env.serveDeferred.resolve(); // signal while loop to process it
  // TODO: protection against client req flooding
  serveConn(env, conn); // try read more (reusing connection)
}

export async function* serve(addr: string) {
  const listener = listen("tcp", addr);
  const env: ServeEnv = {
    reqQueue: [], // in case multiple promises are ready
    serveDeferred: deferred()
  };

  // Routine that keeps calling accept
  const acceptRoutine = () => {
    const handleConn = (conn: Conn) => {
      serveConn(env, conn); // don't block
      scheduleAccept(); // schedule next accept
    };
    const scheduleAccept = () => {
      listener.accept().then(handleConn);
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
    }
  }
  listener.close();
}

export async function listenAndServe(
  addr: string,
  handler: (req: ServerRequest) => void
) {
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

export class ServerRequest {
  url: string;
  method: string;
  proto: string;
  headers: Headers;
  w: BufWriter;

  private async _streamBody(body: Reader, bodyLength: number) {
    const n = await copy(this.w, body);
    assert(n == bodyLength);
  }

  private async _streamChunkedBody(body: Reader) {
    const encoder = new TextEncoder();

    for await (const chunk of toAsyncIterator(body)) {
      const start = encoder.encode(`${chunk.byteLength.toString(16)}\r\n`);
      const end = encoder.encode("\r\n");
      await this.w.write(start);
      await this.w.write(chunk);
      await this.w.write(end);
    }

    const endChunk = encoder.encode("0\r\n\r\n");
    await this.w.write(endChunk);
  }

  async respond(r: Response): Promise<void> {
    const protoMajor = 1;
    const protoMinor = 1;
    const statusCode = r.status || 200;
    const statusText = STATUS_TEXT.get(statusCode);
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
    let n = await this.w.write(header);
    assert(header.byteLength == n);

    if (r.body) {
      if (r.body instanceof Uint8Array) {
        n = await this.w.write(r.body);
        assert(r.body.byteLength == n);
      } else {
        if (r.headers.has("content-length")) {
          await this._streamBody(
            r.body,
            parseInt(r.headers.get("content-length"))
          );
        } else {
          await this._streamChunkedBody(r.body);
        }
      }
    }

    await this.w.flush();
  }
}

async function readRequest(c: Conn): Promise<[ServerRequest, BufState]> {
  const bufr = new BufReader(c);
  const bufw = new BufWriter(c);
  const req = new ServerRequest();
  req.w = bufw;
  const tp = new TextProtoReader(bufr);

  let s: string;
  let err: BufState;

  // First line: GET /index.html HTTP/1.0
  [s, err] = await tp.readLine();
  if (err) {
    return [null, err];
  }
  [req.method, req.url, req.proto] = s.split(" ", 3);

  [req.headers, err] = await tp.readMIMEHeader();

  // TODO: handle body

  return [req, err];
}
