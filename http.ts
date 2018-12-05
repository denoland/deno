import { listen, Conn } from "deno";
import { BufReader, BufState, BufWriter } from "./bufio.ts";
import { TextProtoReader } from "./textproto.ts";
import { STATUS_TEXT } from "./http_status";
import { assert } from "./util";

export async function* serve(addr: string) {
  const listener = listen("tcp", addr);
  while (true) {
    const c = await listener.accept();
    yield* serveConn(c);
  }
  listener.close();
}

export async function* serveConn(c: Conn) {
  let bufr = new BufReader(c);
  let bufw = new BufWriter(c);
  try {
    while (true) {
      const [req, err] = await readRequest(bufr);
      if (err == "EOF") {
        break;
      }
      if (err == "ShortWrite") {
        console.log("ShortWrite error");
        break;
      }
      if (err) {
        throw err;
      }
      req.w = bufw;
      yield req;
    }
  } finally {
    c.close();
  }
}

interface Response {
  status?: number;
  headers?: Headers;
  body?: Uint8Array;
}

function setContentLength(r: Response): void {
  if (r.body) {
    if (!r.headers) {
      r.headers = new Headers();
    }
    if (!r.headers.has("content-length")) {
      r.headers.append("Content-Length", r.body.byteLength.toString());
    }
  }
}

class ServerRequest {
  url: string;
  method: string;
  proto: string;
  headers: Headers;
  w: BufWriter;

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
      for (let [key, value] of r.headers) {
        out += `${key}: ${value}\r\n`;
      }
    }
    out += "\r\n";

    const header = new TextEncoder().encode(out);
    let n = await this.w.write(header);
    assert(header.byteLength == n);
    if (r.body) {
      n = await this.w.write(r.body);
      assert(r.body.byteLength == n);
    }

    await this.w.flush();
  }
}

async function readRequest(b: BufReader): Promise<[ServerRequest, BufState]> {
  const tp = new TextProtoReader(b);
  const req = new ServerRequest();

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
