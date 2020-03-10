// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
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

/** A socket is a generic transport listener for message-oriented protocols */
export interface UDPConn extends AsyncIterator<[Uint8Array, Addr]> {
  /** Waits for and resolves to the next message to the `Socket`. */
  receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;

  /** Sends a message to the target. */
  send(p: Uint8Array, addr: UDPAddr): Promise<void>;

  /** Close closes the socket. Any pending message promises will be rejected
   * with errors.
   */
  close(): void;

  /** Return the address of the `Socket`. */
  addr: Addr;

  [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, Addr]>;
}

/** A Listener is a generic transport listener for stream-oriented protocols. */
export interface Listener extends AsyncIterator<Conn> {
  /** Waits for and resolves to the next connection to the `Listener`. */
  accept(): Promise<Conn>;

  /** Close closes the listener. Any pending accept promises will be rejected
   * with errors.
   */
  close(): void;

  /** Return the address of the `Listener`. */
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

  /** closeRead shuts down (shutdown(2)) the reading side of the TCP connection.
   * Most callers should just use close().
   */
  closeRead(): void {
    netOps.shutdown(this.rid, netOps.ShutdownMode.Read);
  }

  /** closeWrite shuts down (shutdown(2)) the writing side of the TCP
   * connection. Most callers should just use close().
   */
  closeWrite(): void {
    netOps.shutdown(this.rid, netOps.ShutdownMode.Write);
  }
}

export class ListenerImpl implements Listener {
  constructor(
    readonly rid: number,
    readonly addr: Addr,
    private closing: boolean = false
  ) {}

  async accept(): Promise<Conn> {
    const res = await netOps.accept(this.rid);
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }

  close(): void {
    this.closing = true;
    close(this.rid);
  }

  async next(): Promise<IteratorResult<Conn>> {
    if (this.closing) {
      return { value: undefined, done: true };
    }
    return await this.accept()
      .then(value => ({ value, done: false }))
      .catch(e => {
        // It wouldn't be correct to simply check this.closing here.
        // TODO: Get a proper error kind for this case, don't check the message.
        // The current error kind is Other.
        if (e.message == "Listener has been closed") {
          return { value: undefined, done: true };
        }
        throw e;
      });
  }

  [Symbol.asyncIterator](): AsyncIterator<Conn> {
    return this;
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
    public bufSize: number = 1024,
    private closing: boolean = false
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
    this.closing = true;
    close(this.rid);
  }

  async next(): Promise<IteratorResult<[Uint8Array, Addr]>> {
    if (this.closing) {
      return { value: undefined, done: true };
    }
    return await this.receive()
      .then(value => ({ value, done: false }))
      .catch(e => {
        // It wouldn't be correct to simply check this.closing here.
        // TODO: Get a proper error kind for this case, don't check the message.
        // The current error kind is Other.
        if (e.message == "Socket has been closed") {
          return { value: undefined, done: true };
        }
        throw e;
      });
  }

  [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, Addr]> {
    return this;
  }
}

export interface Conn extends Reader, Writer, Closer {
  /** The local address of the connection. */
  localAddr: Addr;
  /** The remote address of the connection. */
  remoteAddr: Addr;
  /** The resource ID of the connection. */
  rid: number;
  /** Shuts down (`shutdown(2)`) the reading side of the TCP connection. Most
   * callers should just use `close()`.
   */
  closeRead(): void;
  /** Shuts down (`shutdown(2)`) the writing side of the TCP connection. Most
   * callers should just use `close()`.
   */
  closeWrite(): void;
}

export interface ListenOptions {
  port: number;
  hostname?: string;
  transport?: Transport;
}

/** Listen announces on the local transport address.
 *
 * @param options
 * @param options.port The port to connect to. (Required.)
 * @param options.hostname A literal IP address or host name that can be
 *   resolved to an IP address. If not specified, defaults to 0.0.0.0
 * @param options.transport Must be "tcp" or "udp". Defaults to "tcp". Later we plan to add "tcp4",
 *   "tcp6", "udp4", "udp6", "ip", "ip4", "ip6", "unix", "unixgram" and
 *   "unixpacket".
 *
 * Examples:
 *
 *     listen({ port: 80 })
 *     listen({ hostname: "192.0.2.1", port: 80 })
 *     listen({ hostname: "[2001:db8::1]", port: 80 });
 *     listen({ hostname: "golang.org", port: 80, transport: "tcp" })
 */
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

/** Connects to the address on the named transport.
 *
 * @param options
 * @param options.port The port to connect to. (Required.)
 * @param options.hostname A literal IP address or host name that can be
 *   resolved to an IP address. If not specified, defaults to 127.0.0.1
 * @param options.transport Must be "tcp" or "udp". Defaults to "tcp". Later we plan to add "tcp4",
 *   "tcp6", "udp4", "udp6", "ip", "ip4", "ip6", "unix", "unixgram" and
 *   "unixpacket".
 *
 * Examples:
 *
 *     connect({ port: 80 })
 *     connect({ hostname: "192.0.2.1", port: 80 })
 *     connect({ hostname: "[2001:db8::1]", port: 80 });
 *     connect({ hostname: "golang.org", port: 80, transport: "tcp" })
 */
export async function connect({
  port,
  hostname = "127.0.0.1",
  transport = "tcp"
}: ConnectOptions): Promise<Conn> {
  const res = await netOps.connect({ port, hostname, transport });
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
