// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { errors } from "./errors.ts";
import { Reader, Writer, Closer } from "./io.ts";
import { read, write } from "./ops/io.ts";
import { close } from "./ops/resources.ts";
import * as netOps from "./ops/net.ts";
import { Addr } from "./ops/net.ts";
export { ShutdownMode, shutdown, NetAddr, UnixAddr } from "./ops/net.ts";

export interface DatagramConn extends AsyncIterable<[Uint8Array, Addr]> {
  receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;

  send(p: Uint8Array, addr: Addr): Promise<void>;

  close(): void;

  addr: Addr;

  [Symbol.asyncIterator](): AsyncIterableIterator<[Uint8Array, Addr]>;
}

export interface Listener extends AsyncIterable<Conn> {
  accept(): Promise<Conn>;

  close(): void;

  addr: Addr;

  rid: number;

  [Symbol.asyncIterator](): AsyncIterableIterator<Conn>;
}

export class ConnImpl implements Conn {
  constructor(
    readonly rid: number,
    readonly remoteAddr: Addr,
    readonly localAddr: Addr
  ) {}

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  read(p: Uint8Array): Promise<number | null> {
    return read(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }

  // TODO(lucacasonato): make this unavailable in stable
  closeWrite(): void {
    netOps.shutdown(this.rid, netOps.ShutdownMode.Write);
  }
}

export class ListenerImpl implements Listener {
  constructor(readonly rid: number, readonly addr: Addr) {}

  async accept(): Promise<Conn> {
    const res = await netOps.accept(this.rid, this.addr.transport);
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }

  async next(): Promise<IteratorResult<Conn>> {
    let conn: Conn;
    try {
      conn = await this.accept();
    } catch (error) {
      if (error instanceof errors.BadResource) {
        return { value: undefined, done: true };
      }
      throw error;
    }
    return { value: conn!, done: false };
  }

  return(value?: Conn): Promise<IteratorResult<Conn>> {
    this.close();
    return Promise.resolve({ value, done: true });
  }

  close(): void {
    close(this.rid);
  }

  [Symbol.asyncIterator](): AsyncIterableIterator<Conn> {
    return this;
  }
}

export class DatagramImpl implements DatagramConn {
  constructor(
    readonly rid: number,
    readonly addr: Addr,
    public bufSize: number = 1024
  ) {}

  async receive(p?: Uint8Array): Promise<[Uint8Array, Addr]> {
    const buf = p || new Uint8Array(this.bufSize);
    const { size, remoteAddr } = await netOps.receive(
      this.rid,
      this.addr.transport,
      buf
    );
    const sub = buf.subarray(0, size);
    return [sub, remoteAddr];
  }

  async send(p: Uint8Array, addr: Addr): Promise<void> {
    const remote = { hostname: "127.0.0.1", ...addr };

    const args = { ...remote, rid: this.rid };
    await netOps.send(args as netOps.SendRequest, p);
  }

  close(): void {
    close(this.rid);
  }

  async *[Symbol.asyncIterator](): AsyncIterableIterator<[Uint8Array, Addr]> {
    while (true) {
      try {
        yield await this.receive();
      } catch (error) {
        if (error instanceof errors.BadResource) {
          break;
        }
        throw error;
      }
    }
  }
}

export interface Conn extends Reader, Writer, Closer {
  localAddr: Addr;
  remoteAddr: Addr;
  rid: number;
  closeWrite(): void;
}

export interface ListenOptions {
  port: number;
  hostname?: string;
  transport?: "tcp";
}

export function listen(
  options: ListenOptions & { transport?: "tcp" }
): Listener;
export function listen(options: ListenOptions): Listener {
  const res = netOps.listen({
    transport: "tcp",
    hostname: "0.0.0.0",
    ...(options as ListenOptions),
  });

  return new ListenerImpl(res.rid, res.localAddr);
}

export interface ConnectOptions {
  port: number;
  hostname?: string;
  transport?: "tcp";
}
export interface UnixConnectOptions {
  transport: "unix";
  path: string;
}
export async function connect(options: UnixConnectOptions): Promise<Conn>;
export async function connect(options: ConnectOptions): Promise<Conn>;
export async function connect(
  options: ConnectOptions | UnixConnectOptions
): Promise<Conn> {
  let res;

  if (options.transport === "unix") {
    res = await netOps.connect(options);
  } else {
    res = await netOps.connect({
      transport: "tcp",
      hostname: "127.0.0.1",
      ...options,
    });
  }

  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
