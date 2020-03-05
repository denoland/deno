// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { EOF, Reader, Writer, Closer } from "./io.ts";
import { read, write, close } from "./files.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";

// TODO support other types: "tcp4" | "tcp6"

export interface TCPAddr {
  transport: "tcp";
  hostname: string;
  port: number;
}

export interface UDPAddr {
  transport: "udp";
  hostname: string;
  port: number;
}

export interface UnixAddr {
  transport: "unix" | "unixpacket";
  address: string;
}

/** A socket is a generic transport listener for message-oriented protocols */
export interface DatagramConn<T> extends AsyncIterator<[Uint8Array, T]> {
  /** Waits for and resolves to the next message to the `Socket`. */
  receive(p?: Uint8Array): Promise<[Uint8Array, T]>;

  /** Sends a message to the target. */
  send(p: Uint8Array, addr: T): Promise<void>;

  /** Close closes the socket. Any pending message promises will be rejected
   * with errors.
   */
  close(): void;

  /** Return the address of the `Socket`. */
  addr: T;

  [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, T]>;
}

/** A Listener is a generic transport listener for stream-oriented protocols. */
export interface Listener<T> extends AsyncIterator<Conn<T>> {
  /** Waits for and resolves to the next connection to the `Listener`. */
  accept(): Promise<Conn<T>>;

  /** Close closes the listener. Any pending accept promises will be rejected
   * with errors.
   */
  close(): void;

  /** Return the address of the `Listener`. */
  addr: T;

  [Symbol.asyncIterator](): AsyncIterator<Conn<T>>;
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
  sendSync("op_shutdown", { rid, how });
}

export class ConnImpl<T> implements Conn<T> {
  constructor(
    readonly rid: number,
    readonly remoteAddr: T,
    readonly localAddr: T
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

export class ListenerImpl<T> implements Listener<T> {
  constructor(
    readonly rid: number,
    readonly addr: T,
    private closing: boolean = false
  ) {}

  async accept(): Promise<Conn<T>> {
    const res = await sendAsync("op_accept", {
      rid: this.rid,
      ...this.addr
    });
    return new ConnImpl<T>(res.rid, res.remoteAddr, res.localAddr);
  }

  close(): void {
    this.closing = true;
    close(this.rid);
  }

  async next(): Promise<IteratorResult<Conn<T>>> {
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

  [Symbol.asyncIterator](): AsyncIterator<Conn<T>> {
    return this;
  }
}

export async function recvfrom<T>(
  args: { rid: number; transport: string },
  p: Uint8Array
): Promise<[number, T]> {
  const { size, remoteAddr } = await sendAsync("op_receive", args, p);
  return [size, remoteAddr];
}

export class DatagramImpl<T> implements DatagramConn<T> {
  constructor(
    readonly rid: number,
    readonly addr: T,
    public bufSize: number = 1024,
    private closing: boolean = false
  ) {}

  async receive(p?: Uint8Array): Promise<[Uint8Array, T]> {
    const buf = p || new Uint8Array(this.bufSize);
    const { size, remoteAddr } = await sendAsync(
      "op_receive",
      { rid: this.rid, ...this.addr },
      buf
    );
    const sub = buf.subarray(0, size);
    return [sub, remoteAddr];
  }

  async send(p: Uint8Array, addr: T): Promise<void> {
    const args = { rid: this.rid, ...addr };
    await sendAsync("op_send", args, p);
  }

  close(): void {
    this.closing = true;
    close(this.rid);
  }

  async next(): Promise<IteratorResult<[Uint8Array, T]>> {
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

  [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, T]> {
    return this;
  }
}

export interface Conn<T> extends Reader, Writer, Closer {
  /** The local address of the connection. */
  localAddr: T;
  /** The remote address of the connection. */
  remoteAddr: T;
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
}

export interface UnixListenOptions {
  address: string;
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
export function listen(
  options: ListenOptions & { transport?: "tcp" }
): Listener<TCPAddr>;
export function listen(
  options: UnixListenOptions & { transport: "unix" }
): Listener<UnixAddr>;
export function listen(
  options: ListenOptions & { transport: "udp" }
): DatagramConn<UDPAddr>;
export function listen(
  options: UnixListenOptions & { transport: "unixpacket" }
): DatagramConn<UnixAddr>;
export function listen(
  options: ListenOptions | UnixListenOptions
): Listener<TCPAddr | UnixAddr> | DatagramConn<UDPAddr | UnixAddr> {
  const args = { ...listenDefaults, ...options };
  const res = sendSync("op_listen", args);
  if (args.transport === "tcp") {
    return new ListenerImpl<TCPAddr>(res.rid, res.localAddr);
  } else if (args.transport === "unix") {
    return new ListenerImpl<UnixAddr>(res.rid, res.localAddr);
  } else if (args.transport === "udp") {
    return new DatagramImpl<UDPAddr>(res.rid, res.localAddr);
  } else {
    return new DatagramImpl<UnixAddr>(res.rid, res.localAddr);
  }
}

export interface ConnectOptions {
  port: number;
  hostname?: string;
  transport?: "tcp";
}
export interface UnixConnectOptions {
  transport: "unix";
  address: string;
}

const connectDefaults = { hostname: "127.0.0.1", transport: "tcp" };

/** Connects to the address on the named transport.
 *
 * @param options
 * @param options.port The port to connect to. (Required.)
 * @param options.hostname A literal IP address or host name that can be
 *   resolved to an IP address. If not specified, defaults to 127.0.0.1
 * @param options.transport Must be "tcp" or "unix". Defaults to "tcp". Later we plan to add "tcp4",
 *   "tcp6", "ip", "ip4", "ip6", "unix".
 *
 * Examples:
 *
 *     connect({ port: 80 })
 *     connect({ hostname: "192.0.2.1", port: 80 })
 *     connect({ hostname: "[2001:db8::1]", port: 80 });
 *     connect({ hostname: "golang.org", port: 80, transport: "tcp" })
 */
export async function connect(options: ConnectOptions): Promise<Conn<TCPAddr>>;
export async function connect(
  options: UnixConnectOptions
): Promise<Conn<UnixAddr>>;
export async function connect(
  options: ConnectOptions | UnixConnectOptions
): Promise<Conn<TCPAddr | UnixAddr>> {
  options = Object.assign(connectDefaults, options);
  const res = await sendAsync("op_connect", options);
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}
