// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.
import { main as pb } from "./msg.pb";

declare module "deno" {
  type MessageCallback = (msg: Uint8Array) => void;
  function sub(channel: string, cb: MessageCallback): void;
  function pub(channel: string, payload: Uint8Array): null | ArrayBuffer;

  function readFileSync(filename: string): Uint8Array;
  function writeFileSync(
    filename: string,
    data: Uint8Array,
    perm: number
  ): void;

  interface RequestOptions {
    method?: string;
    url?: string;
    referrer?: string;
    mode?: string;
    credentials?: string;
    redirect?: string;
    integrity?: string;
    cache?: string;
  }

  // deno.Request Is used for incoming HTTP server requests
  // but ideally matches fetch's Request interface:
  // https://developer.mozilla.org/en-US/docs/Web/API/Request
  class Request {
    method: string;
    path: string;
    body: any | string;
    constructor(url: string, opts?: RequestOptions);
  }

  // deno.Response Is used for outgoing HTTP server responses
  // but ideally matches fetch's Response interface:
  // https://developer.mozilla.org/en-US/docs/Web/API/Response
  class Response {
    channel: string;
    constructor();
    write(data: Uint8Array | string): void;
    status(code: number): void;
    end(): void;
  }

  class HttpServer {
    private readonly id: number;
    private port: number;
    private cb: (req: Request, res: Response) => void;
    constructor(cb: (req: Request, res: Response) => void);
    listen(port: number): void;
    buildRequest(msg: pb.Msg): Request;
    buildResponse(msg: pb.Msg): Response;
    onMsg(msg: pb.Msg): void;
  }

  function createHttpServer(
    cb: (req: Request, res: Response) => void
  ): HttpServer;
}
