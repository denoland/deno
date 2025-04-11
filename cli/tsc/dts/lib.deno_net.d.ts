// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="esnext.disposable" />

declare namespace Deno {
  /** @category Network */
  export interface NetAddr {
    transport: "tcp" | "udp";
    hostname: string;
    port: number;
  }

  /** @category Network */
  export interface UnixAddr {
    transport: "unix" | "unixpacket";
    path: string;
  }

  /**
   * @experimental **UNSTABLE**: New API, yet to be vetted.
   * @category Network
   */
  export interface VsockAddr {
    transport: "vsock";
    cid: number;
    port: number;
  }

  /** @category Network */
  export type Addr = NetAddr | UnixAddr | VsockAddr;

  /** A generic network listener for stream-oriented protocols.
   *
   * @category Network
   */
  export interface Listener<T extends Conn = Conn, A extends Addr = Addr>
    extends AsyncIterable<T>, Disposable {
    /** Waits for and resolves to the next connection to the `Listener`. */
    accept(): Promise<T>;
    /** Close closes the listener. Any pending accept promises will be rejected
     * with errors. */
    close(): void;
    /** Return the address of the `Listener`. */
    readonly addr: A;

    [Symbol.asyncIterator](): AsyncIterableIterator<T>;

    /**
     * Make the listener block the event loop from finishing.
     *
     * Note: the listener blocks the event loop from finishing by default.
     * This method is only meaningful after `.unref()` is called.
     */
    ref(): void;

    /** Make the listener not block the event loop from finishing. */
    unref(): void;
  }

  /** Specialized listener that accepts TLS connections.
   *
   * @category Network
   */
  export type TlsListener = Listener<TlsConn, NetAddr>;

  /** Specialized listener that accepts TCP connections.
   *
   * @category Network
   */
  export type TcpListener = Listener<TcpConn, NetAddr>;

  /** Specialized listener that accepts Unix connections.
   *
   * @category Network
   */
  export type UnixListener = Listener<UnixConn, UnixAddr>;

  /** Specialized listener that accepts VSOCK connections.
   *
   * @experimental **UNSTABLE**: New API, yet to be vetted.
   *
   * @category Network
   */
  export type VsockListener = Listener<VsockConn, VsockAddr>;

  /** @category Network */
  export interface Conn<A extends Addr = Addr> extends Disposable {
    /** Read the incoming data from the connection into an array buffer (`p`).
     *
     * Resolves to either the number of bytes read during the operation or EOF
     * (`null`) if there was nothing more to read.
     *
     * It is possible for a read to successfully return with `0` bytes. This
     * does not indicate EOF.
     *
     * **It is not guaranteed that the full buffer will be read in a single
     * call.**
     *
     * ```ts
     * // If the text "hello world" is received by the client:
     * const conn = await Deno.connect({ hostname: "example.com", port: 80 });
     * const buf = new Uint8Array(100);
     * const numberOfBytesRead = await conn.read(buf); // 11 bytes
     * const text = new TextDecoder().decode(buf);  // "hello world"
     * ```
     *
     * @category I/O
     */
    read(p: Uint8Array): Promise<number | null>;
    /** Write the contents of the array buffer (`p`) to the connection.
     *
     * Resolves to the number of bytes written.
     *
     * **It is not guaranteed that the full buffer will be written in a single
     * call.**
     *
     * ```ts
     * const conn = await Deno.connect({ hostname: "example.com", port: 80 });
     * const encoder = new TextEncoder();
     * const data = encoder.encode("Hello world");
     * const bytesWritten = await conn.write(data); // 11
     * ```
     *
     * @category I/O
     */
    write(p: Uint8Array): Promise<number>;
    /** Closes the connection, freeing the resource.
     *
     * ```ts
     * const conn = await Deno.connect({ hostname: "example.com", port: 80 });
     *
     * // ...
     *
     * conn.close();
     * ```
     */
    close(): void;
    /** The local address of the connection. */
    readonly localAddr: A;
    /** The remote address of the connection. */
    readonly remoteAddr: A;
    /** Shuts down (`shutdown(2)`) the write side of the connection. Most
     * callers should just use `close()`. */
    closeWrite(): Promise<void>;

