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

  respond(r: Response = { status: 200 }): Promise<void> {
    throw Error("not implemented");
  }
}

async function readRequest(b: BufReader): Promise<ServerRequest> {
  const tp = new TextProtoReader(b);
  const req = new ServerRequest();

  // First line: GET /index.html HTTP/1.0
  let s: string;
  let err: BufState;
  [s, err] = await tp.readLine();
  const { method, url, proto } = parseRequestLine(s);
  req.method = method;
  req.url = url;
  req.proto = proto;

  let headers: Headers;
  [headers, err] = await tp.readMIMEHeader();

  return req;
}

// Returns [method, url, proto]
function parseRequestLine(
  line: string
): { method: string; url: string; proto: string } {
  let [method, url, proto] = line.split(" ", 3);
  return { method, url, proto };
}
