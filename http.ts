import { log } from "./util";

import { pubInternal, sub } from "./dispatch";
import { main as pb } from "./msg.pb";

const servers = new Map<number, HttpServer>();
const dec = new TextDecoder("utf8");
const enc = new TextEncoder();

export function initHttp() {
  sub("http", (payload: Uint8Array) => {
    const msg = pb.Msg.decode(payload);
    const id = msg.httpServerId;
    const s = servers.get(id);
    s.onMsg(msg);
  });
}

export interface RequestOptions {
  method?: string;
  referrer?: string;
  mode?: string;
  credentials?: string;
  redirect?: string;
  integrity?: string;
  cache?: string;
}

export class Request {
  readonly defaultMethod = "GET";
  path: string;
  method: string;
  // tslint:disable-next-line: no-any
  body: any | string;
  constructor(url: string, opts?: RequestOptions) {
    if (opts == null) {
      opts = {
        method: this.defaultMethod
      };
    }
    this.method = opts.method;
  }
}

export class Response {
  serverId: number;
  channel: string;
  constructor() {}
  write(data: Uint8Array | string) {
    let rawData: Uint8Array;
    if (typeof data === "string") {
      rawData = enc.encode(data);
    } else {
      rawData = data as Uint8Array;
    }
    pubInternal(this.channel, {
      command: pb.Msg.Command.HTTP_RES_WRITE,
      httpResBody: rawData
    });
  }
  status(code: number) {
    pubInternal(this.channel, {
      command: pb.Msg.Command.HTTP_RES_STATUS,
      httpResCode: code
    });
  }
  end() {
    pubInternal(this.channel, {
      command: pb.Msg.Command.HTTP_RES_END
    });
  }
}

let nextServerId = 0;
export class HttpServer {
  private readonly id: number;
  private port: number;
  private cb: (req: Request, res: Response) => void;
  constructor(cb: (req: Request, res: Response) => void) {
    this.id = nextServerId++;
    this.cb = cb;
    servers.set(this.id, this);
    pubInternal("http", {
      command: pb.Msg.Command.HTTP_CREATE,
      httpServerId: this.id
    });
  }
  listen(port: number) {
    log("Starting server on", port);
    pubInternal("http", {
      command: pb.Msg.Command.HTTP_LISTEN,
      httpServerId: this.port,
      httpListenPort: port
    });
  }
  buildRequest(msg: pb.Msg) {
    const req = new Request("", {});
    req.path = msg.httpReqPath;
    req.method = msg.httpReqMethod;
    const rawBody = dec.decode(msg.httpReqBody);
    // Use a JSON body parser by default, fallback to string representation:
    try {
      const body = JSON.parse(rawBody);
      req.body = body;
    } catch (e) {
      req.body = rawBody.toString();
    }
    return req;
  }
  buildResponse(msg: pb.Msg) {
    const res = new Response();
    res.channel = `http/${msg.httpReqId}`;
    return res;
  }
  onMsg(msg: pb.Msg) {
    if (msg.command === pb.Msg.Command.HTTP_REQ) {
      const req = this.buildRequest(msg);
      const res = this.buildResponse(msg);
      this.cb(req, res);
    }
  }
}

export function createHttpServer(
  cb: (req: Request, res: Response) => void
): HttpServer {
  const s = new HttpServer(cb);
  return s;
}