    /** Make the connection block the event loop from finishing.
     *
     * Note: the connection blocks the event loop from finishing by default.
     * This method is only meaningful after `.unref()` is called.
     */
    ref(): void;
    /** Make the connection not block the event loop from finishing. */
    unref(): void;

    readonly readable: ReadableStream<Uint8Array<ArrayBuffer>>;
    readonly writable: WritableStream<Uint8Array<ArrayBufferLike>>;
  }

  /** @category Network */
  export interface TlsHandshakeInfo {
    /**
     * Contains the ALPN protocol selected during negotiation with the server.
     * If no ALPN protocol selected, returns `null`.
     */
    alpnProtocol: string | null;
  }

  /** @category Network */
  export interface TlsConn extends Conn<NetAddr> {
    /** Runs the client or server handshake protocol to completion if that has
     * not happened yet. Calling this method is optional; the TLS handshake
     * will be completed automatically as soon as data is sent or received. */
    handshake(): Promise<TlsHandshakeInfo>;
  }

  /** @category Network */
  export interface ListenOptions {
    /** The port to listen on.
     *
     * Set to `0` to listen on any available port.
     */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
     *
     * __Note about `0.0.0.0`__ While listening `0.0.0.0` works on all platforms,
     * the browsers on Windows don't work with the address `0.0.0.0`.
     * You should show the message like `server running on localhost:8080` instead of
     * `server running on 0.0.0.0:8080` if your program supports Windows.
     *
     * @default {"0.0.0.0"} */
    hostname?: string;
  }

  /** @category Network */
  export interface TcpListenOptions extends ListenOptions {
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
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function listen(
    options: TcpListenOptions & { transport?: "tcp" },
  ): TcpListener;

  /** Options which can be set when opening a Unix listener via
   * {@linkcode Deno.listen} or {@linkcode Deno.listenDatagram}.
   *
   * @category Network
   */
  export interface UnixListenOptions {
    /** A path to the Unix Socket. */
    path: string;
  }

  /** Listen announces on the local transport address.
   *
   * ```ts
   * const listener = Deno.listen({ path: "/foo/bar.sock", transport: "unix" })
   * ```
   *
   * Requires `allow-read` and `allow-write` permission.
   *
   * @tags allow-read, allow-write
   * @category Network
   */
  // deno-lint-ignore adjacent-overload-signatures
  export function listen(
    options: UnixListenOptions & { transport: "unix" },
  ): UnixListener;

  /** Options which can be set when opening a VSOCK listener via
   * {@linkcode Deno.listen}.
   *
   * @experimental **UNSTABLE**: New API, yet to be vetted.
   *
   * @category Network
   */
  export interface VsockListenOptions {
    cid: number;
    port: number;
  }

  /** Listen announces on the local transport address.
   *
   * @experimental **UNSTABLE**: New API, yet to be vetted.
   *
   * The VSOCK address family facilitates communication between virtual machines and the host they are running on: https://man7.org/linux/man-pages/man7/vsock.7.html
   *
   * ```ts
   * const listener = Deno.listen({ cid: -1, port: 80, transport: "vsock" })
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  // deno-lint-ignore adjacent-overload-signatures
  export function listen(
    options: VsockListenOptions & { transport: "vsock" },
  ): VsockListener;

  /**
   * Provides certified key material from strings. The key material is provided in
   * `PEM`-format (Privacy Enhanced Mail, https://www.rfc-editor.org/rfc/rfc1422) which can be identified by having
   * `-----BEGIN-----` and `-----END-----` markers at the beginning and end of the strings. This type of key is not compatible
   * with `DER`-format keys which are binary.
   *
   * Deno supports RSA, EC, and PKCS8-format keys.
   *
   * ```ts
   * const key = {
   *  key: "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----\n",
   *  cert: "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----\n" }
   * };
   * ```
   *
   * @category Network
   */
  export interface TlsCertifiedKeyPem {
    /** The format of this key material, which must be PEM. */
    keyFormat?: "pem";
    /** Private key in `PEM` format. RSA, EC, and PKCS8-format keys are supported. */
    key: string;
    /** Certificate chain in `PEM` format. */
    cert: string;
  }

