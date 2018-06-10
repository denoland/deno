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

  export class NetSocket {
    private connectCb: () => void;
    private onDataCb: (data: Uint8Array) => void;
    constructor();
    connect(port: number, address: string, cb: () => void): void;
    write(data: Uint8Array | string): void;
    onData(cb: (data: Uint8Array) => void): void;
    onMsg(msg: pb.Msg): void;
  }

  function Socket(): NetSocket;

  export class NetServerConn {
    private readonly id: number;
    private onDataCb: (data: Uint8Array) => void;
    constructor()
    write(data: Uint8Array | string): void;
    close(): void;
    onData(cb: (data: Uint8Array) => void): void;
    onMsg(msg: pb.Msg): void;
  }


  export class NetServer {
    private readonly id: number;
    private connectCb: (conn: NetServerConn) => void;
    constructor(cb: (conn: NetServerConn) => void);
    listen(port): void;
    private buildConn(): NetServerConn;
    onMsg(msg: pb.Msg): void;
  }

  function createServer(cb: (conn: NetServerConn) => void): NetServer;
}