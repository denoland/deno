// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { EOF, Reader, Writer, Closer } from "./io.ts";
import { notImplemented } from "./util.ts";
import { read, write, close } from "./files.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";

export type Network = "tcp";
// TODO support other types:
// export type Network = "tcp" | "tcp4" | "tcp6" | "unix" | "unixpacket";

export interface Addr {
  network: Network;
  address: string;
}

export interface NetworkOptions {
  network: Network;
}

/** A Listener is a generic network listener for stream-oriented protocols. */
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

class ConnImpl implements Conn {
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
    private network: Network,
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
      network: this.network,
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

/** Listen announces on the local network address.
 *
 * For TCP networks, if the host in the address parameter is empty or a literal
 * unspecified IP address, `listen()` listens on all available unicast and
 * anycast IP addresses of the local system. To only use IPv4, use network
 * `tcp4`. The address can use a host name, but this is not recommended,
 * because it will create a listener for at most one of the host's IP
 * addresses. If the port in the address parameter is empty or `0`, as in
 * `127.0.0.1:` or `[::1]:0`, a port number is automatically chosen. The
 * `addr()` method of `Listener` can be used to discover the chosen port.
 *
 * See `dial()` for a description of the address parameters and network options.
 */
export function listen(
  address: string,
  options: NetworkOptions = { network: "tcp" }
): Listener {
  const res = sendSync(dispatch.OP_LISTEN, {
    network: options.network,
    address
  });
  return new ListenerImpl(res.rid, options.network, res.localAddr);
}

/** Dial connects to the address on the named network.
 *
 * Options: port, hostname, transport.
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
export async function dial(
  options: NetworkOptions = { }
): Promise<Conn> {
  const res = await sendAsync(dispatch.OP_DIAL, {
    network: options.network,
    address
  });
  // TODO(bartlomieju): add remoteAddr and localAddr on Rust side
  return new ConnImpl(res.rid, res.remoteAddr!, res.localAddr!);
}

/** **RESERVED** */
export async function connect(
  _network: Network,
  _address: string
): Promise<Conn> {
  return notImplemented();
}