  /** @category Network */
  export interface ListenTlsOptions extends TcpListenOptions {
    transport?: "tcp";

    /** Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** Listen announces on the local transport address over TLS (transport layer
   * security).
   *
   * ```ts
   * using listener = Deno.listenTls({
   *   port: 443,
   *   cert: Deno.readTextFileSync("./server.crt"),
   *   key: Deno.readTextFileSync("./server.key"),
   * });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function listenTls(
    options: ListenTlsOptions & TlsCertifiedKeyPem,
  ): TlsListener;

  /** @category Network */
  export interface ConnectOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
     * If not specified,
     *
     * @default {"127.0.0.1"} */
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
   * Requires `allow-net` permission for "tcp".
   *
   * @tags allow-net
   * @category Network
   */
  export function connect(options: ConnectOptions): Promise<TcpConn>;

  /** @category Network */
  export interface TcpConn extends Conn<NetAddr> {
    /**
     * Enable/disable the use of Nagle's algorithm.
     *
     * @param [noDelay=true]
     */
    setNoDelay(noDelay?: boolean): void;
    /** Enable/disable keep-alive functionality. */
    setKeepAlive(keepAlive?: boolean): void;
  }

  /** @category Network */
  export interface UnixConnectOptions {
    transport: "unix";
    path: string;
  }

  /** @category Network */
  export interface UnixConn extends Conn<UnixAddr> {}

  /** Connects to the hostname (default is "127.0.0.1") and port on the named
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
   * Requires `allow-net` permission for "tcp" and `allow-read` for "unix".
   *
   * @tags allow-net, allow-read
   * @category Network
   */
  // deno-lint-ignore adjacent-overload-signatures
  export function connect(options: UnixConnectOptions): Promise<UnixConn>;

  /**
   * @experimental **UNSTABLE**: New API, yet to be vetted.
   * @category Network
   */
  export interface VsockConnectOptions {
    transport: "vsock";
    cid: number;
    port: number;
  }

  /** @category Network */
  export interface VsockConn extends Conn<VsockAddr> {}

  /** Connects to the hostname (default is "127.0.0.1") and port on the named
   * transport (default is "tcp"), and resolves to the connection (`Conn`).
   *
   * @experimental **UNSTABLE**: New API, yet to be vetted.
   *
   * ```ts
   * const conn1 = await Deno.connect({ port: 80 });
   * const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
   * const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
   * const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
   * const conn5 = await Deno.connect({ path: "/foo/bar.sock", transport: "unix" });
   * const conn6 = await Deno.connect({ cid: -1, port: 80, transport: "vsock" });
   * ```
   *
   * Requires `allow-net` permission for "tcp" and "vsock", and `allow-read` for "unix".
   *
   * @tags allow-net, allow-read
   * @category Network
   */
  // deno-lint-ignore adjacent-overload-signatures
  export function connect(options: VsockConnectOptions): Promise<VsockConn>;

