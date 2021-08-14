// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  export interface NetAddr {
    transport: "tcp" | "udp";
    hostname: string;
    port: number;
  }

  export interface UnixAddr {
    transport: "unix" | "unixpacket";
    path: string;
  }

  export type Addr = NetAddr | UnixAddr;

  /** A generic network listener for stream-oriented protocols. */
  export interface Listener extends AsyncIterable<Conn> {
    /** Waits for and resolves to the next connection to the `Listener`. */
    accept(): Promise<Conn>;
    /** Close closes the listener. Any pending accept promises will be rejected
   * with errors. */
    close(): void;
    /** Return the address of the `Listener`. */
    readonly addr: Addr;

    /** Return the rid of the `Listener`. */
    readonly rid: number;

    [Symbol.asyncIterator](): AsyncIterableIterator<Conn>;
  }

  export interface Conn extends Reader, Writer, Closer {
    /** The local address of the connection. */
    readonly localAddr: Addr;
    /** The remote address of the connection. */
    readonly remoteAddr: Addr;
    /** The resource ID of the connection. */
    readonly rid: number;
    /** Shuts down (`shutdown(2)`) the write side of the connection. Most
   * callers should just use `close()`. */
    closeWrite(): Promise<void>;
  }

  export interface ListenOptions {
    /** The port to listen on. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
   * If not specified, defaults to `0.0.0.0`. */
    hostname?: string;
  }

  /** Listen announces on the local transport address.
 *
 * ```ts
 * const listener1 = Deno.listen({ port: 80 })
 * const listener2 = Deno.listen({ hostname: "192.0.2.1", port: 80 })
 * const listener3 = Deno.listen({ hostname: "[2001:db8::1]", port: 80 });
 * const listener4 = Deno.listen({ hostname: "golang.org", port: 80, transport: "tcp" });
 * ```
 *
 * Requires `allow-net` permission. */
  export function listen(
    options: ListenOptions & { transport?: "tcp" },
  ): Listener;

  export interface ListenTlsOptions extends ListenOptions {
    /** Path to a file containing a PEM formatted CA certificate. Requires
     * `--allow-read`. */
    certFile: string;
    /** Server public key file. Requires `--allow-read`.*/
    keyFile: string;

    transport?: "tcp";
  }

  /** Listen announces on the local transport address over TLS (transport layer
 * security).
 *
 * ```ts
 * const lstnr = Deno.listenTls({ port: 443, certFile: "./server.crt", keyFile: "./server.key" });
 * ```
 *
 * Requires `allow-net` permission. */
  export function listenTls(options: ListenTlsOptions): Listener;

  export interface ConnectOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
   * If not specified, defaults to `127.0.0.1`. */
    hostname?: string;
    transport?: "tcp";
  }

  /**
 * Connects to the hostname (default is "127.0.0.1") and port on the named
 * transport (default is "tcp"), and resolves to the connection (`Conn`).
 *
 * ```ts
 * const conn1 = await Deno.connect({ port: 80 });
 * const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
 * const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
 * const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
 * ```
 *
 * Requires `allow-net` permission for "tcp". */
  export function connect(options: ConnectOptions): Promise<Conn>;

  export interface ConnectTlsOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
   * If not specified, defaults to `127.0.0.1`. */
    hostname?: string;
    /** Server certificate file. */
    certFile?: string;
  }

  /** Establishes a secure connection over TLS (transport layer security) using
 * an optional cert file, hostname (default is "127.0.0.1") and port.  The
 * cert file is optional and if not included Mozilla's root certificates will
 * be used (see also https://github.com/ctz/webpki-roots for specifics)
 *
 * ```ts
 * const conn1 = await Deno.connectTls({ port: 80 });
 * const conn2 = await Deno.connectTls({ certFile: "./certs/my_custom_root_CA.pem", hostname: "192.0.2.1", port: 80 });
 * const conn3 = await Deno.connectTls({ hostname: "[2001:db8::1]", port: 80 });
 * const conn4 = await Deno.connectTls({ certFile: "./certs/my_custom_root_CA.pem", hostname: "golang.org", port: 80});
 * ```
 *
 * Requires `allow-net` permission.
 */
  export function connectTls(options: ConnectTlsOptions): Promise<Conn>;

  /** Shutdown socket send operations.
 *
 * Matches behavior of POSIX shutdown(3).
 *
 * ```ts
 * const listener = Deno.listen({ port: 80 });
 * const conn = await listener.accept();
 * Deno.shutdown(conn.rid);
 * ```
 */
  export function shutdown(rid: number): Promise<void>;
}
