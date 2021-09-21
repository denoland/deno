// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />

declare namespace Deno {
  /**
   * **UNSTABLE**: New API, yet to be vetted.  This API is under consideration to
   * determine if permissions are required to call it.
   *
   * Retrieve the process umask.  If `mask` is provided, sets the process umask.
   * This call always returns what the umask was before the call.
   *
   * ```ts
   * console.log(Deno.umask());  // e.g. 18 (0o022)
   * const prevUmaskValue = Deno.umask(0o077);  // e.g. 18 (0o022)
   * console.log(Deno.umask());  // e.g. 63 (0o077)
   * ```
   *
   * NOTE:  This API is not implemented on Windows
   */
  export function umask(mask?: number): number;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Gets the size of the console as columns/rows.
   *
   * ```ts
   * const { columns, rows } = Deno.consoleSize(Deno.stdout.rid);
   * ```
   */
  export function consoleSize(
    rid: number,
  ): {
    columns: number;
    rows: number;
  };

  /** **Unstable**  There are questions around which permission this needs. And
   * maybe should be renamed (loadAverage?)
   *
   * Returns an array containing the 1, 5, and 15 minute load averages. The
   * load average is a measure of CPU and IO utilization of the last one, five,
   * and 15 minute periods expressed as a fractional number.  Zero means there
   * is no load. On Windows, the three values are always the same and represent
   * the current load, not the 1, 5 and 15 minute load averages.
   *
   * ```ts
   * console.log(Deno.loadavg());  // e.g. [ 0.71, 0.44, 0.44 ]
   * ```
   *
   * Requires `allow-env` permission.
   */
  export function loadavg(): number[];

  /** **Unstable** new API. yet to be vetted. Under consideration to possibly move to
   * Deno.build or Deno.versions and if it should depend sys-info, which may not
   * be desireable.
   *
   * Returns the release version of the Operating System.
   *
   * ```ts
   * console.log(Deno.osRelease());
   * ```
   *
   * Requires `allow-env` permission.
   */
  export function osRelease(): string;

  /** **Unstable** new API. yet to be vetted.
   *
   * Displays the total amount of free and used physical and swap memory in the
   * system, as well as the buffers and caches used by the kernel.
   *
   * This is similar to the `free` command in Linux
   *
   * ```ts
   * console.log(Deno.systemMemoryInfo());
   * ```
   *
   * Requires `allow-env` permission.
   */
  export function systemMemoryInfo(): SystemMemoryInfo;

  export interface SystemMemoryInfo {
    /** Total installed memory */
    total: number;
    /** Unused memory */
    free: number;
    /** Estimation of how much memory is available  for  starting  new
     * applications, without  swapping. Unlike the data provided by the cache or
     * free fields, this field takes into account page cache and also that not
     * all reclaimable memory slabs will be reclaimed due to items being in use
     */
    available: number;
    /** Memory used by kernel buffers */
    buffers: number;
    /** Memory  used  by  the  page  cache  and  slabs */
    cached: number;
    /** Total swap memory */
    swapTotal: number;
    /** Unused swap memory */
    swapFree: number;
  }

  /** All possible types for interfacing with foreign functions */
  export type NativeType =
    | "void"
    | "u8"
    | "i8"
    | "u16"
    | "i16"
    | "u32"
    | "i32"
    | "u64"
    | "i64"
    | "usize"
    | "isize"
    | "f32"
    | "f64";

  /** A foreign function as defined by its parameter and result types */
  export interface ForeignFunction {
    parameters: NativeType[];
    result: NativeType;
  }

  /** A dynamic library resource */
  export interface DynamicLibrary<S extends Record<string, ForeignFunction>> {
    /** All of the registered symbols along with functions for calling them */
    symbols: { [K in keyof S]: (...args: unknown[]) => unknown };

    close(): void;
  }

  /** **UNSTABLE**: new API
   *
   * Opens a dynamic library and registers symbols
   */
  export function dlopen<S extends Record<string, ForeignFunction>>(
    filename: string | URL,
    symbols: S,
  ): DynamicLibrary<S>;