  /** @category Network */
  export interface ConnectTlsOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
     *
     * @default {"127.0.0.1"} */
    hostname?: string;
    /** A list of root certificates that will be used in addition to the
     * default root certificates to verify the peer's certificate.
     *
     * Must be in PEM format. */
    caCerts?: string[];
    /** Application-Layer Protocol Negotiation (ALPN) protocols supported by
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** Establishes a secure connection over TLS (transport layer security) using
   * an optional list of CA certs, hostname (default is "127.0.0.1") and port.
   *
   * The CA cert list is optional and if not included Mozilla's root
   * certificates will be used (see also https://github.com/ctz/webpki-roots for
   * specifics).
   *
   * Mutual TLS (mTLS or client certificates) are supported by providing a
   * `key` and `cert` in the options as PEM-encoded strings.
   *
   * ```ts
   * const caCert = await Deno.readTextFile("./certs/my_custom_root_CA.pem");
   * const conn1 = await Deno.connectTls({ port: 80 });
   * const conn2 = await Deno.connectTls({ caCerts: [caCert], hostname: "192.0.2.1", port: 80 });
   * const conn3 = await Deno.connectTls({ hostname: "[2001:db8::1]", port: 80 });
   * const conn4 = await Deno.connectTls({ caCerts: [caCert], hostname: "golang.org", port: 80});
   *
   * const key = "----BEGIN PRIVATE KEY----...";
   * const cert = "----BEGIN CERTIFICATE----...";
   * const conn5 = await Deno.connectTls({ port: 80, key, cert });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function connectTls(
    options: ConnectTlsOptions | (ConnectTlsOptions & TlsCertifiedKeyPem),
  ): Promise<TlsConn>;

  /** @category Network */
  export interface StartTlsOptions {
    /** A literal IP address or host name that can be resolved to an IP address.
     *
     * @default {"127.0.0.1"} */
    hostname?: string;
    /** A list of root certificates that will be used in addition to the
     * default root certificates to verify the peer's certificate.
     *
     * Must be in PEM format. */
    caCerts?: string[];
    /** Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** Start TLS handshake from an existing connection using an optional list of
   * CA certificates, and hostname (default is "127.0.0.1"). Specifying CA certs
   * is optional. By default the configured root certificates are used. Using
   * this function requires that the other end of the connection is prepared for
   * a TLS handshake.
   *
   * Note that this function *consumes* the TCP connection passed to it, thus the
   * original TCP connection will be unusable after calling this. Additionally,
   * you need to ensure that the TCP connection is not being used elsewhere when
   * calling this function in order for the TCP connection to be consumed properly.
   * For instance, if there is a `Promise` that is waiting for read operation on
   * the TCP connection to complete, it is considered that the TCP connection is
   * being used elsewhere. In such a case, this function will fail.
   *
   * ```ts
   * const conn = await Deno.connect({ port: 80, hostname: "127.0.0.1" });
   * const caCert = await Deno.readTextFile("./certs/my_custom_root_CA.pem");
   * // `conn` becomes unusable after calling `Deno.startTls`
   * const tlsConn = await Deno.startTls(conn, { caCerts: [caCert], hostname: "localhost" });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function startTls(
    conn: TcpConn,
    options?: StartTlsOptions,
  ): Promise<TlsConn>;

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface QuicEndpointOptions {
    /**
     * A literal IP address or host name that can be resolved to an IP address.
     * @default {"::"}
     */
    hostname?: string;
    /**
     * The port to bind to.
     * @default {0}
     */
    port?: number;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface QuicTransportOptions {
    /** Period of inactivity before sending a keep-alive packet. Keep-alive
     * packets prevent an inactive but otherwise healthy connection from timing
     * out. Only one side of any given connection needs keep-alive enabled for
     * the connection to be preserved.
     * @default {undefined}
     */
    keepAliveInterval?: number;
    /** Maximum duration of inactivity to accept before timing out the
     * connection. The true idle timeout is the minimum of this and the peer’s
     * own max idle timeout.
     * @default {undefined}
     */
    maxIdleTimeout?: number;
    /** Maximum number of incoming bidirectional streams that may be open
     * concurrently.
     * @default {100}
     */
    maxConcurrentBidirectionalStreams?: number;
    /** Maximum number of incoming unidirectional streams that may be open
     * concurrently.
     * @default {100}
     */
    maxConcurrentUnidirectionalStreams?: number;
    /**
     * The congestion control algorithm used when sending data over this connection.
     * @default {"default"}
     */
    congestionControl?: "throughput" | "low-latency" | "default";
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface ConnectQuicOptions<ZRTT extends boolean>
    extends QuicTransportOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address. */
    hostname: string;
    /** The name used for validating the certificate provided by the server. If
     * not provided, defaults to `hostname`. */
    serverName?: string | undefined;
    /** Application-Layer Protocol Negotiation (ALPN) protocols supported by
     * the client. QUIC requires the use of ALPN.
     */
    alpnProtocols: string[];
    /** A list of root certificates that will be used in addition to the
     * default root certificates to verify the peer's certificate.
     *
     * Must be in PEM format. */
    caCerts?: string[];
    /**
     * If no endpoint is provided, a new one is bound on an ephemeral port.
     */
    endpoint?: QuicEndpoint;
    /**
     * Attempt to convert the connection into 0-RTT. Any data sent before
     * the TLS handshake completes is vulnerable to replay attacks.
     * @default {false}
     */
    zeroRtt?: ZRTT;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface QuicServerTransportOptions extends QuicTransportOptions {
    /**
     * Preferred IPv4 address to be communicated to the client during
     * handshaking. If the client is able to reach this address it will switch
     * to it.
     * @default {undefined}
     */
    preferredAddressV4?: string;
    /**
     * Preferred IPv6 address to be communicated to the client during
     * handshaking. If the client is able to reach this address it will switch
     * to it.
     * @default {undefined}
     */
    preferredAddressV6?: string;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface QuicListenOptions extends QuicServerTransportOptions {
    /** Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. QUIC requires the use of ALPN.
     */
    alpnProtocols: string[];
    /** Server private key in PEM format */
    key: string;
    /** Cert chain in PEM format */
    cert: string;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface QuicAcceptOptions<ZRTT extends boolean>
    extends QuicServerTransportOptions {
    /** Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. QUIC requires the use of ALPN.
     */
    alpnProtocols?: string[];
    /**
     * Convert this connection into 0.5-RTT at the cost of weakened security, as
     * 0.5-RTT data may be sent before TLS client authentication has occurred.
     * @default {false}
     */
    zeroRtt?: ZRTT;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export interface QuicCloseInfo {
    /** A number representing the error code for the error. */
    closeCode: number;
    /** A string representing the reason for closing the connection. */
    reason: string;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * @experimental
   * @category Network
   */
  export interface QuicSendStreamOptions {
    /** Indicates the send priority of this stream relative to other streams for
     * which the value has been set.
     * @default {0}
     */
    sendOrder?: number;
    /** Wait until there is sufficient flow credit to create the stream.
     * @default {false}
     */
    waitUntilAvailable?: boolean;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * @experimental
   * @category Network
   */
  export class QuicEndpoint {
    /**
     * Create a QUIC endpoint which may be used for client or server connections.
     *
     * Requires `allow-net` permission.
     *
     * @experimental
     * @tags allow-net
     * @category Network
     */
    constructor(options?: QuicEndpointOptions);

    /** Return the address of the `QuicListener`. */
    readonly addr: NetAddr;

    /**
     * **UNSTABLE**: New API, yet to be vetted.
     * Listen announces on the local transport address over QUIC.
     *
     * @experimental
     * @category Network
     */
    listen(options: QuicListenOptions): QuicListener;

    /**
     * Closes the endpoint. All associated connections will be closed and incoming
     * connections will be rejected.
     */
    close(info?: QuicCloseInfo): void;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * Specialized listener that accepts QUIC connections.
   *
   * @experimental
   * @category Network
   */
  export interface QuicListener extends AsyncIterable<QuicConn> {
    /** Waits for and resolves to the next incoming connection. */
    incoming(): Promise<QuicIncoming>;

    /** Wait for the next incoming connection and accepts it. */
    accept(): Promise<QuicConn>;

    /** Stops the listener. This does not close the endpoint. */
    stop(): void;

    [Symbol.asyncIterator](): AsyncIterableIterator<QuicConn>;

    /** The endpoint for this listener. */
    readonly endpoint: QuicEndpoint;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * An incoming connection for which the server has not yet begun its part of
   * the handshake.
   *
   * @experimental
   * @category Network
   */
  export interface QuicIncoming {
    /**
     * The local IP address which was used when the peer established the
     * connection.
     */
    readonly localIp: string;

    /**
     * The peer’s UDP address.
     */
    readonly remoteAddr: NetAddr;

    /**
     * Whether the socket address that is initiating this connection has proven
     * that they can receive traffic.
     */
    readonly remoteAddressValidated: boolean;

    /**
     * Accept this incoming connection.
     */
    accept<ZRTT extends boolean>(
      options?: QuicAcceptOptions<ZRTT>,
    ): ZRTT extends true ? QuicConn : Promise<QuicConn>;

    /**
     * Refuse this incoming connection.
     */
    refuse(): void;

    /**
     * Ignore this incoming connection attempt, not sending any packet in response.
     */
    ignore(): void;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * @experimental
   * @category Network
   */
  export interface QuicConn {
    /** Close closes the listener. Any pending accept promises will be rejected
     * with errors. */
    close(info?: QuicCloseInfo): void;
    /** Opens and returns a bidirectional stream. */
    createBidirectionalStream(
      options?: QuicSendStreamOptions,
    ): Promise<QuicBidirectionalStream>;
    /** Opens and returns a unidirectional stream. */
    createUnidirectionalStream(
      options?: QuicSendStreamOptions,
    ): Promise<QuicSendStream>;
    /** Send a datagram. The provided data cannot be larger than
     * `maxDatagramSize`. */
    sendDatagram(data: Uint8Array): Promise<void>;
    /** Receive a datagram. */
    readDatagram(): Promise<Uint8Array<ArrayBuffer>>;

    /** The endpoint for this connection. */
    readonly endpoint: QuicEndpoint;
    /** Returns a promise that resolves when the TLS handshake is complete. */
    readonly handshake: Promise<void>;
    /** Return the remote address for the connection. Clients may change
     * addresses at will, for example when switching to a cellular internet
     * connection.
     */
    readonly remoteAddr: NetAddr;
    /**
     * The negotiated ALPN protocol, if provided. Only available after the
     * handshake is complete. */
    readonly protocol: string | undefined;
    /** The negotiated server name. Only available on the server after the
     * handshake is complete. */
    readonly serverName: string | undefined;
    /** Returns a promise that resolves when the connection is closed. */
    readonly closed: Promise<QuicCloseInfo>;
    /** A stream of bidirectional streams opened by the peer. */
    readonly incomingBidirectionalStreams: ReadableStream<
      QuicBidirectionalStream
    >;
    /** A stream of unidirectional streams opened by the peer. */
    readonly incomingUnidirectionalStreams: ReadableStream<QuicReceiveStream>;
    /** Returns the datagram stream for sending and receiving datagrams. */
    readonly maxDatagramSize: number;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * @experimental
   * @category Network
   */
  export interface QuicBidirectionalStream {
    /** Returns a QuicReceiveStream instance that can be used to read incoming data. */
    readonly readable: QuicReceiveStream;
    /** Returns a QuicSendStream instance that can be used to write outgoing data. */
    readonly writable: QuicSendStream;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * @experimental
   * @category Network
   */
  export interface QuicSendStream
    extends WritableStream<Uint8Array<ArrayBufferLike>> {
    /** Indicates the send priority of this stream relative to other streams for
     * which the value has been set. */
    sendOrder: number;

    /**
     * 62-bit stream ID, unique within this connection.
     */
    readonly id: bigint;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * @experimental
   * @category Network
   */
  export interface QuicReceiveStream
    extends ReadableStream<Uint8Array<ArrayBuffer>> {
    /**
     * 62-bit stream ID, unique within this connection.
     */
    readonly id: bigint;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   * Establishes a secure connection over QUIC using a hostname and port.  The
   * cert file is optional and if not included Mozilla's root certificates will
   * be used. See also https://github.com/ctz/webpki-roots for specifics.
   *
   * ```ts
   * const caCert = await Deno.readTextFile("./certs/my_custom_root_CA.pem");
   * const conn1 = await Deno.connectQuic({ hostname: "example.com", port: 443, alpnProtocols: ["h3"] });
   * const conn2 = await Deno.connectQuic({ caCerts: [caCert], hostname: "example.com", port: 443, alpnProtocols: ["h3"] });
   * ```
   *
   * If an endpoint is shared among many connections, 0-RTT can be enabled.
   * When 0-RTT is successful, a QuicConn will be synchronously returned
   * and data can be sent immediately with it. **Any data sent before the
   * TLS handshake completes is vulnerable to replay attacks.**
   *
   * Requires `allow-net` permission.
   *
   * @experimental
   * @tags allow-net
   * @category Network
   */
  export function connectQuic<ZRTT extends boolean>(
    options: ConnectQuicOptions<ZRTT>,
  ): ZRTT extends true ? (QuicConn | Promise<QuicConn>) : Promise<QuicConn>;

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * Upgrade a QUIC connection into a WebTransport instance.
   *
   * @category Network
   * @experimental
   */
  export function upgradeWebTransport(
    conn: QuicConn,
  ): Promise<WebTransport & { url: string }>;

  export {}; // only export exports
}
