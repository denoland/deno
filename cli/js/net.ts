// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { errors } from "./errors.ts";
import { EOF, Reader, Writer, Closer } from "./io.ts";
import { read, write } from "./ops/io.ts";
import { close } from "./ops/resources.ts";
import * as netOps from "./ops/net.ts";
import { Transport } from "./ops/net.ts";
export { ShutdownMode, shutdown, Transport } from "./ops/net.ts";

export interface Addr {
  transport: Transport;
  hostname: string;
  port: number;
}

export interface UDPAddr {
  transport?: Transport;
  hostname?: string;
  port: number;
}

export interface UDPConn extends AsyncIterable<[Uint8Array, Addr]> {
  receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;

  send(p: Uint8Array, addr: UDPAddr): Promise<void>;

  close(): void;

  addr: Addr;

  [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, Addr]>;
}

export interface Listener extends AsyncIterable<Conn> {
  accept(): Promise<Conn>;

  close(): void;

  addr: Addr;

  [Symbol.asyncIterator](): AsyncIterator<Conn>;
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

  read(p: Uint8Array): Promise<number | EOF> {
    return read(this.rid, p);
  }

  close(): void {
    close(this.rid);
  }

  closeRead(): void {
    netOps.shutdown(this.rid, netOps.ShutdownMode.Read);
  }

  closeWrite(): void {
    netOps.shutdown(this.rid, netOps.ShutdownMode.Write);
  }
}

export class ListenerImpl implements Listener {
  constructor(readonly rid: number, readonly addr: Addr) {}

  async accept(): Promise<Conn> {
    const res = await netOps.accept(this.rid);
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }

  close(): void {
    close(this.rid);
  }

  async *[Symbol.asyncIterator](): AsyncIterator<Conn> {
    while (true) {
      try {
        yield await this.accept();
      } catch (error) {
        if (error instanceof errors.BadResource) {
          break;
        }
        throw error;
      }
    }
  }
}

export async function recvfrom(
  rid: number,
  p: Uint8Array
): Promise<[number, Addr]> {
  const { size, remoteAddr } = await netOps.receive(rid, p);
  return [size, remoteAddr];
}

export class UDPConnImpl implements UDPConn {
  constructor(
    readonly rid: number,
    readonly addr: Addr,
    public bufSize: number = 1024
  ) {}

  async receive(p?: Uint8Array): Promise<[Uint8Array, Addr]> {
    const buf = p || new Uint8Array(this.bufSize);
    const [size, remoteAddr] = await recvfrom(this.rid, buf);
    const sub = buf.subarray(0, size);
    return [sub, remoteAddr];
  }

  async send(p: Uint8Array, addr: UDPAddr): Promise<void> {
    const remote = { hostname: "127.0.0.1", transport: "udp", ...addr };
    if (remote.transport !== "udp") throw Error("Remote transport must be UDP");
    const args = { ...remote, rid: this.rid };
    await netOps.send(args as netOps.SendRequest, p);
  }

  close(): void {
    close(this.rid);
  }

  async *[Symbol.asyncIterator](): AsyncIterator<[Uint8Array, Addr]> {
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
  closeRead(): void;
  closeWrite(): void;
}

export interface ListenOptions {
  port: number;
  hostname?: string;
  transport?: Transport;
}

export function listen(
  options: ListenOptions & { transport?: "tcp" }
): Listener;
export function listen(options: ListenOptions & { transport: "udp" }): UDPConn;
export function listen({
  port,
  hostname = "0.0.0.0",
  transport = "tcp"
}: ListenOptions): Listener | UDPConn {
  const res = netOps.listen({ port, hostname, transport });

  if (transport === "tcp") {
    return new ListenerImpl(res.rid, res.localAddr);
  } else {
    return new UDPConnImpl(res.rid, res.localAddr);
  }
}

export interface ConnectOptions {
  port: number;
  hostname?: string;
  transport?: Transport;
}

export async function connect({
  port,
  hostname = "127.0.0.1",
  transport = "tcp"
}: ConnectOptions): Promise<Conn> {
  const res = await netOps.connect({ port, hostname, transport });
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
