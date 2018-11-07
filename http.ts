import * as deno from "deno";
import * as bufio from "./bufio.ts";
import { TextProtoReader } from "./textproto.ts";

type Handler = (req: ServerRequest) => Promise<void>;

class Server {
  _closing = false;

  constructor(readonly listener: deno.Listener) {}

  async serve(handler: Handler) {
    while (!this._closing) {
      const c = await this.listener.accept();
      const sc = new ServerConn(c);
      sc.serve(handler);
    }
  }

  close() {
    this._closing = true;
    this.listener.close();
  }
}

class ServerConn {
  constructor(readonly c: deno.Conn) {
    // TODO Use memory pool to obtain bufr and bufw.
    this.bufr = new bufio.Reader(c);
    this.bufw = new bufio.Writer(c);
  }

  async serve(handler: Handler): Promise<void> {
    const buffer = new Uint8Array(1024);
    try {
      while (true) {
        const req = readRequest(this.bufr);

        /*
        const response = new TextEncoder().encode(
          "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
        );
        await this.c.write(response);
        */
      }
    } finally {
      this.c.close();
    }
  }
}

function readRequest(b: bufio.Reader): ServerRequest {
  const tp = new TextProtoReader(b);
  const req = new ServerRequest();

  // First line: GET /index.html HTTP/1.0
  const s = await tp.readLine();
  const [ method, url, proto ] = parseRequestLine(s);
  console.log("readRequest", method, url);
}

// Returns [method, url, proto]
function parseRequestLine(line: string): [ string, string, string ] {
  return line.split(" ", 3);
}

export function listen(addr: string): Server {
  const listener = deno.listen("tcp", addr);
  const s = new Server(listener);
  return s;
}

