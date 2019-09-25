// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { EOF, Reader, Writer, Closer } from "./io.ts";
import { notImplemented } from "./util.ts";
import { read, write, close } from "./files.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";

export type Transport = "tcp";
// TODO support other types:
// export type Transport = "tcp" | "tcp4" | "tcp6" | "unix" | "unixpacket";

// TODO(ry) Replace 'address' with 'hostname' and 'port', similar to DialOptions
// and ListenOptions.
export interface Addr {
  transport: Transport;
  address: string;
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
  addr(): Addr;

  [Symbol.asyncIterator](): AsyncIterator<Conn>;
}

enum ShutdownMode {
  // See http://man7.org/linux/man-pages/man2/shutdown.2.html
  // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
  Read = 0,
  Write,
  ReadWrite // unused
}

function shutdown(rid: number, how: ShutdownMode): void {
  sendSync(dispatch.OP_SHUTDOWN, { rid, how });
}

export class ConnImpl implements Conn {
  constructor(
    readonly rid: number,
    readonly remoteAddr: string,
    readonly localAddr: string
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

class ListenerImpl implements Listener {
  constructor(
    readonly rid: number,
    private transport: Transport,
    private localAddr: string
  ) {}

  async accept(): Promise<Conn> {
    const res = await sendAsync(dispatch.OP_ACCEPT, { rid: this.rid });
    return new ConnImpl(res.rid, res.remoteAddr, res.localAddr);
  }

  close(): void {
    close(this.rid);
  }

  addr(): Addr {
    return {
      transport: this.transport,
      address: this.localAddr
    };
  }

  async next(): Promise<IteratorResult<Conn>> {
    return {
      done: false,
      value: await this.accept()
    };
  }

  [Symbol.asyncIterator](): AsyncIterator<Conn> {
    return this;
  }
}

export interface Conn extends Reader, Writer, Closer {
  /** The local address of the connection. */
  localAddr: string;
  /** The remote address of the connection. */
  remoteAddr: string;
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
 * @param options.transport Defaults to "tcp". Later we plan to add "tcp4",
 *   "tcp6", "udp", "udp4", "udp6", "ip", "ip4", "ip6", "unix", "unixgram" and
 *   "unixpacket".
 *
 * Examples:
 *
 *     listen({ port: 80 })
 *     listen({ hostname: "192.0.2.1", port: 80 })
 *     listen({ hostname: "[2001:db8::1]", port: 80 });
 *     listen({ hostname: "golang.org", port: 80, transport: "tcp" })
 */
export function listen(options: ListenOptions): Listener {
  options = Object.assign(listenDefaults, options);
  const res = sendSync(dispatch.OP_LISTEN, options);
  return new ListenerImpl(res.rid, options.transport, res.localAddr);
}

export interface DialOptions {
  port: number;
  hostname?: string;
  transport?: Transport;
}
const dialDefaults = { hostname: "127.0.0.1", transport: "tcp" };

/** Dial connects to the address on the named transport.
 *
 * @param options
 * @param options.port The port to connect to. (Required.)
 * @param options.hostname A literal IP address or host name that can be
 *   resolved to an IP address. If not specified, defaults to 127.0.0.1
 * @param options.transport Defaults to "tcp". Later we plan to add "tcp4",
 *   "tcp6", "udp", "udp4", "udp6", "ip", "ip4", "ip6", "unix", "unixgram" and
 *   "unixpacket".
 *
 * Examples:
 *
 *     dial({ port: 80 })
 *     dial({ hostname: "192.0.2.1", port: 80 })
 *     dial({ hostname: "[2001:db8::1]", port: 80 });
 *     dial({ hostname: "golang.org", port: 80, transport: "tcp" })
 */
export async function dial(options: DialOptions): Promise<Conn> {
  options = Object.assign(dialDefaults, options);
  const res = await sendAsync(dispatch.OP_DIAL, options);
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}

/** **RESERVED** */
export async function connect(
  _transport: Transport,
  _address: string
): Promise<Conn> {
  return notImplemented();
}
