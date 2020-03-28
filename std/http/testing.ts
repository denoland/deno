import { ServerRequest } from "./server.ts";
import { BufReader, BufWriter } from "../io/bufio.ts";

/** Create dummy Deno.Conn object with given base properties */
export function mockConn(base: Partial<Deno.Conn> = {}): Deno.Conn {
  return {
    localAddr: {
      transport: "tcp",
      hostname: "",
      port: 0
    },
    remoteAddr: {
      transport: "tcp",
      hostname: "",
      port: 0
    },
    rid: -1,
    closeRead: (): void => {},
    closeWrite: (): void => {},
    read: (): Promise<number | Deno.EOF> => {
      return Promise.resolve(0);
    },
    write: (): Promise<number> => {
      return Promise.resolve(-1);
    },
    close: (): void => {},
    ...base
  };
}

export function mockRequest(p: Partial<ServerRequest> = {}): ServerRequest {
  const { method, url, proto, headers, r, w } = p;
  const conn = p.conn ?? mockConn();
  const req = new ServerRequest({
    method: method ?? "GET",
    url: url ?? "/",
    proto: proto ?? "HTTP/1.1",
    headers: headers ?? new Headers(),
    conn: conn ?? mockConn(),
    r: r ?? new BufReader(conn),
    w: w ?? new BufWriter(conn)
  });
  return req;
}
