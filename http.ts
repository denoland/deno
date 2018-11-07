import * as deno from "deno";

class Server {
  _closing = false;

  constructor(readonly listener: deno.Listener) {}

  async serveConn(conn: deno.Conn) {
    const buffer = new Uint8Array(1024);
    try {
      while (true) {
        const r = await conn.read(buffer);
        if (r.eof) {
          break;
        }
        const response = new TextEncoder().encode(
          "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\nHello World\n"
        );
        await conn.write(response);
      }
    } finally {
      conn.close();
    }
  }

  async serve() {
    while (!this._closing) {
      const conn = await this.listener.accept();
      this.serveConn(conn);
    }
  }

  close() {
    this._closing = true;
  }
}

export function listen(addr: string): Server {
  const listener = deno.listen("tcp", addr);
  const s = new Server(listener);
  return s;
}
