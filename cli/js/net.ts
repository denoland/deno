// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { EOF, Reader, Writer, Closer } from "./io.ts";
import { read, write, close } from "./files.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";

export type Transport = "tcp" | "udp";
// TODO support other types:
// export type Transport = "tcp" | "tcp4" | "tcp6" | "unix" | "unixpacket";

export interface Addr {
  transport: Transport;
  hostname: string;
  port: number;
}

export interface PartialAddr {
  transport?: Transport;
  hostname?: string;
  port: number;
}

export type Message = [Uint8Array, Addr];

/** A socket is a generic transport listener for message-oriented protocols */
export interface Socket extends AsyncIterator<Message> {
  /** Waits for and resolves to the next message to the `Socket`. */
  receive(): Promise<Message>;

  /** Sends a message to the target. */
  send(buffer: Uint8Array, remote: PartialAddr): Promise<void>;

  /** Close closes the socket. Any pending message promises will be rejected
   * with errors.
   */
  close(): void;

  /** Return the address of the `Socket`. */
  addr: Addr;

  [Symbol.asyncIterator](): AsyncIterator<Message>;
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

export enum ShutdownMode {
  // See http://man7.org/linux/man-pages/man2/shutdown.2.html
  // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
  Read = 0,
  Write,
  ReadWrite // unused
}

/** Shut down socket send and receive operations.
 *
 * Matches behavior of POSIX shutdown(3).
 *
 *       const listener = Deno.listen({ port: 80 });
 *       const conn = await listener.accept();
 *       Deno.shutdown(conn.rid, Deno.ShutdownMode.Write);
 */
export function shutdown(rid: number, how: ShutdownMode): void {
  sendSync(dispatch.OP_SHUTDOWN, { rid, how });
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
    shutdown(this.rid, ShutdownMode.Read);
  }

  /** closeWrite shuts down (shutdown(2)) the writing side of the TCP
   * connection. Most callers should just use close().
   */
  closeWrite(): void {
    shutdown(this.rid, ShutdownMode.Write);
  }
}

export class ListenerImpl implements Listener {
  constructor(
    readonly rid: number,
    public addr: Addr,
    private closing: boolean = false
  ) {}

  async accept(): Promise<Conn> {
    const res = await sendAsync(dispatch.OP_ACCEPT, { rid: this.rid });
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

export class SocketImpl implements Socket {
  constructor(
    readonly rid: number,
    public addr: Addr,
    private closing: boolean = false
  ) {}

  async receive(): Promise<Message> {
    const res = await sendAsync(dispatch.OP_RECEIVE, { rid: this.rid });
    const buffer = new Uint8Array(res.buffer);
    return [buffer, res.remoteAddr];
  }

  async send(buffer: Uint8Array, rem: PartialAddr): Promise<void> {
    const remote = { hostname: "127.0.0.1", transport: "udp", ...rem };
    if (remote.transport !== "udp") throw Error("Remote transport must be UDP");
    const args = { ...remote, rid: this.rid, buffer: Array.from(buffer) };
    await sendAsync(dispatch.OP_SEND, args);
  }

  close(): void {
    this.closing = true;
    close(this.rid);
  }

  async next(): Promise<IteratorResult<Message>> {
    if (this.closing) {
      return { value: undefined, done: true };
    }
    const value = await this.receive();
    return { value, done: false };
  }

  [Symbol.asyncIterator](): AsyncIterator<Message> {
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

const listenDefaults = { hostname: "0.0.0.0", transport: "tcp" };

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
export function listen(options: ListenOptions & { transport: "tcp" }): Listener;
export function listen(options: ListenOptions & { transport: "udp" }): Socket;
export function listen(options: ListenOptions): Listener | Socket {
  const args = { ...listenDefaults, ...options };
  const res = sendSync(dispatch.OP_LISTEN, args);

  if (args.transport === "tcp") {
    return new ListenerImpl(res.rid, res.localAddr);
  } else {
    return new SocketImpl(res.rid, res.localAddr);
  }
}

export interface ConnectOptions {
  port: number;
  hostname?: string;
  transport?: Transport;
}

const connectDefaults = { hostname: "127.0.0.1", transport: "tcp" };

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
export async function connect(options: ConnectOptions): Promise<Conn> {
  options = Object.assign(connectDefaults, options);
  const res = await sendAsync(dispatch.OP_CONNECT, options);
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
