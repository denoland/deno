// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { ReadResult, Reader, Writer, Closer } from "./io";
import * as msg from "gen/msg_generated";
import { assert, notImplemented } from "./util";
import * as dispatch from "./dispatch";
import * as flatbuffers from "./flatbuffers";
import { read, write, close } from "./files";

export type Network = "tcp";
// TODO support other types:
// export type Network = "tcp" | "tcp4" | "tcp6" | "unix" | "unixpacket";

// TODO Support finding network from Addr, see https://golang.org/pkg/net/#Addr
export type Addr = string;

/** A Listener is a generic network listener for stream-oriented protocols. */
export interface Listener {
  /** Waits for and resolves to the next connection to the `Listener`. */
  accept(): Promise<Conn>;

  /** Close closes the listener. Any pending accept promises will be rejected
   * with errors.
   */
  close(): void;

  /** Return the address of the `Listener`. */
  addr(): Addr;
}

class ListenerImpl implements Listener {
  constructor(readonly rid: number) {}

  async accept(): Promise<Conn> {
    const builder = flatbuffers.createBuilder();
    msg.Accept.startAccept(builder);
    msg.Accept.addRid(builder, this.rid);
    const inner = msg.Accept.endAccept(builder);
    const baseRes = await dispatch.sendAsync(builder, msg.Any.Accept, inner);
    assert(baseRes != null);
    assert(msg.Any.NewConn === baseRes!.innerType());
    const res = new msg.NewConn();
    assert(baseRes!.inner(res) != null);
    return new ConnImpl(res.rid(), res.remoteAddr()!, res.localAddr()!);
  }

  close(): void {
    close(this.rid);
  }

  addr(): Addr {
    return notImplemented();
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

class ConnImpl implements Conn {
  constructor(
    readonly rid: number,
    readonly remoteAddr: string,
    readonly localAddr: string
  ) {}

  write(p: Uint8Array): Promise<number> {
    return write(this.rid, p);
  }

  read(p: Uint8Array): Promise<ReadResult> {
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

enum ShutdownMode {
  // See http://man7.org/linux/man-pages/man2/shutdown.2.html
  // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
  Read = 0,
  Write,
  ReadWrite // unused
}

function shutdown(rid: number, how: ShutdownMode) {
  const builder = flatbuffers.createBuilder();
  msg.Shutdown.startShutdown(builder);
  msg.Shutdown.addRid(builder, rid);
  msg.Shutdown.addHow(builder, how);
  const inner = msg.Shutdown.endShutdown(builder);
  const baseRes = dispatch.sendSync(builder, msg.Any.Shutdown, inner);
  assert(baseRes == null);
}

/** Listen announces on the local network address.
 *
 * The network must be `tcp`, `tcp4`, `tcp6`, `unix` or `unixpacket`.
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
 * See `dial()` for a description of the network and address parameters.
 */
export function listen(network: Network, address: string): Listener {
  const builder = flatbuffers.createBuilder();
  const network_ = builder.createString(network);
  const address_ = builder.createString(address);
  msg.Listen.startListen(builder);
  msg.Listen.addNetwork(builder, network_);
  msg.Listen.addAddress(builder, address_);
  const inner = msg.Listen.endListen(builder);
  const baseRes = dispatch.sendSync(builder, msg.Any.Listen, inner);
  assert(baseRes != null);
  assert(msg.Any.ListenRes === baseRes!.innerType());
  const res = new msg.ListenRes();
  assert(baseRes!.inner(res) != null);
  return new ListenerImpl(res.rid());
}

/** Dial connects to the address on the named network.
 *
 * Supported networks are only `tcp` currently.
 *
 * TODO: `tcp4` (IPv4-only), `tcp6` (IPv6-only), `udp`, `udp4` (IPv4-only),
 * `udp6` (IPv6-only), `ip`, `ip4` (IPv4-only), `ip6` (IPv6-only), `unix`,
 * `unixgram` and `unixpacket`.
 *
 * For TCP and UDP networks, the address has the form `host:port`. The host must
 * be a literal IP address, or a host name that can be resolved to IP addresses.
 * The port must be a literal port number or a service name. If the host is a
 * literal IPv6 address it must be enclosed in square brackets, as in
 * `[2001:db8::1]:80` or `[fe80::1%zone]:80`. The zone specifies the scope of
 * the literal IPv6 address as defined in RFC 4007. The functions JoinHostPort
 * and SplitHostPort manipulate a pair of host and port in this form. When using
 * TCP, and the host resolves to multiple IP addresses, Dial will try each IP
 * address in order until one succeeds.
 *
 * Examples:
 *
 *     dial("tcp", "golang.org:http")
 *     dial("tcp", "192.0.2.1:http")
 *     dial("tcp", "198.51.100.1:80")
 *     dial("udp", "[2001:db8::1]:domain")
 *     dial("udp", "[fe80::1%lo0]:53")
 *     dial("tcp", ":80")
 */
export async function dial(network: Network, address: string): Promise<Conn> {
  const builder = flatbuffers.createBuilder();
  const network_ = builder.createString(network);
  const address_ = builder.createString(address);
  msg.Dial.startDial(builder);
  msg.Dial.addNetwork(builder, network_);
  msg.Dial.addAddress(builder, address_);
  const inner = msg.Dial.endDial(builder);
  const baseRes = await dispatch.sendAsync(builder, msg.Any.Dial, inner);
  assert(baseRes != null);
  assert(msg.Any.NewConn === baseRes!.innerType());
  const res = new msg.NewConn();
  assert(baseRes!.inner(res) != null);
  return new ConnImpl(res.rid(), res.remoteAddr()!, res.localAddr()!);
}

/** **RESERVED** */
export async function connect(
  network: Network,
  address: string
): Promise<Conn> {
  return notImplemented();
}