  /** **UNSTABLE**: Should not have same name as `window.location` type. */
  interface Location {
    /** The full url for the module, e.g. `file://some/file.ts` or
     * `https://some/file.ts`. */
    fileName: string;
    /** The line number in the file. It is assumed to be 1-indexed. */
    lineNumber: number;
    /** The column number in the file. It is assumed to be 1-indexed. */
    columnNumber: number;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Given a current location in a module, lookup the source location and return
   * it.
   *
   * When Deno transpiles code, it keep source maps of the transpiled code. This
   * function can be used to lookup the original location. This is
   * automatically done when accessing the `.stack` of an error, or when an
   * uncaught error is logged. This function can be used to perform the lookup
   * for creating better error handling.
   *
   * **Note:** `lineNumber` and `columnNumber` are 1 indexed, which matches display
   * expectations, but is not typical of most index numbers in Deno.
   *
   * An example:
   *
   * ```ts
   * const origin = Deno.applySourceMap({
   *   fileName: "file://my/module.ts",
   *   lineNumber: 5,
   *   columnNumber: 15
   * });
   *
   * console.log(`${origin.fileName}:${origin.lineNumber}:${origin.columnNumber}`);
   * ```
   */
  export function applySourceMap(location: Location): Location;

  export type Signal =
    | "SIGABRT"
    | "SIGALRM"
    | "SIGBUS"
    | "SIGCHLD"
    | "SIGCONT"
    | "SIGEMT"
    | "SIGFPE"
    | "SIGHUP"
    | "SIGILL"
    | "SIGINFO"
    | "SIGINT"
    | "SIGIO"
    | "SIGKILL"
    | "SIGPIPE"
    | "SIGPROF"
    | "SIGPWR"
    | "SIGQUIT"
    | "SIGSEGV"
    | "SIGSTKFLT"
    | "SIGSTOP"
    | "SIGSYS"
    | "SIGTERM"
    | "SIGTRAP"
    | "SIGTSTP"
    | "SIGTTIN"
    | "SIGTTOU"
    | "SIGURG"
    | "SIGUSR1"
    | "SIGUSR2"
    | "SIGVTALRM"
    | "SIGWINCH"
    | "SIGXCPU"
    | "SIGXFSZ";

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Represents the stream of signals, implements both `AsyncIterator` and
   * `PromiseLike`. */
  export class SignalStream
    implements AsyncIterableIterator<void>, PromiseLike<void> {
    constructor(signal: Signal);
    then<T, S>(
      f: (v: void) => T | Promise<T>,
      g?: (v: void) => S | Promise<S>,
    ): Promise<T | S>;
    next(): Promise<IteratorResult<void>>;
    [Symbol.asyncIterator](): AsyncIterableIterator<void>;
    dispose(): void;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Returns the stream of the given signal number. You can use it as an async
   * iterator.
   *
   * ```ts
   * for await (const _ of Deno.signal("SIGTERM")) {
   *   console.log("got SIGTERM!");
   * }
   * ```
   *
   * You can also use it as a promise. In this case you can only receive the
   * first one.
   *
   * ```ts
   * await Deno.signal("SIGTERM");
   * console.log("SIGTERM received!")
   * ```
   *
   * If you want to stop receiving the signals, you can use `.dispose()` method
   * of the signal stream object.
   *
   * ```ts
   * const sig = Deno.signal("SIGTERM");
   * setTimeout(() => { sig.dispose(); }, 5000);
   * for await (const _ of sig) {
   *   console.log("SIGTERM!")
   * }
   * ```
   *
   * The above for-await loop exits after 5 seconds when `sig.dispose()` is
   * called.
   *
   * NOTE: This functionality is not yet implemented on Windows.
   */
  export function signal(sig: Signal): SignalStream;

  export type SetRawOptions = {
    cbreak: boolean;
  };

  /** **UNSTABLE**: new API, yet to be vetted
   *
   * Set TTY to be under raw mode or not. In raw mode, characters are read and
   * returned as is, without being processed. All special processing of
   * characters by the terminal is disabled, including echoing input characters.
   * Reading from a TTY device in raw mode is faster than reading from a TTY
   * device in canonical mode.
   *
   * The `cbreak` option can be used to indicate that characters that correspond
   * to a signal should still be generated. When disabling raw mode, this option
   * is ignored. This functionality currently only works on Linux and Mac OS.
   *
   * ```ts
   * Deno.setRaw(Deno.stdin.rid, true, { cbreak: true });
   * ```
   */
  export function setRaw(
    rid: number,
    mode: boolean,
    options?: SetRawOptions,
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Synchronously changes the access (`atime`) and modification (`mtime`) times
   * of a file system object referenced by `path`. Given times are either in
   * seconds (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * Deno.utimeSync("myfile.txt", 1556495550, new Date());
   * ```
   *
   * Requires `allow-write` permission. */
  export function utimeSync(
    path: string | URL,
    atime: number | Date,
    mtime: number | Date,
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Changes the access (`atime`) and modification (`mtime`) times of a file
   * system object referenced by `path`. Given times are either in seconds
   * (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * await Deno.utime("myfile.txt", 1556495550, new Date());
   * ```
   *
   * Requires `allow-write` permission. */
  export function utime(
    path: string | URL,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

  export function run<
    T extends RunOptions & {
      clearEnv?: boolean;
      gid?: number;
      uid?: number;
    } = RunOptions & {
      clearEnv?: boolean;
      gid?: number;
      uid?: number;
    },
  >(opt: T): Process<T>;

  /** **UNSTABLE**: Send a signal to process under given `pid`. This
   * functionality only works on Linux and Mac OS.
   *
   * If `pid` is negative, the signal will be sent to the process group
   * identified by `pid`.
   *
   *      const p = Deno.run({
   *        cmd: ["sleep", "10000"]
   *      });
   *
   *      Deno.kill(p.pid, "SIGINT");
   *
   * Requires `allow-run` permission. */
  export function kill(pid: number, signo: Signal): void;

  /**  **UNSTABLE**: New API, yet to be vetted.  Additional consideration is still
   * necessary around the permissions required.
   *
   * Get the `hostname` of the machine the Deno process is running on.
   *
   * ```ts
   * console.log(Deno.hostname());
   * ```
   *
   *  Requires `allow-env` permission.
   */
  export function hostname(): string;

  /** **UNSTABLE**: New API, yet to be vetted.
   * A custom HttpClient for use with `fetch`.
   *
   * ```ts
   * const client = Deno.createHttpClient({ caData: await Deno.readTextFile("./ca.pem") });
   * const req = await fetch("https://myserver.com", { client });
   * ```
   */
  export class HttpClient {
    rid: number;
    close(): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   * The options used when creating a [HttpClient].
   */
  export interface CreateHttpClientOptions {
    /** A certificate authority to use when validating TLS certificates. Certificate data must be PEM encoded.
     */
    caData?: string;
    proxy?: Proxy;
    certChain?: string;
    privateKey?: string;
  }

  export interface Proxy {
    url: string;
    basicAuth?: BasicAuth;
  }

  export interface BasicAuth {
    username: string;
    password: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   * Create a custom HttpClient for to use with `fetch`.
   *
   * ```ts
   * const client = Deno.createHttpClient({ caData: await Deno.readTextFile("./ca.pem") });
   * const response = await fetch("https://myserver.com", { client });
   * ```
   *
   * ```ts
   * const client = Deno.createHttpClient({ proxy: { url: "http://myproxy.com:8080" } });
   * const response = await fetch("https://myserver.com", { client });
   * ```
   */
  export function createHttpClient(
    options: CreateHttpClientOptions,
  ): HttpClient;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Synchronously changes the access (`atime`) and modification (`mtime`) times
   * of a file stream resource referenced by `rid`. Given times are either in
   * seconds (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * const file = Deno.openSync("file.txt", { create: true, write: true });
   * Deno.futimeSync(file.rid, 1556495550, new Date());
   * ```
   */
  export function futimeSync(
    rid: number,
    atime: number | Date,
    mtime: number | Date,
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Changes the access (`atime`) and modification (`mtime`) times of a file
   * stream resource referenced by `rid`. Given times are either in seconds
   * (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * const file = await Deno.open("file.txt", { create: true, write: true });
   * await Deno.futime(file.rid, 1556495550, new Date());
   * ```
   */
  export function futime(
    rid: number,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

  /** *UNSTABLE**: new API, yet to be vetted.
   *
   * SleepSync puts the main thread to sleep synchronously for a given amount of
   * time in milliseconds.
   *
   * ```ts
   * Deno.sleepSync(10);
   * ```
   */
  export function sleepSync(millis: number): void;

  export interface Metrics extends OpMetrics {
    ops: Record<string, OpMetrics>;
  }

  export interface OpMetrics {
    opsDispatched: number;
    opsDispatchedSync: number;
    opsDispatchedAsync: number;
    opsDispatchedAsyncUnref: number;
    opsCompleted: number;
    opsCompletedSync: number;
    opsCompletedAsync: number;
    opsCompletedAsyncUnref: number;
    bytesSentControl: number;
    bytesSentData: number;
    bytesReceived: number;
  }

  /** **UNSTABLE**: New option, yet to be vetted. */
  export interface TestDefinition {
    /** Specifies the permissions that should be used to run the test.
     * Set this to "inherit" to keep the calling thread's permissions.
     * Set this to "none" to revoke all permissions.
     *
     * Defaults to "inherit".
     */
    permissions?: "inherit" | "none" | {
      /** Specifies if the `net` permission should be requested or revoked.
       * If set to `"inherit"`, the current `env` permission will be inherited.
       * If set to `true`, the global `net` permission will be requested.
       * If set to `false`, the global `net` permission will be revoked.
       *
       * Defaults to "inherit".
       */
      env?: "inherit" | boolean | string[];

      /** Specifies if the `hrtime` permission should be requested or revoked.
       * If set to `"inherit"`, the current `hrtime` permission will be inherited.
       * If set to `true`, the global `hrtime` permission will be requested.
       * If set to `false`, the global `hrtime` permission will be revoked.
       *
       * Defaults to "inherit".
       */
      hrtime?: "inherit" | boolean;

      /** Specifies if the `net` permission should be requested or revoked.
       * if set to `"inherit"`, the current `net` permission will be inherited.
       * if set to `true`, the global `net` permission will be requested.
       * if set to `false`, the global `net` permission will be revoked.
       * if set to `string[]`, the `net` permission will be requested with the
       * specified host strings with the format `"<host>[:<port>]`.
       *
       * Defaults to "inherit".
       *
       * Examples:
       *
       * ```ts
       * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
       *
       * Deno.test({
       *   name: "inherit",
       *   permissions: {
       *     net: "inherit",
       *   },
       *   async fn() {
       *     const status = await Deno.permissions.query({ name: "net" })
       *     assertEquals(status.state, "granted");
       *   },
       * });
       * ```
       *
       * ```ts
       * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
       *
       * Deno.test({
       *   name: "true",
       *   permissions: {
       *     net: true,
       *   },
       *   async fn() {
       *     const status = await Deno.permissions.query({ name: "net" });
       *     assertEquals(status.state, "granted");
       *   },
       * });
       * ```
       *
       * ```ts
       * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
       *
       * Deno.test({
       *   name: "false",
       *   permissions: {
       *     net: false,
       *   },
       *   async fn() {
       *     const status = await Deno.permissions.query({ name: "net" });
       *     assertEquals(status.state, "denied");
       *   },
       * });
       * ```
       *
       * ```ts
       * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
       *
       * Deno.test({
       *   name: "localhost:8080",
       *   permissions: {
       *     net: ["localhost:8080"],
       *   },
       *   async fn() {
       *     const status = await Deno.permissions.query({ name: "net", host: "localhost:8080" });
       *     assertEquals(status.state, "granted");
       *   },
       * });
       * ```
       */
      net?: "inherit" | boolean | string[];

      /** Specifies if the `ffi` permission should be requested or revoked.
       * If set to `"inherit"`, the current `ffi` permission will be inherited.
       * If set to `true`, the global `ffi` permission will be requested.
       * If set to `false`, the global `ffi` permission will be revoked.
       *
       * Defaults to "inherit".
       */
      ffi?: "inherit" | boolean;

      /** Specifies if the `read` permission should be requested or revoked.
       * If set to `"inherit"`, the current `read` permission will be inherited.
       * If set to `true`, the global `read` permission will be requested.
       * If set to `false`, the global `read` permission will be revoked.
       * If set to `Array<string | URL>`, the `read` permission will be requested with the
       * specified file paths.
       *
       * Defaults to "inherit".
       */
      read?: "inherit" | boolean | Array<string | URL>;

      /** Specifies if the `run` permission should be requested or revoked.
       * If set to `"inherit"`, the current `run` permission will be inherited.
       * If set to `true`, the global `run` permission will be requested.
       * If set to `false`, the global `run` permission will be revoked.
       *
       * Defaults to "inherit".
       */
      run?: "inherit" | boolean | Array<string | URL>;

      /** Specifies if the `write` permission should be requested or revoked.
       * If set to `"inherit"`, the current `write` permission will be inherited.
       * If set to `true`, the global `write` permission will be requested.
       * If set to `false`, the global `write` permission will be revoked.
       * If set to `Array<string | URL>`, the `write` permission will be requested with the
       * specified file paths.
       *
       * Defaults to "inherit".
       */
      write?: "inherit" | boolean | Array<string | URL>;
    };
  }

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

  /** **UNSTABLE**: New API should be tested first.
   *
   * Acquire an advisory file-system lock for the provided file. `exclusive`
   * defaults to `false`.
   */
  export function flock(rid: number, exclusive?: boolean): Promise<void>;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Acquire an advisory file-system lock for the provided file. `exclusive`
   * defaults to `false`.
   */
  export function flockSync(rid: number, exclusive?: boolean): void;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Release an advisory file-system lock for the provided file.
   */
  export function funlock(rid: number): Promise<void>;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Release an advisory file-system lock for the provided file.
   */
  export function funlockSync(rid: number): void;
}

declare function fetch(
  input: Request | URL | string,
  init?: RequestInit & { client: Deno.HttpClient },
): Promise<Response>;

declare interface WorkerOptions {
  /** UNSTABLE: New API.
   *
   * Set deno.namespace to `true` to make `Deno` namespace and all of its
   * methods available to the worker environment. Defaults to `false`.
   *
   * Configure deno.permissions options to change the level of access the worker will
   * have. By default it will inherit the permissions of its parent thread. The permissions
   * of a worker can't be extended beyond its parent's permissions reach.
   * - "inherit" will take the permissions of the thread the worker is created in
   * - You can disable/enable permissions all together by passing a boolean
   * - You can provide a list of routes relative to the file the worker
   *   is created in to limit the access of the worker (read/write permissions only)
   *
   * Example:
   *
   * ```ts
   * // mod.ts
   * const worker = new Worker(
   *   new URL("deno_worker.ts", import.meta.url).href, {
   *     type: "module",
   *     deno: {
   *       namespace: true,
   *       permissions: {
   *         read: true,
   *       },
   *     },
   *   }
   * );
   * ```
   */
  // TODO(Soremwar)
  // `deno: boolean` is kept for backwards compatibility with the previous
  // worker options implementation. Remove for 2.0.
  deno?: boolean | {
    namespace?: boolean;
    /** Set to `"none"` to disable all the permissions in the worker. */
    permissions?: "inherit" | "none" | {
      env?: "inherit" | boolean | string[];
      hrtime?: "inherit" | boolean;
      /** The format of the net access list must be `hostname[:port]`
       * in order to be resolved.
       *
       * For example: `["https://deno.land", "localhost:8080"]`.
       */
      net?: "inherit" | boolean | string[];
      ffi?: "inherit" | boolean;
      read?: "inherit" | boolean | Array<string | URL>;
      run?: "inherit" | boolean | Array<string | URL>;
      write?: "inherit" | boolean | Array<string | URL>;
    };
  };
}

declare interface WebSocketStreamOptions {
  protocols?: string[];
  signal?: AbortSignal;
}

declare interface WebSocketConnection {
  readable: ReadableStream<string | Uint8Array>;
  writable: WritableStream<string | Uint8Array>;
  extensions: string;
  protocol: string;
}

declare interface WebSocketCloseInfo {
  code?: number;
  reason?: string;
}

declare class WebSocketStream {
  constructor(url: string, options?: WebSocketStreamOptions);
  url: string;
  connection: Promise<WebSocketConnection>;
  closed: Promise<WebSocketCloseInfo>;
  close(closeInfo?: WebSocketCloseInfo): void;
}
