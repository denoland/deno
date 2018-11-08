import { listen, Conn } from "deno";
import { BufReader, BufState } from "./bufio.ts";
import { TextProtoReader } from "./textproto.ts";
import { Headers } from "./headers.ts";

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
  try {
    while (true) {
      const req = await readRequest(bufr);
      yield req;
    }
  } finally {
    c.close();
  }
}

interface Response {
  status?: number;
  body: string;
}

class ServerRequest {
  url: string;
  method: string;
  proto: string;
  headers: Headers;

  respond(r: Response): Promise<void> {
    throw Error("not implemented");
  }
}

async function readRequest(b: BufReader): Promise<ServerRequest> {
  const tp = new TextProtoReader(b);
  const req = new ServerRequest();

  let s: string;
  let err: BufState;

  // First line: GET /index.html HTTP/1.0
  [s, err] = await tp.readLine();
  [req.method, req.url, req.proto] = s.split(" ", 3);

  [req.headers, err] = await tp.readMIMEHeader();

  return req;
}

