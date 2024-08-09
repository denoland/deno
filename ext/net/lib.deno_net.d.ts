// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

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

  /** @category Network */
  export type Addr = NetAddr | UnixAddr;

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

    /**
     * Return the rid of the `Listener`.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    readonly rid: number;

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

  /** @category Network */
  export interface Conn<A extends Addr = Addr>
    extends Reader, Writer, Closer, Disposable {
    /** The local address of the connection. */
    readonly localAddr: A;
    /** The remote address of the connection. */
    readonly remoteAddr: A;
    /**
     * The resource ID of the connection.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    readonly rid: number;
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

    /**
     * A readable stream representing the data on the connection.
     *
     * When the connection is closed (EOF is encountered), the stream will also
     * be closed. By default, when the stream is closed, the underlying
     * network connection will also be closed. This means in particular that if
     * `readable` is closed due to an EOF, the entire connection will be closed
     * and this the `writable` end of the stream will also be closed.
     *
     * To prevent this behavior, you can set the `preventCloseOnEOF` option to
     * `true` when creating the connection. This will prevent the connection
     * from being closed when the stream is closed due to an EOF.
     */
    readonly readable: ReadableStream<Uint8Array>;
    /**
     * A writable stream representing the data on the connection.
     *
     * When the connection is closed, the stream will also be closed. When the
     * stream is closed, the underlying network connection will also be closed.
     * This means in particular that if `writable` is closed, the entire
     * connection will be closed and thus the `readable` end of the stream will
     * also be closed.
     *
     * In cases where one wants to keep the connection open after writing to the
     * writable, do not close the writable stream. For example, when using
     * `ReadableStream#pipeTo()` to pipe a readable stream to the writable
     * stream, pass the `preventClose` option with a value of `true` to prevent
     * the writable stream from being closed when the readable stream is closed.
     */
    readonly writable: WritableStream<Uint8Array>;
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
    /**
     * The resource ID of the connection.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    readonly rid: number;
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

  /** Provides TLS certified keys, ie: a key that has been certified by a trusted certificate authority.
   * A certified key generally consists of a private key and certificate part.
   *
   * @category Network
   */
  export type TlsCertifiedKeyOptions =
    | TlsCertifiedKeyPem
    | TlsCertifiedKeyFromFile
    | TlsCertifiedKeyConnectTls;

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

  /**
   * @deprecated This will be removed in Deno 2.0. See the
   * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
   * for migration instructions.
   *
   * @category Network
   */
  export interface TlsCertifiedKeyFromFile {
    /** Path to a file containing a PEM formatted CA certificate. Requires
     * `--allow-read`.
     *
     * @tags allow-read
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    certFile: string;
    /** Path to a file containing a private key file. Requires `--allow-read`.
     *
     * @tags allow-read
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    keyFile: string;
  }

  /**
   * @deprecated This will be removed in Deno 2.0. See the
   * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
   * for migration instructions.
   *
   * @category Network
   */
  export interface TlsCertifiedKeyConnectTls {
    /**
     * Certificate chain in `PEM` format.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    certChain: string;
    /**
     * Private key in `PEM` format. RSA, EC, and PKCS8-format keys are supported.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    privateKey: string;
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
    options: ListenTlsOptions & TlsCertifiedKeyOptions,
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
    /** Prevent the connection from being closed automatically when the readable
     * stream reaches EOF. This allows the connection to be reused for further
     * writes or reads.
     *
     * @default {false} */
    preventCloseOnEOF?: boolean;
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
    /**
     * The resource ID of the connection.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    readonly rid: number;
  }

  /** @category Network */
  export interface UnixConnectOptions {
    transport: "unix";
    /**
     * The path to the Unix Socket to connect to.
     */
    path: string;
    /** Prevent the connection from being closed automatically when the readable
     * stream reaches EOF. This allows the connection to be reused for further
     * writes or reads.
     *
     * @default {false} */
    preventCloseOnEOF?: boolean;
  }

  /** @category Network */
  export interface UnixConn extends Conn<UnixAddr> {
    /**
     * The resource ID of the connection.
     *
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    readonly rid: number;
  }

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

  /** @category Network */
  export interface ConnectTlsOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
     *
     * @default {"127.0.0.1"} */
    hostname?: string;
    /** Path to a file containing a PEM formatted list of root certificates that will
     * be used in addition to the default root certificates to verify the peer's certificate. Requires
     * `--allow-read`.
     *
     * @tags allow-read
     * @deprecated This will be removed in Deno 2.0. See the
     * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
     * for migration instructions.
     */
    certFile?: string;
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
    /** Prevent the connection from being closed automatically when the readable
     * stream reaches EOF. This allows the connection to be reused for further
     * writes or reads.
     *
     * @default {false} */
    preventCloseOnEOF?: boolean;
  }

  /** Establishes a secure connection over TLS (transport layer security) using
   * an optional cert file, hostname (default is "127.0.0.1") and port.  The
   * cert file is optional and if not included Mozilla's root certificates will
   * be used (see also https://github.com/ctz/webpki-roots for specifics)
   *
   * ```ts
   * const caCert = await Deno.readTextFile("./certs/my_custom_root_CA.pem");
   * const conn1 = await Deno.connectTls({ port: 80 });
   * const conn2 = await Deno.connectTls({ caCerts: [caCert], hostname: "192.0.2.1", port: 80 });
   * const conn3 = await Deno.connectTls({ hostname: "[2001:db8::1]", port: 80 });
   * const conn4 = await Deno.connectTls({ caCerts: [caCert], hostname: "golang.org", port: 80});
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function connectTls(options: ConnectTlsOptions): Promise<TlsConn>;

  /** Establishes a secure connection over TLS (transport layer security) using
   * an optional cert file, client certificate, hostname (default is "127.0.0.1") and
   * port.  The cert file is optional and if not included Mozilla's root certificates will
   * be used (see also https://github.com/ctz/webpki-roots for specifics)
   *
   * ```ts
   * const caCert = await Deno.readTextFile("./certs/my_custom_root_CA.pem");
   * const key = "----BEGIN PRIVATE KEY----...";
   * const cert = "----BEGIN CERTIFICATE----...";
   * const conn1 = await Deno.connectTls({ port: 80, key, cert });
   * const conn2 = await Deno.connectTls({ caCerts: [caCert], hostname: "192.0.2.1", port: 80, key, cert });
   * const conn3 = await Deno.connectTls({ hostname: "[2001:db8::1]", port: 80, key, cert });
   * const conn4 = await Deno.connectTls({ caCerts: [caCert], hostname: "golang.org", port: 80, key, cert });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function connectTls(
    options: ConnectTlsOptions & TlsCertifiedKeyOptions,
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
    /** Prevent the connection from being closed automatically when the readable
     * stream reaches EOF. This allows the connection to be reused for further
     * writes or reads.
     *
     * @default {false} */
    preventCloseOnEOF?: boolean;
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

  /** Shutdown socket send operations.
   *
   * Matches behavior of POSIX shutdown(3).
   *
   * ```ts
   * const listener = Deno.listen({ port: 80 });
   * const conn = await listener.accept();
   * Deno.shutdown(conn.rid);
   * ```
   *
   * @deprecated This will be removed in Deno 2.0. See the
   * {@link https://docs.deno.com/runtime/manual/advanced/migrate_deprecations | Deno 1.x to 2.x Migration Guide}
   * for migration instructions.
   *
   * @category Network
   */
  export function shutdown(rid: number): Promise<void>;
}
