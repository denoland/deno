// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  /** The type of the resource record.
   * Only the listed types are supported currently. */
  export type RecordType =
    | "A"
    | "AAAA"
    | "ANAME"
    | "CNAME"
    | "MX"
    | "PTR"
    | "SRV"
    | "TXT";

  export interface ResolveDnsOptions {
    /** The name server to be used for lookups.
  * If not specified, defaults to the system configuration e.g. `/etc/resolv.conf` on Unix. */
    nameServer?: {
      /** The IP address of the name server */
      ipAddr: string;
      /** The port number the query will be sent to.
    * If not specified, defaults to 53. */
      port?: number;
    };
  }

  /** If `resolveDns` is called with "MX" record type specified, it will return an array of this interface. */
  export interface MXRecord {
    preference: number;
    exchange: string;
  }

  /** If `resolveDns` is called with "SRV" record type specified, it will return an array of this interface. */
  export interface SRVRecord {
    priority: number;
    weight: number;
    port: number;
    target: string;
  }

  export function resolveDns(
    query: string,
    recordType: "A" | "AAAA" | "ANAME" | "CNAME" | "PTR",
    options?: ResolveDnsOptions,
  ): Promise<string[]>;

  export function resolveDns(
    query: string,
    recordType: "MX",
    options?: ResolveDnsOptions,
  ): Promise<MXRecord[]>;

  export function resolveDns(
    query: string,
    recordType: "SRV",
    options?: ResolveDnsOptions,
  ): Promise<SRVRecord[]>;

  export function resolveDns(
    query: string,
    recordType: "TXT",
    options?: ResolveDnsOptions,
  ): Promise<string[][]>;

  /** ** UNSTABLE**: new API, yet to be vetted.
*
* Performs DNS resolution against the given query, returning resolved records.
* Fails in the cases such as:
* - the query is in invalid format
* - the options have an invalid parameter, e.g. `nameServer.port` is beyond the range of 16-bit unsigned integer
* - timed out
*
* ```ts
* const a = await Deno.resolveDns("example.com", "A");
*
* const aaaa = await Deno.resolveDns("example.com", "AAAA", {
*   nameServer: { ipAddr: "8.8.8.8", port: 1234 },
* });
* ```
*
* Requires `allow-net` permission.
  */
  export function resolveDns(
    query: string,
    recordType: RecordType,
    options?: ResolveDnsOptions,
  ): Promise<string[] | MXRecord[] | SRVRecord[] | string[][]>;

  /** **UNSTABLE**: new API, yet to be vetted.
*
* A generic transport listener for message-oriented protocols. */
  export interface DatagramConn extends AsyncIterable<[Uint8Array, Addr]> {
    /** **UNSTABLE**: new API, yet to be vetted.
  *
  * Waits for and resolves to the next message to the `UDPConn`. */
    receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;
    /** UNSTABLE: new API, yet to be vetted.
  *
  * Sends a message to the target. */
    send(p: Uint8Array, addr: Addr): Promise<number>;
    /** UNSTABLE: new API, yet to be vetted.
  *
  * Close closes the socket. Any pending message promises will be rejected
  * with errors. */
    close(): void;
    /** Return the address of the `UDPConn`. */
    readonly addr: Addr;
    [Symbol.asyncIterator](): AsyncIterableIterator<[Uint8Array, Addr]>;
  }

  export interface UnixListenOptions {
    /** A Path to the Unix Socket. */
    path: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
*
* Listen announces on the local transport address.
*
* ```ts
* const listener = Deno.listen({ path: "/foo/bar.sock", transport: "unix" })
* ```
*
* Requires `allow-read` and `allow-write` permission. */
  export function listen(
    options: UnixListenOptions & { transport: "unix" },
  ): Listener;

  /** **UNSTABLE**: new API, yet to be vetted
*
* Listen announces on the local transport address.
*
* ```ts
* const listener1 = Deno.listenDatagram({
*   port: 80,
*   transport: "udp"
* });
* const listener2 = Deno.listenDatagram({
*   hostname: "golang.org",
*   port: 80,
*   transport: "udp"
* });
* ```
*
* Requires `allow-net` permission. */
  export function listenDatagram(
    options: ListenOptions & { transport: "udp" },
  ): DatagramConn;

  /** **UNSTABLE**: new API, yet to be vetted
*
* Listen announces on the local transport address.
*
* ```ts
* const listener = Deno.listenDatagram({
*   path: "/foo/bar.sock",
*   transport: "unixpacket"
* });
* ```
*
* Requires `allow-read` and `allow-write` permission. */
  export function listenDatagram(
    options: UnixListenOptions & { transport: "unixpacket" },
  ): DatagramConn;

  export interface UnixConnectOptions {
    transport: "unix";
    path: string;
  }

  /** **UNSTABLE**:  The unix socket transport is unstable as a new API yet to
* be vetted.  The TCP transport is considered stable.
*
* Connects to the hostname (default is "127.0.0.1") and port on the named
* transport (default is "tcp"), and resolves to the connection (`Conn`).
*
* ```ts
* const conn1 = await Deno.connect({ port: 80 });
* const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
* const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
* const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
* const conn5 = await Deno.connect({ path: "/foo/bar.sock", transport: "unix" });
* ```
*
* Requires `allow-net` permission for "tcp" and `allow-read` for "unix". */
  export function connect(
    options: ConnectOptions | UnixConnectOptions,
  ): Promise<Conn>;

  export interface ConnectTlsClientCertOptions {
    /** PEM formatted client certificate chain. */
    certChain: string;
    /** PEM formatted (RSA or PKCS8) private key of client certificate. */
    privateKey: string;
  }

  /** **UNSTABLE** New API, yet to be vetted.
   *
   * Create a TLS connection with an attached client certificate.
   *
   * ```ts
   * const conn = await Deno.connectTls({
   *   hostname: "deno.land",
   *   port: 443,
   *   certChain: "---- BEGIN CERTIFICATE ----\n ...",
   *   privateKey: "---- BEGIN PRIVATE KEY ----\n ...",
   * });
   * ```
   *
   * Requires `allow-net` permission.
   */
  export function connectTls(
    options: ConnectTlsOptions & ConnectTlsClientCertOptions,
  ): Promise<Conn>;

  export interface StartTlsOptions {
    /** A literal IP address or host name that can be resolved to an IP address.
  * If not specified, defaults to `127.0.0.1`. */
    hostname?: string;
    /** Server certificate file. */
    certFile?: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
*
* Start TLS handshake from an existing connection using
* an optional cert file, hostname (default is "127.0.0.1").  The
* cert file is optional and if not included Mozilla's root certificates will
* be used (see also https://github.com/ctz/webpki-roots for specifics)
* Using this function requires that the other end of the connection is
* prepared for TLS handshake.
*
* ```ts
* const conn = await Deno.connect({ port: 80, hostname: "127.0.0.1" });
* const tlsConn = await Deno.startTls(conn, { certFile: "./certs/my_custom_root_CA.pem", hostname: "localhost" });
* ```
*
* Requires `allow-net` permission.
  */
  export function startTls(
    conn: Conn,
    options?: StartTlsOptions,
  ): Promise<Conn>;

  export interface ListenTlsOptions {
    /** **UNSTABLE**: new API, yet to be vetted.
  *
  * Application-Layer Protocol Negotiation (ALPN) protocols to announce to
  * the client. If not specified, no ALPN extension will be included in the
  * TLS handshake.
  */
    alpnProtocols?: string[];
  }
}
