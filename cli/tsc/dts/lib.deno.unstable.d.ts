// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.broadcast_channel" />
/// <reference lib="esnext" />
/// <reference lib="es2022.intl" />

declare namespace Deno {
  export {}; // stop default export type behavior

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   *  Creates a presentable WebGPU surface from given window and
   *  display handles.
   *
   *  The parameters correspond to the table below:
   *
   *  | system            | winHandle     | displayHandle   |
   *  | ----------------- | ------------- | --------------- |
   *  | "cocoa" (macOS)   | `NSView*`     | -               |
   *  | "win32" (Windows) | `HWND`        | `HINSTANCE`     |
   *  | "x11" (Linux)     | Xlib `Window` | Xlib `Display*` |
   *  | "wayland" (Linux) | `wl_surface*` | `wl_display*`   |
   *
   * @category GPU
   * @experimental
   */
  export class UnsafeWindowSurface {
    constructor(
      options: {
        system: "cocoa" | "win32" | "x11" | "wayland";
        windowHandle: Deno.PointerValue<unknown>;
        displayHandle: Deno.PointerValue<unknown>;
        width: number;
        height: number;
      },
    );
    getContext(context: "webgpu"): GPUCanvasContext;
    present(): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Represents membership of a IPv4 multicast group.
   *
   * @category Network
   * @experimental
   */
  export interface MulticastV4Membership {
    /** Leaves the multicast group. */
    leave: () => Promise<void>;
    /** Sets the multicast loopback option. If enabled, multicast packets will be looped back to the local socket. */
    setLoopback: (loopback: boolean) => Promise<void>;
    /** Sets the time-to-live of outgoing multicast packets for this socket. */
    setTTL: (ttl: number) => Promise<void>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Represents membership of a IPv6 multicast group.
   *
   * @category Network
   * @experimental
   */
  export interface MulticastV6Membership {
    /** Leaves the multicast group. */
    leave: () => Promise<void>;
    /** Sets the multicast loopback option. If enabled, multicast packets will be looped back to the local socket. */
    setLoopback: (loopback: boolean) => Promise<void>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A generic transport listener for message-oriented protocols.
   *
   * @category Network
   * @experimental
   */
  export interface DatagramConn
    extends AsyncIterable<[Uint8Array<ArrayBuffer>, Addr]> {
    /** Joins an IPv4 multicast group. */
    joinMulticastV4(
      address: string,
      networkInterface: string,
    ): Promise<MulticastV4Membership>;

    /** Joins an IPv6 multicast group. */
    joinMulticastV6(
      address: string,
      networkInterface: number,
    ): Promise<MulticastV6Membership>;

    /** Waits for and resolves to the next message to the instance.
     *
     * Messages are received in the format of a tuple containing the data array
     * and the address information.
     */
    receive(p?: Uint8Array): Promise<[Uint8Array<ArrayBuffer>, Addr]>;
    /** Sends a message to the target via the connection. The method resolves
     * with the number of bytes sent. */
    send(p: Uint8Array, addr: Addr): Promise<number>;
    /** Close closes the socket. Any pending message promises will be rejected
     * with errors. */
    close(): void;
    /** Return the address of the instance. */
    readonly addr: Addr;
    [Symbol.asyncIterator](): AsyncIterableIterator<
      [Uint8Array<ArrayBuffer>, Addr]
    >;
  }

  /**
   * @category Network
   * @experimental
   */
  export interface TcpListenOptions extends ListenOptions {
    /** When `true` the SO_REUSEPORT flag will be set on the listener. This
     * allows multiple processes to listen on the same address and port.
     *
     * On Linux this will cause the kernel to distribute incoming connections
     * across the different processes that are listening on the same address and
     * port.
     *
     * This flag is only supported on Linux. It is silently ignored on other
     * platforms.
     *
     * @default {false} */
    reusePort?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Unstable options which can be set when opening a datagram listener via
   * {@linkcode Deno.listenDatagram}.
   *
   * @category Network
   * @experimental
   */
  export interface UdpListenOptions extends ListenOptions {
    /** When `true` the specified address will be reused, even if another
     * process has already bound a socket on it. This effectively steals the
     * socket from the listener.
     *
     * @default {false} */
    reuseAddress?: boolean;

    /** When `true`, sent multicast packets will be looped back to the local socket.
     *
     * @default {false} */
    loopback?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
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
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   * @experimental
   */
  export function listenDatagram(
    options: UdpListenOptions & { transport: "udp" },
  ): DatagramConn;

  /** **UNSTABLE**: New API, yet to be vetted.
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
   * Requires `allow-read` and `allow-write` permission.
   *
   * @tags allow-read, allow-write
   * @category Network
   * @experimental
   */
  export function listenDatagram(
    options: UnixListenOptions & { transport: "unixpacket" },
  ): DatagramConn;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Open a new {@linkcode Deno.Kv} connection to persist data.
   *
   * When a path is provided, the database will be persisted to disk at that
   * path. Read and write access to the file is required.
   *
   * When no path is provided, the database will be opened in a default path for
   * the current script. This location is persistent across script runs and is
   * keyed on the origin storage key (the same key that is used to determine
   * `localStorage` persistence). More information about the origin storage key
   * can be found in the Deno Manual.
   *
   * @tags allow-read, allow-write
   * @category Cloud
   * @experimental
   */
  export function openKv(path?: string): Promise<Kv>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * CronScheduleExpression is used as the type of `minute`, `hour`,
   * `dayOfMonth`, `month`, and `dayOfWeek` in {@linkcode CronSchedule}.
   * @category Cloud
   * @experimental
   */
  export type CronScheduleExpression = number | { exact: number | number[] } | {
    start?: number;
    end?: number;
    every?: number;
  };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * CronSchedule is the interface used for JSON format
   * cron `schedule`.
   * @category Cloud
   * @experimental
   */
  export interface CronSchedule {
    minute?: CronScheduleExpression;
    hour?: CronScheduleExpression;
    dayOfMonth?: CronScheduleExpression;
    month?: CronScheduleExpression;
    dayOfWeek?: CronScheduleExpression;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a cron job that will periodically execute the provided handler
   * callback based on the specified schedule.
   *
   * ```ts
   * Deno.cron("sample cron", "20 * * * *", () => {
   *   console.log("cron job executed");
   * });
   * ```
   *
   * ```ts
   * Deno.cron("sample cron", { hour: { every: 6 } }, () => {
   *   console.log("cron job executed");
   * });
   * ```
   *
   * `schedule` can be a string in the Unix cron format or in JSON format
   * as specified by interface {@linkcode CronSchedule}, where time is specified
   * using UTC time zone.
   *
   * @category Cloud
   * @experimental
   */
  export function cron(
    name: string,
    schedule: string | CronSchedule,
    handler: () => Promise<void> | void,
  ): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a cron job that will periodically execute the provided handler
   * callback based on the specified schedule.
   *
   * ```ts
   * Deno.cron("sample cron", "20 * * * *", {
   *   backoffSchedule: [10, 20]
   * }, () => {
   *   console.log("cron job executed");
   * });
   * ```
   *
   * `schedule` can be a string in the Unix cron format or in JSON format
   * as specified by interface {@linkcode CronSchedule}, where time is specified
   * using UTC time zone.
   *
   * `backoffSchedule` option can be used to specify the retry policy for failed
   * executions. Each element in the array represents the number of milliseconds
   * to wait before retrying the execution. For example, `[1000, 5000, 10000]`
   * means that a failed execution will be retried at most 3 times, with 1
   * second, 5 seconds, and 10 seconds delay between each retry. There is a
   * limit of 5 retries and a maximum interval of 1 hour (3600000 milliseconds).
   *
   * @category Cloud
   * @experimental
   */
  export function cron(
    name: string,
    schedule: string | CronSchedule,
    options: { backoffSchedule?: number[]; signal?: AbortSignal },
    handler: () => Promise<void> | void,
  ): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A key to be persisted in a {@linkcode Deno.Kv}. A key is a sequence
   * of {@linkcode Deno.KvKeyPart}s.
   *
   * Keys are ordered lexicographically by their parts. The first part is the
   * most significant, and the last part is the least significant. The order of
   * the parts is determined by both the type and the value of the part. The
   * relative significance of the types can be found in documentation for the
   * {@linkcode Deno.KvKeyPart} type.
   *
   * Keys have a maximum size of 2048 bytes serialized. If the size of the key
   * exceeds this limit, an error will be thrown on the operation that this key
   * was passed to.
   *
   * @category Cloud
   * @experimental
   */
  export type KvKey = readonly KvKeyPart[];

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A single part of a {@linkcode Deno.KvKey}. Parts are ordered
   * lexicographically, first by their type, and within a given type by their
   * value.
   *
   * The ordering of types is as follows:
   *
   * 1. `Uint8Array`
   * 2. `string`
   * 3. `number`
   * 4. `bigint`
   * 5. `boolean`
   *
   * Within a given type, the ordering is as follows:
   *
   * - `Uint8Array` is ordered by the byte ordering of the array
   * - `string` is ordered by the byte ordering of the UTF-8 encoding of the
   *   string
   * - `number` is ordered following this pattern: `-NaN`
   *   < `-Infinity` < `-100.0` < `-1.0` < -`0.5` < `-0.0` < `0.0` < `0.5`
   *   < `1.0` < `100.0` < `Infinity` < `NaN`
   * - `bigint` is ordered by mathematical ordering, with the largest negative
   *   number being the least first value, and the largest positive number
   *   being the last value
   * - `boolean` is ordered by `false` < `true`
   *
   * This means that the part `1.0` (a number) is ordered before the part `2.0`
   * (also a number), but is greater than the part `0n` (a bigint), because
   * `1.0` is a number and `0n` is a bigint, and type ordering has precedence
   * over the ordering of values within a type.
   *
   * @category Cloud
   * @experimental
   */
  export type KvKeyPart =
    | Uint8Array
    | string
    | number
    | bigint
    | boolean
    | symbol;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Consistency level of a KV operation.
   *
   * - `strong` - This operation must be strongly-consistent.
   * - `eventual` - Eventually-consistent behavior is allowed.
   *
   * @category Cloud
   * @experimental
   */
  export type KvConsistencyLevel = "strong" | "eventual";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A selector that selects the range of data returned by a list operation on a
   * {@linkcode Deno.Kv}.
   *
   * The selector can either be a prefix selector or a range selector. A prefix
   * selector selects all keys that start with the given prefix (optionally
   * starting at a given key). A range selector selects all keys that are
   * lexicographically between the given start and end keys.
   *
   * @category Cloud
   * @experimental
   */
  export type KvListSelector =
    | { prefix: KvKey }
    | { prefix: KvKey; start: KvKey }
    | { prefix: KvKey; end: KvKey }
    | { start: KvKey; end: KvKey };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A mutation to a key in a {@linkcode Deno.Kv}. A mutation is a
   * combination of a key, a value, and a type. The type determines how the
   * mutation is applied to the key.
   *
   * - `set` - Sets the value of the key to the given value, overwriting any
   *   existing value. Optionally an `expireIn` option can be specified to
   *   set a time-to-live (TTL) for the key. The TTL is specified in
   *   milliseconds, and the key will be deleted from the database at earliest
   *   after the specified number of milliseconds have elapsed. Once the
   *   specified duration has passed, the key may still be visible for some
   *   additional time. If the `expireIn` option is not specified, the key will
   *   not expire.
   * - `delete` - Deletes the key from the database. The mutation is a no-op if
   *   the key does not exist.
   * - `sum` - Adds the given value to the existing value of the key. Both the
   *   value specified in the mutation, and any existing value must be of type
   *   `Deno.KvU64`. If the key does not exist, the value is set to the given
   *   value (summed with 0). If the result of the sum overflows an unsigned
   *   64-bit integer, the result is wrapped around.
   * - `max` - Sets the value of the key to the maximum of the existing value
   *   and the given value. Both the value specified in the mutation, and any
   *   existing value must be of type `Deno.KvU64`. If the key does not exist,
   *   the value is set to the given value.
   * - `min` - Sets the value of the key to the minimum of the existing value
   *   and the given value. Both the value specified in the mutation, and any
   *   existing value must be of type `Deno.KvU64`. If the key does not exist,
   *   the value is set to the given value.
   *
   * @category Cloud
   * @experimental
   */
  export type KvMutation =
    & { key: KvKey }
    & (
      | { type: "set"; value: unknown; expireIn?: number }
      | { type: "delete" }
      | { type: "sum"; value: KvU64 }
      | { type: "max"; value: KvU64 }
      | { type: "min"; value: KvU64 }
    );

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An iterator over a range of data entries in a {@linkcode Deno.Kv}.
   *
   * The cursor getter returns the cursor that can be used to resume the
   * iteration from the current position in the future.
   *
   * @category Cloud
   * @experimental
   */
  export class KvListIterator<T> implements AsyncIterableIterator<KvEntry<T>> {
    /**
     * Returns the cursor of the current position in the iteration. This cursor
     * can be used to resume the iteration from the current position in the
     * future by passing it to the `cursor` option of the `list` method.
     */
    get cursor(): string;

    next(): Promise<IteratorResult<KvEntry<T>, undefined>>;
    [Symbol.asyncIterator](): AsyncIterableIterator<KvEntry<T>>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A versioned pair of key and value in a {@linkcode Deno.Kv}.
   *
   * The `versionstamp` is a string that represents the current version of the
   * key-value pair. It can be used to perform atomic operations on the KV store
   * by passing it to the `check` method of a {@linkcode Deno.AtomicOperation}.
   *
   * @category Cloud
   * @experimental
   */
  export interface KvEntry<T> {
    key: KvKey;
    value: T;
    versionstamp: string;
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * An optional versioned pair of key and value in a {@linkcode Deno.Kv}.
   *
   * This is the same as a {@linkcode KvEntry}, but the `value` and `versionstamp`
   * fields may be `null` if no value exists for the given key in the KV store.
   *
   * @category Cloud
   * @experimental
   */
  export type KvEntryMaybe<T> = KvEntry<T> | {
    key: KvKey;
    value: null;
    versionstamp: null;
  };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Options for listing key-value pairs in a {@linkcode Deno.Kv}.
   *
   * @category Cloud
   * @experimental
   */
  export interface KvListOptions {
    /**
     * The maximum number of key-value pairs to return. If not specified, all
     * matching key-value pairs will be returned.
     */
    limit?: number;
    /**
     * The cursor to resume the iteration from. If not specified, the iteration
     * will start from the beginning.
     */
    cursor?: string;
    /**
     * Whether to reverse the order of the returned key-value pairs. If not
     * specified, the order will be ascending from the start of the range as per
     * the lexicographical ordering of the keys. If `true`, the order will be
     * descending from the end of the range.
     *
     * The default value is `false`.
     */
    reverse?: boolean;
    /**
     * The consistency level of the list operation. The default consistency
     * level is "strong". Some use cases can benefit from using a weaker
     * consistency level. For more information on consistency levels, see the
     * documentation for {@linkcode Deno.KvConsistencyLevel}.
     *
     * List operations are performed in batches (in sizes specified by the
     * `batchSize` option). The consistency level of the list operation is
     * applied to each batch individually. This means that while each batch is
     * guaranteed to be consistent within itself, the entire list operation may
     * not be consistent across batches because a mutation may be applied to a
     * key-value pair between batches, in a batch that has already been returned
     * by the list operation.
     */
    consistency?: KvConsistencyLevel;
    /**
     * The size of the batches in which the list operation is performed. Larger
     * or smaller batch sizes may positively or negatively affect the
     * performance of a list operation depending on the specific use case and
     * iteration behavior. Slow iterating queries may benefit from using a
     * smaller batch size for increased overall consistency, while fast
     * iterating queries may benefit from using a larger batch size for better
     * performance.
     *
     * The default batch size is equal to the `limit` option, or 100 if this is
     * unset. The maximum value for this option is 500. Larger values will be
     * clamped.
     */
    batchSize?: number;
  }

  /**
   * @category Cloud
   * @experimental
   */
  export interface KvCommitResult {
    ok: true;
    /** The versionstamp of the value committed to KV. */
    versionstamp: string;
  }

  /**
   * @category Cloud
   * @experimental
   */
  export interface KvCommitError {
    ok: false;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A check to perform as part of a {@linkcode Deno.AtomicOperation}. The check
   * will fail if the versionstamp for the key-value pair in the KV store does
   * not match the given versionstamp. A check with a `null` versionstamp checks
   * that the key-value pair does not currently exist in the KV store.
   *
   * @category Cloud
   * @experimental
   */
  export interface AtomicCheck {
    key: KvKey;
    versionstamp: string | null;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An operation on a {@linkcode Deno.Kv} that can be performed
   * atomically. Atomic operations do not auto-commit, and must be committed
   * explicitly by calling the `commit` method.
   *
   * Atomic operations can be used to perform multiple mutations on the KV store
   * in a single atomic transaction. They can also be used to perform
   * conditional mutations by specifying one or more
   * {@linkcode Deno.AtomicCheck}s that ensure that a mutation is only performed
   * if the key-value pair in the KV has a specific versionstamp. If any of the
   * checks fail, the entire operation will fail and no mutations will be made.
   *
   * The ordering of mutations is guaranteed to be the same as the ordering of
   * the mutations specified in the operation. Checks are performed before any
   * mutations are performed. The ordering of checks is unobservable.
   *
   * Atomic operations can be used to implement optimistic locking, where a
   * mutation is only performed if the key-value pair in the KV store has not
   * been modified since the last read. This can be done by specifying a check
   * that ensures that the versionstamp of the key-value pair matches the
   * versionstamp that was read. If the check fails, the mutation will not be
   * performed and the operation will fail. One can then retry the read-modify-
   * write operation in a loop until it succeeds.
   *
   * The `commit` method of an atomic operation returns a value indicating
   * whether checks passed and mutations were performed. If the operation failed
   * because of a failed check, the return value will be a
   * {@linkcode Deno.KvCommitError} with an `ok: false` property. If the
   * operation failed for any other reason (storage error, invalid value, etc.),
   * an exception will be thrown. If the operation succeeded, the return value
   * will be a {@linkcode Deno.KvCommitResult} object with a `ok: true` property
   * and the versionstamp of the value committed to KV.
   *
   * @category Cloud
   * @experimental
   */
  export class AtomicOperation {
    /**
     * Add to the operation a check that ensures that the versionstamp of the
     * key-value pair in the KV store matches the given versionstamp. If the
     * check fails, the entire operation will fail and no mutations will be
     * performed during the commit.
     */
    check(...checks: AtomicCheck[]): this;
    /**
     * Add to the operation a mutation that performs the specified mutation on
     * the specified key if all checks pass during the commit. The types and
     * semantics of all available mutations are described in the documentation
     * for {@linkcode Deno.KvMutation}.
     */
    mutate(...mutations: KvMutation[]): this;
    /**
     * Shortcut for creating a `sum` mutation. This method wraps `n` in a
     * {@linkcode Deno.KvU64}, so the value of `n` must be in the range
     * `[0, 2^64-1]`.
     */
    sum(key: KvKey, n: bigint): this;
    /**
     * Shortcut for creating a `min` mutation. This method wraps `n` in a
     * {@linkcode Deno.KvU64}, so the value of `n` must be in the range
     * `[0, 2^64-1]`.
     */
    min(key: KvKey, n: bigint): this;
    /**
     * Shortcut for creating a `max` mutation. This method wraps `n` in a
     * {@linkcode Deno.KvU64}, so the value of `n` must be in the range
     * `[0, 2^64-1]`.
     */
    max(key: KvKey, n: bigint): this;
    /**
     * Add to the operation a mutation that sets the value of the specified key
     * to the specified value if all checks pass during the commit.
     *
     * Optionally an `expireIn` option can be specified to set a time-to-live
     * (TTL) for the key. The TTL is specified in milliseconds, and the key will
     * be deleted from the database at earliest after the specified number of
     * milliseconds have elapsed. Once the specified duration has passed, the
     * key may still be visible for some additional time. If the `expireIn`
     * option is not specified, the key will not expire.
     */
    set(key: KvKey, value: unknown, options?: { expireIn?: number }): this;
    /**
     * Add to the operation a mutation that deletes the specified key if all
     * checks pass during the commit.
     */
    delete(key: KvKey): this;
    /**
     * Add to the operation a mutation that enqueues a value into the queue
     * if all checks pass during the commit.
     */
    enqueue(
      value: unknown,
      options?: {
        delay?: number;
        keysIfUndelivered?: KvKey[];
        backoffSchedule?: number[];
      },
    ): this;
    /**
     * Commit the operation to the KV store. Returns a value indicating whether
     * checks passed and mutations were performed. If the operation failed
     * because of a failed check, the return value will be a {@linkcode
     * Deno.KvCommitError} with an `ok: false` property. If the operation failed
     * for any other reason (storage error, invalid value, etc.), an exception
     * will be thrown. If the operation succeeded, the return value will be a
     * {@linkcode Deno.KvCommitResult} object with a `ok: true` property and the
     * versionstamp of the value committed to KV.
     *
     * If the commit returns `ok: false`, one may create a new atomic operation
     * with updated checks and mutations and attempt to commit it again. See the
     * note on optimistic locking in the documentation for
     * {@linkcode Deno.AtomicOperation}.
     */
    commit(): Promise<KvCommitResult | KvCommitError>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A key-value database that can be used to store and retrieve data.
   *
   * Data is stored as key-value pairs, where the key is a {@linkcode Deno.KvKey}
   * and the value is an arbitrary structured-serializable JavaScript value.
   * Keys are ordered lexicographically as described in the documentation for
   * {@linkcode Deno.KvKey}. Keys are unique within a database, and the last
   * value set for a given key is the one that is returned when reading the
   * key. Keys can be deleted from the database, in which case they will no
   * longer be returned when reading keys.
   *
   * Values can be any structured-serializable JavaScript value (objects,
   * arrays, strings, numbers, etc.). The special value {@linkcode Deno.KvU64}
   * can be used to store 64-bit unsigned integers in the database. This special
   * value can not be nested within other objects or arrays. In addition to the
   * regular database mutation operations, the unsigned 64-bit integer value
   * also supports `sum`, `max`, and `min` mutations.
   *
   * Keys are versioned on write by assigning the key an ever-increasing
   * "versionstamp". The versionstamp represents the version of a key-value pair
   * in the database at some point in time, and can be used to perform
   * transactional operations on the database without requiring any locking.
   * This is enabled by atomic operations, which can have conditions that ensure
   * that the operation only succeeds if the versionstamp of the key-value pair
   * matches an expected versionstamp.
   *
   * Keys have a maximum length of 2048 bytes after serialization. Values have a
   * maximum length of 64 KiB after serialization. Serialization of both keys
   * and values is somewhat opaque, but one can usually assume that the
   * serialization of any value is about the same length as the resulting string
   * of a JSON serialization of that same value. If theses limits are exceeded,
   * an exception will be thrown.
   *
   * @category Cloud
   * @experimental
   */
  export class Kv implements Disposable {
    /**
     * Retrieve the value and versionstamp for the given key from the database
     * in the form of a {@linkcode Deno.KvEntryMaybe}. If no value exists for
     * the key, the returned entry will have a `null` value and versionstamp.
     *
     * ```ts
     * const db = await Deno.openKv();
     * const result = await db.get(["foo"]);
     * result.key; // ["foo"]
     * result.value; // "bar"
     * result.versionstamp; // "00000000000000010000"
     * ```
     *
     * The `consistency` option can be used to specify the consistency level
     * for the read operation. The default consistency level is "strong". Some
     * use cases can benefit from using a weaker consistency level. For more
     * information on consistency levels, see the documentation for
     * {@linkcode Deno.KvConsistencyLevel}.
     */
    get<T = unknown>(
      key: KvKey,
      options?: { consistency?: KvConsistencyLevel },
    ): Promise<KvEntryMaybe<T>>;

    /**
     * Retrieve multiple values and versionstamps from the database in the form
     * of an array of {@linkcode Deno.KvEntryMaybe} objects. The returned array
     * will have the same length as the `keys` array, and the entries will be in
     * the same order as the keys. If no value exists for a given key, the
     * returned entry will have a `null` value and versionstamp.
     *
     * ```ts
     * const db = await Deno.openKv();
     * const result = await db.getMany([["foo"], ["baz"]]);
     * result[0].key; // ["foo"]
     * result[0].value; // "bar"
     * result[0].versionstamp; // "00000000000000010000"
     * result[1].key; // ["baz"]
     * result[1].value; // null
     * result[1].versionstamp; // null
     * ```
     *
     * The `consistency` option can be used to specify the consistency level
     * for the read operation. The default consistency level is "strong". Some
     * use cases can benefit from using a weaker consistency level. For more
     * information on consistency levels, see the documentation for
     * {@linkcode Deno.KvConsistencyLevel}.
     */
    getMany<T extends readonly unknown[]>(
      keys: readonly [...{ [K in keyof T]: KvKey }],
      options?: { consistency?: KvConsistencyLevel },
    ): Promise<{ [K in keyof T]: KvEntryMaybe<T[K]> }>;
    /**
     * Set the value for the given key in the database. If a value already
     * exists for the key, it will be overwritten.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.set(["foo"], "bar");
     * ```
     *
     * Optionally an `expireIn` option can be specified to set a time-to-live
     * (TTL) for the key. The TTL is specified in milliseconds, and the key will
     * be deleted from the database at earliest after the specified number of
     * milliseconds have elapsed. Once the specified duration has passed, the
     * key may still be visible for some additional time. If the `expireIn`
     * option is not specified, the key will not expire.
     */
    set(
      key: KvKey,
      value: unknown,
      options?: { expireIn?: number },
    ): Promise<KvCommitResult>;

    /**
     * Delete the value for the given key from the database. If no value exists
     * for the key, this operation is a no-op.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.delete(["foo"]);
     * ```
     */
    delete(key: KvKey): Promise<void>;

    /**
     * Retrieve a list of keys in the database. The returned list is an
     * {@linkcode Deno.KvListIterator} which can be used to iterate over the
     * entries in the database.
     *
     * Each list operation must specify a selector which is used to specify the
     * range of keys to return. The selector can either be a prefix selector, or
     * a range selector:
     *
     * - A prefix selector selects all keys that start with the given prefix of
     *   key parts. For example, the selector `["users"]` will select all keys
     *   that start with the prefix `["users"]`, such as `["users", "alice"]`
     *   and `["users", "bob"]`. Note that you can not partially match a key
     *   part, so the selector `["users", "a"]` will not match the key
     *   `["users", "alice"]`. A prefix selector may specify a `start` key that
     *   is used to skip over keys that are lexicographically less than the
     *   start key.
     * - A range selector selects all keys that are lexicographically between
     *   the given start and end keys (including the start, and excluding the
     *   end). For example, the selector `["users", "a"], ["users", "n"]` will
     *   select all keys that start with the prefix `["users"]` and have a
     *   second key part that is lexicographically between `a` and `n`, such as
     *   `["users", "alice"]`, `["users", "bob"]`, and `["users", "mike"]`, but
     *   not `["users", "noa"]` or `["users", "zoe"]`.
     *
     * ```ts
     * const db = await Deno.openKv();
     * const entries = db.list({ prefix: ["users"] });
     * for await (const entry of entries) {
     *   entry.key; // ["users", "alice"]
     *   entry.value; // { name: "Alice" }
     *   entry.versionstamp; // "00000000000000010000"
     * }
     * ```
     *
     * The `options` argument can be used to specify additional options for the
     * list operation. See the documentation for {@linkcode Deno.KvListOptions}
     * for more information.
     */
    list<T = unknown>(
      selector: KvListSelector,
      options?: KvListOptions,
    ): KvListIterator<T>;

    /**
     * Add a value into the database queue to be delivered to the queue
     * listener via {@linkcode Deno.Kv.listenQueue}.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.enqueue("bar");
     * ```
     *
     * The `delay` option can be used to specify the delay (in milliseconds)
     * of the value delivery. The default delay is 0, which means immediate
     * delivery.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.enqueue("bar", { delay: 60000 });
     * ```
     *
     * The `keysIfUndelivered` option can be used to specify the keys to
     * be set if the value is not successfully delivered to the queue
     * listener after several attempts. The values are set to the value of
     * the queued message.
     *
     * The `backoffSchedule` option can be used to specify the retry policy for
     * failed message delivery. Each element in the array represents the number of
     * milliseconds to wait before retrying the delivery. For example,
     * `[1000, 5000, 10000]` means that a failed delivery will be retried
     * at most 3 times, with 1 second, 5 seconds, and 10 seconds delay
     * between each retry.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.enqueue("bar", {
     *   keysIfUndelivered: [["foo", "bar"]],
     *   backoffSchedule: [1000, 5000, 10000],
     * });
     * ```
     */
    enqueue(
      value: unknown,
      options?: {
        delay?: number;
        keysIfUndelivered?: KvKey[];
        backoffSchedule?: number[];
      },
    ): Promise<KvCommitResult>;

    /**
     * Listen for queue values to be delivered from the database queue, which
     * were enqueued with {@linkcode Deno.Kv.enqueue}. The provided handler
     * callback is invoked on every dequeued value. A failed callback
     * invocation is automatically retried multiple times until it succeeds
     * or until the maximum number of retries is reached.
     *
     * ```ts
     * const db = await Deno.openKv();
     * db.listenQueue(async (msg: unknown) => {
     *   await db.set(["foo"], msg);
     * });
     * ```
     */
    // deno-lint-ignore no-explicit-any
    listenQueue(handler: (value: any) => Promise<void> | void): Promise<void>;

    /**
     * Create a new {@linkcode Deno.AtomicOperation} object which can be used to
     * perform an atomic transaction on the database. This does not perform any
     * operations on the database - the atomic transaction must be committed
     * explicitly using the {@linkcode Deno.AtomicOperation.commit} method once
     * all checks and mutations have been added to the operation.
     */
    atomic(): AtomicOperation;

    /**
     * Watch for changes to the given keys in the database. The returned stream
     * is a {@linkcode ReadableStream} that emits a new value whenever any of
     * the watched keys change their versionstamp. The emitted value is an array
     * of {@linkcode Deno.KvEntryMaybe} objects, with the same length and order
     * as the `keys` array. If no value exists for a given key, the returned
     * entry will have a `null` value and versionstamp.
     *
     * The returned stream does not return every single intermediate state of
     * the watched keys, but rather only keeps you up to date with the latest
     * state of the keys. This means that if a key is modified multiple times
     * quickly, you may not receive a notification for every single change, but
     * rather only the latest state of the key.
     *
     * ```ts
     * const db = await Deno.openKv();
     *
     * const stream = db.watch([["foo"], ["bar"]]);
     * for await (const entries of stream) {
     *   entries[0].key; // ["foo"]
     *   entries[0].value; // "bar"
     *   entries[0].versionstamp; // "00000000000000010000"
     *   entries[1].key; // ["bar"]
     *   entries[1].value; // null
     *   entries[1].versionstamp; // null
     * }
     * ```
     *
     * The `options` argument can be used to specify additional options for the
     * watch operation. The `raw` option can be used to specify whether a new
     * value should be emitted whenever a mutation occurs on any of the watched
     * keys (even if the value of the key does not change, such as deleting a
     * deleted key), or only when entries have observably changed in some way.
     * When `raw: true` is used, it is possible for the stream to occasionally
     * emit values even if no mutations have occurred on any of the watched
     * keys. The default value for this option is `false`.
     */
    watch<T extends readonly unknown[]>(
      keys: readonly [...{ [K in keyof T]: KvKey }],
      options?: { raw?: boolean },
    ): ReadableStream<{ [K in keyof T]: KvEntryMaybe<T[K]> }>;

    /**
     * Close the database connection. This will prevent any further operations
     * from being performed on the database, and interrupt any in-flight
     * operations immediately.
     */
    close(): void;

    /**
     * Get a symbol that represents the versionstamp of the current atomic
     * operation. This symbol can be used as the last part of a key in
     * `.set()`, both directly on the `Kv` object and on an `AtomicOperation`
     * object created from this `Kv` instance.
     */
    commitVersionstamp(): symbol;

    [Symbol.dispose](): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Wrapper type for 64-bit unsigned integers for use as values in a
   * {@linkcode Deno.Kv}.
   *
   * @category Cloud
   * @experimental
   */
  export class KvU64 {
    /** Create a new `KvU64` instance from the given bigint value. If the value
     * is signed or greater than 64-bits, an error will be thrown. */
    constructor(value: bigint);
    /** The value of this unsigned 64-bit integer, represented as a bigint. */
    readonly value: bigint;
  }

  /**
   * A namespace containing runtime APIs available in Jupyter notebooks.
   *
   * When accessed outside of Jupyter notebook context an error will be thrown.
   *
   * @category Jupyter
   * @experimental
   */
  export namespace jupyter {
    /**
     * @category Jupyter
     * @experimental
     */
    export interface DisplayOptions {
      raw?: boolean;
      update?: boolean;
      display_id?: string;
    }

    /**
     * @category Jupyter
     * @experimental
     */
    export interface VegaObject {
      $schema: string;
      [key: string]: unknown;
    }

    /**
     * A collection of supported media types and data for Jupyter frontends.
     *
     * @category Jupyter
     * @experimental
     */
    export interface MediaBundle {
      "text/plain"?: string;
      "text/html"?: string;
      "image/svg+xml"?: string;
      "text/markdown"?: string;
      "application/javascript"?: string;

      // Images (per Jupyter spec) must be base64 encoded. We could _allow_
      // accepting Uint8Array or ArrayBuffer within `display` calls, however we still
      // must encode them for jupyter.
      "image/png"?: string; // WISH: Uint8Array | ArrayBuffer
      "image/jpeg"?: string; // WISH: Uint8Array | ArrayBuffer
      "image/gif"?: string; // WISH: Uint8Array | ArrayBuffer
      "application/pdf"?: string; // WISH: Uint8Array | ArrayBuffer

      // NOTE: all JSON types must be objects at the top level (no arrays, strings, or other primitives)
      "application/json"?: object;
      "application/geo+json"?: object;
      "application/vdom.v1+json"?: object;
      "application/vnd.plotly.v1+json"?: object;
      "application/vnd.vega.v5+json"?: VegaObject;
      "application/vnd.vegalite.v4+json"?: VegaObject;
      "application/vnd.vegalite.v5+json"?: VegaObject;

      // Must support a catch all for custom media types / mimetypes
      [key: string]: string | object | undefined;
    }

    /**
     * @category Jupyter
     * @experimental
     */
    export const $display: unique symbol;

    /**
     * @category Jupyter
     * @experimental
     */
    export interface Displayable {
      [$display]: () => MediaBundle | Promise<MediaBundle>;
    }

    /**
     * Display function for Jupyter Deno Kernel.
     * Mimics the behavior of IPython's `display(obj, raw=True)` function to allow
     * asynchronous displaying of objects in Jupyter.
     *
     * @param obj - The object to be displayed
     * @param options - Display options with a default { raw: true }
     * @category Jupyter
     * @experimental
     */
    export function display(
      obj: unknown,
      options?: DisplayOptions,
    ): Promise<void>;

    /**
     * Show Markdown in Jupyter frontends with a tagged template function.
     *
     * Takes a template string and returns a displayable object for Jupyter frontends.
     *
     * @example
     * Create a Markdown view.
     *
     * ```typescript
     * const { md } = Deno.jupyter;
     * md`# Notebooks in TypeScript via Deno ![Deno logo](https://github.com/denoland.png?size=32)
     *
     * * TypeScript ${Deno.version.typescript}
     * * V8 ${Deno.version.v8}
     * * Deno ${Deno.version.deno}
     *
     * Interactive compute with Jupyter _built into Deno_!
     * `
     * ```
     *
     * @category Jupyter
     * @experimental
     */
    export function md(
      strings: TemplateStringsArray,
      ...values: unknown[]
    ): Displayable;

    /**
     * Show HTML in Jupyter frontends with a tagged template function.
     *
     * Takes a template string and returns a displayable object for Jupyter frontends.
     *
     * @example
     * Create an HTML view.
     * ```typescript
     * const { html } = Deno.jupyter;
     * html`<h1>Hello, world!</h1>`
     * ```
     *
     * @category Jupyter
     * @experimental
     */
    export function html(
      strings: TemplateStringsArray,
      ...values: unknown[]
    ): Displayable;

    /**
     * SVG Tagged Template Function.
     *
     * Takes a template string and returns a displayable object for Jupyter frontends.
     *
     * Example usage:
     *
     * svg`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
     *      <circle cx="50" cy="50" r="40" stroke="green" stroke-width="4" fill="yellow" />
     *    </svg>`
     *
     * @category Jupyter
     * @experimental
     */
    export function svg(
      strings: TemplateStringsArray,
      ...values: unknown[]
    ): Displayable;

    /**
     * Display a JPG or PNG image.
     *
     * ```
     * Deno.jupyter.image("./cat.jpg");
     * Deno.jupyter.image("./dog.png");
     * ```
     *
     * @category Jupyter
     * @experimental
     */
    export function image(path: string): Displayable;

    /**
     * Display a JPG or PNG image.
     *
     * ```
     * const img = Deno.readFileSync("./cat.jpg");
     * Deno.jupyter.image(img);
     * ```
     *
     * @category Jupyter
     * @experimental
     */
    export function image(data: Uint8Array): Displayable;

    /**
     * Format an object for displaying in Deno
     *
     * @param obj - The object to be displayed
     * @returns Promise<MediaBundle>
     *
     * @category Jupyter
     * @experimental
     */
    export function format(obj: unknown): Promise<MediaBundle>;

    /**
     * Broadcast a message on IO pub channel.
     *
     * ```
     * await Deno.jupyter.broadcast("display_data", {
     *   data: { "text/html": "<b>Processing.</b>" },
     *   metadata: {},
     *   transient: { display_id: "progress" }
     * });
     *
     * await new Promise((resolve) => setTimeout(resolve, 500));
     *
     * await Deno.jupyter.broadcast("update_display_data", {
     *   data: { "text/html": "<b>Processing..</b>" },
     *   metadata: {},
     *   transient: { display_id: "progress" }
     * });
     * ```
     *
     * @category Jupyter
     * @experimental
     */
    export function broadcast(
      msgType: string,
      content: Record<string, unknown>,
      extra?: {
        metadata?: Record<string, unknown>;
        buffers?: Uint8Array[];
      },
    ): Promise<void>;

    export {}; // only export exports
  }

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * APIs for working with the OpenTelemetry observability framework. Deno can
   * export traces, metrics, and logs to OpenTelemetry compatible backends via
   * the OTLP protocol.
   *
   * Deno automatically instruments the runtime with OpenTelemetry traces and
   * metrics. This data is exported via OTLP to OpenTelemetry compatible
   * backends. User logs from the `console` API are exported as OpenTelemetry
   * logs via OTLP.
   *
   * User code can also create custom traces, metrics, and logs using the
   * OpenTelemetry API. This is done using the official OpenTelemetry package
   * for JavaScript:
   * [`npm:@opentelemetry/api`](https://opentelemetry.io/docs/languages/js/).
   * Deno integrates with this package to provide tracing, metrics, and trace
   * context propagation between native Deno APIs (like `Deno.serve` or `fetch`)
   * and custom user code. Deno automatically registers the providers with the
   * OpenTelemetry API, so users can start creating custom traces, metrics, and
   * logs without any additional setup.
   *
   * @example Using OpenTelemetry API to create custom traces
   * ```ts,ignore
   * import { trace } from "npm:@opentelemetry/api@1";
   *
   * const tracer = trace.getTracer("example-tracer");
   *
   * async function doWork() {
   *   return tracer.startActiveSpan("doWork", async (span) => {
   *     span.setAttribute("key", "value");
   *     await new Promise((resolve) => setTimeout(resolve, 1000));
   *     span.end();
   *   });
   * }
   *
   * Deno.serve(async (req) => {
   *   await doWork();
   *   const resp = await fetch("https://example.com");
   *   return resp;
   * });
   * ```
   *
   * @category Telemetry
   * @experimental
   */
  export namespace telemetry {
    /**
     * A TracerProvider compatible with OpenTelemetry.js
     * https://open-telemetry.github.io/opentelemetry-js/interfaces/_opentelemetry_api.TracerProvider.html
     *
     * This is a singleton object that implements the OpenTelemetry
     * TracerProvider interface.
     *
     * @category Telemetry
     * @experimental
     */
    // deno-lint-ignore no-explicit-any
    export const tracerProvider: any;

    /**
     * A ContextManager compatible with OpenTelemetry.js
     * https://open-telemetry.github.io/opentelemetry-js/interfaces/_opentelemetry_api.ContextManager.html
     *
     * This is a singleton object that implements the OpenTelemetry
     * ContextManager interface.
     *
     * @category Telemetry
     * @experimental
     */
    // deno-lint-ignore no-explicit-any
    export const contextManager: any;

    /**
     * A MeterProvider compatible with OpenTelemetry.js
     * https://open-telemetry.github.io/opentelemetry-js/interfaces/_opentelemetry_api.MeterProvider.html
     *
     * This is a singleton object that implements the OpenTelemetry
     * MeterProvider interface.
     *
     * @category Telemetry
     * @experimental
     */
    // deno-lint-ignore no-explicit-any
    export const meterProvider: any;

    export {}; // only export exports
  }

  /**
   * @category Linter
   * @experimental
   */
  export namespace lint {
    /**
     * @category Linter
     * @experimental
     */
    export type Range = [number, number];

    /**
     * @category Linter
     * @experimental
     */
    export interface Fix {
      range: Range;
      text?: string;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface Fixer {
      insertTextAfter(node: Node, text: string): Fix;
      insertTextAfterRange(range: Range, text: string): Fix;
      insertTextBefore(node: Node, text: string): Fix;
      insertTextBeforeRange(range: Range, text: string): Fix;
      remove(node: Node): Fix;
      removeRange(range: Range): Fix;
      replaceText(node: Node, text: string): Fix;
      replaceTextRange(range: Range, text: string): Fix;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ReportData {
      node?: Node;
      range?: Range;
      message: string;
      hint?: string;
      fix?(fixer: Fixer): Fix | Iterable<Fix>;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface SourceCode {
      /**
       * Get the source test of a node. Omit `node` to get the
       * full source code.
       */
      getText(node?: Node): string;
      /**
       * Returns array of ancestors of the current node, excluding the
       * current node.
       */
      getAncestors(node: Node): Node[];
      /**
       * Get the full source code.
       */
      text: string;
      /**
       * Get the root node of the file. It's always the `Program` node.
       */
      ast: Program;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface RuleContext {
      /**
       * The running rule id: `<plugin-name>/<rule-name>`
       */
      id: string;
      /**
       * Name of the file that's currently being linted.
       */
      filename: string;
      /**
       * Helper methods for working with the raw source code.
       */
      sourceCode: SourceCode;
      /**
       * Report a lint error.
       */
      report(data: ReportData): void;
      /**
       * @deprecated Use `ctx.filename` instead.
       */
      getFilename(): string;
      /**
       * @deprecated Use `ctx.sourceCode` instead.
       */
      getSourceCode(): SourceCode;
    }

    /**
     * @category Linter
     * @experimental
     */
    export type LintVisitor =
      & {
        [P in Node["type"]]?: (node: Extract<Node, { type: P }>) => void;
      }
      & {
        [P in Node["type"] as `${P}:exit`]?: (
          node: Extract<Node, { type: P }>,
        ) => void;
      }
      & // Custom selectors which cannot be typed by us
      // deno-lint-ignore no-explicit-any
      Partial<{ [key: string]: (node: any) => void }>;

    /**
     * @category Linter
     * @experimental
     */
    export interface Rule {
      create(ctx: RuleContext): LintVisitor;
      destroy?(ctx: RuleContext): void;
    }

    /**
     * In your plugins file do something like
     *
     * ```ts
     * export default {
     *   name: "my-plugin",
     *   rules: {
     *     "no-foo": {
     *        create(ctx) {
     *          return {
     *             VariableDeclaration(node) {}
     *          }
     *        }
     *     }
     *   }
     * } satisfies Deno.lint.Plugin
     * ```
     * @category Linter
     * @experimental
     */
    export interface Plugin {
      name: string;
      rules: Record<string, Rule>;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface Diagnostic {
      id: string;
      message: string;
      hint?: string;
      range: Range;
      fix?: Fix[];
    }

    /**
     * This API is useful for testing lint plugins.
     *
     * It throws an error if it's not used in `deno test` subcommand.
     * @category Linter
     * @experimental
     */
    export function runPlugin(
      plugin: Plugin,
      fileName: string,
      source: string,
    ): Diagnostic[];

    /**
     * @category Linter
     * @experimental
     */
    export interface Program {
      type: "Program";
      range: Range;
      sourceType: "module" | "script";
      body: Statement[];
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ImportSpecifier {
      type: "ImportSpecifier";
      range: Range;
      imported: Identifier | StringLiteral;
      local: Identifier;
      importKind: "type" | "value";
      parent: ExportAllDeclaration | ExportNamedDeclaration | ImportDeclaration;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ImportDefaultSpecifier {
      type: "ImportDefaultSpecifier";
      range: Range;
      local: Identifier;
      parent: ImportDeclaration;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ImportNamespaceSpecifier {
      type: "ImportNamespaceSpecifier";
      range: Range;
      local: Identifier;
      parent: ImportDeclaration;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ImportAttribute {
      type: "ImportAttribute";
      range: Range;
      key: Identifier | Literal;
      value: Literal;
      parent:
        | ExportAllDeclaration
        | ExportNamedDeclaration
        | ImportDeclaration
        | TSImportType;
    }

    /**
     * An import declaration, examples:
     * @category Linter
     * @experimental
     */
    export interface ImportDeclaration {
      type: "ImportDeclaration";
      range: Range;
      importKind: "type" | "value";
      source: StringLiteral;
      specifiers: Array<
        | ImportDefaultSpecifier
        | ImportNamespaceSpecifier
        | ImportSpecifier
      >;
      attributes: ImportAttribute[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ExportDefaultDeclaration {
      type: "ExportDefaultDeclaration";
      range: Range;
      declaration:
        | ClassDeclaration
        | Expression
        | FunctionDeclaration
        | TSDeclareFunction
        | TSEnumDeclaration
        | TSInterfaceDeclaration
        | TSModuleDeclaration
        | TSTypeAliasDeclaration
        | VariableDeclaration;
      exportKind: "type" | "value";
      parent: BlockStatement | Program | TSModuleBlock;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ExportNamedDeclaration {
      type: "ExportNamedDeclaration";
      range: Range;
      exportKind: "type" | "value";
      specifiers: ExportSpecifier[];
      declaration:
        | ClassDeclaration
        | FunctionDeclaration
        | TSDeclareFunction
        | TSEnumDeclaration
        | TSImportEqualsDeclaration
        | TSInterfaceDeclaration
        | TSModuleDeclaration
        | TSTypeAliasDeclaration
        | VariableDeclaration
        | null;
      source: StringLiteral | null;
      attributes: ImportAttribute[];
      parent: BlockStatement | Program | TSModuleBlock;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ExportAllDeclaration {
      type: "ExportAllDeclaration";
      range: Range;
      exportKind: "type" | "value";
      exported: Identifier | null;
      source: StringLiteral;
      attributes: ImportAttribute[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSNamespaceExportDeclaration {
      type: "TSNamespaceExportDeclaration";
      range: Range;
      id: Identifier;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSImportEqualsDeclaration {
      type: "TSImportEqualsDeclaration";
      range: Range;
      importKind: "type" | "value";
      id: Identifier;
      moduleReference: Identifier | TSExternalModuleReference | TSQualifiedName;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSExternalModuleReference {
      type: "TSExternalModuleReference";
      range: Range;
      expression: StringLiteral;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface ExportSpecifier {
      type: "ExportSpecifier";
      range: Range;
      exportKind: "type" | "value";
      exported: Identifier | StringLiteral;
      local: Identifier | StringLiteral;
      parent: ExportNamedDeclaration;
    }

    /**
     * Variable declaration.
     * @category Linter
     * @experimental
     */
    export interface VariableDeclaration {
      type: "VariableDeclaration";
      range: Range;
      declare: boolean;
      kind: "let" | "var" | "const" | "await using" | "using";
      declarations: VariableDeclarator[];
      parent: Node;
    }

    /**
     * A VariableDeclaration can declare multiple variables. This node
     * represents a single declaration out of that.
     * @category Linter
     * @experimental
     */
    export interface VariableDeclarator {
      type: "VariableDeclarator";
      range: Range;
      id: ArrayPattern | ObjectPattern | Identifier;
      init: Expression | null;
      definite: boolean;
      parent: VariableDeclaration;
    }

    /**
     * Function/Method parameter
     * @category Linter
     * @experimental
     */
    export type Parameter =
      | ArrayPattern
      | AssignmentPattern
      | Identifier
      | ObjectPattern
      | RestElement
      | TSParameterProperty;

    /**
     * TypeScript accessibility modifiers used in classes
     * @category Linter
     * @experimental
     */
    export type Accessibility = "private" | "protected" | "public";

    /**
     * Declares a function in the current scope
     * @category Linter
     * @experimental
     */
    export interface FunctionDeclaration {
      type: "FunctionDeclaration";
      range: Range;
      declare: boolean;
      async: boolean;
      generator: boolean;
      id: Identifier | null;
      typeParameters: TSTypeParameterDeclaration | undefined;
      returnType: TSTypeAnnotation | undefined;
      body: BlockStatement | null;
      params: Parameter[];
      parent:
        | BlockStatement
        | ExportDefaultDeclaration
        | ExportNamedDeclaration
        | Program;
    }

    /**
     * Experimental: Decorators
     * @category Linter
     * @experimental
     */
    export interface Decorator {
      type: "Decorator";
      range: Range;
      expression:
        | ArrayExpression
        | ArrayPattern
        | ArrowFunctionExpression
        | CallExpression
        | ClassExpression
        | FunctionExpression
        | Identifier
        | JSXElement
        | JSXFragment
        | Literal
        | TemplateLiteral
        | MemberExpression
        | MetaProperty
        | ObjectExpression
        | ObjectPattern
        | SequenceExpression
        | Super
        | TaggedTemplateExpression
        | ThisExpression
        | TSAsExpression
        | TSNonNullExpression
        | TSTypeAssertion;
      parent: Node;
    }

    /**
     * Declares a class in the current scope
     * @category Linter
     * @experimental
     */
    export interface ClassDeclaration {
      type: "ClassDeclaration";
      range: Range;
      declare: boolean;
      abstract: boolean;
      id: Identifier | null;
      superClass:
        | ArrayExpression
        | ArrayPattern
        | ArrowFunctionExpression
        | CallExpression
        | ClassExpression
        | FunctionExpression
        | Identifier
        | JSXElement
        | JSXFragment
        | Literal
        | TemplateLiteral
        | MemberExpression
        | MetaProperty
        | ObjectExpression
        | ObjectPattern
        | SequenceExpression
        | Super
        | TaggedTemplateExpression
        | ThisExpression
        | TSAsExpression
        | TSNonNullExpression
        | TSTypeAssertion
        | null;
      implements: TSClassImplements[];
      body: ClassBody;
      parent: Node;
    }

    /**
     * Similar to ClassDeclaration but for declaring a class as an
     * expression. The main difference is that the class name(=id) can
     * be omitted.
     * @category Linter
     * @experimental
     */
    export interface ClassExpression {
      type: "ClassExpression";
      range: Range;
      declare: boolean;
      abstract: boolean;
      id: Identifier | null;
      superClass:
        | ArrayExpression
        | ArrayPattern
        | ArrowFunctionExpression
        | CallExpression
        | ClassExpression
        | FunctionExpression
        | Identifier
        | JSXElement
        | JSXFragment
        | Literal
        | TemplateLiteral
        | MemberExpression
        | MetaProperty
        | ObjectExpression
        | ObjectPattern
        | SequenceExpression
        | Super
        | TaggedTemplateExpression
        | ThisExpression
        | TSAsExpression
        | TSNonNullExpression
        | TSTypeAssertion
        | null;
      superTypeArguments: TSTypeParameterInstantiation | undefined;
      typeParameters: TSTypeParameterDeclaration | undefined;
      implements: TSClassImplements[];
      body: ClassBody;
      parent: Node;
    }

    /**
     * Represents the body of a class and contains all members
     * @category Linter
     * @experimental
     */
    export interface ClassBody {
      type: "ClassBody";
      range: Range;
      body: Array<
        | AccessorProperty
        | MethodDefinition
        | PropertyDefinition
        | StaticBlock
        // Stage 1 Proposal:
        // https://github.com/tc39/proposal-grouped-and-auto-accessors
        // | TSAbstractAccessorProperty
        | TSAbstractMethodDefinition
        | TSAbstractPropertyDefinition
        | TSIndexSignature
      >;
      parent: ClassDeclaration | ClassExpression;
    }

    /**
     * Static class initializiation block.
     * @category Linter
     * @experimental
     */
    export interface StaticBlock {
      type: "StaticBlock";
      range: Range;
      body: Statement[];
      parent: ClassBody;
    }

    // Stage 1 Proposal:
    // https://github.com/tc39/proposal-grouped-and-auto-accessors
    // | TSAbstractAccessorProperty
    /**
     * @category Linter
     * @experimental
     */
    export interface AccessorProperty {
      type: "AccessorProperty";
      range: Range;
      declare: boolean;
      computed: boolean;
      optional: boolean;
      override: boolean;
      readonly: boolean;
      static: boolean;
      accessibility: Accessibility | undefined;
      decorators: Decorator[];
      key: Expression | Identifier | NumberLiteral | StringLiteral;
      value: Expression | null;
      parent: ClassBody;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface PropertyDefinition {
      type: "PropertyDefinition";
      range: Range;
      declare: boolean;
      computed: boolean;
      optional: boolean;
      override: boolean;
      readonly: boolean;
      static: boolean;
      accessibility: Accessibility | undefined;
      decorators: Decorator[];
      key:
        | Expression
        | Identifier
        | NumberLiteral
        | StringLiteral
        | PrivateIdentifier;
      value: Expression | null;
      typeAnnotation: TSTypeAnnotation | undefined;
      parent: ClassBody;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface MethodDefinition {
      type: "MethodDefinition";
      range: Range;
      declare: boolean;
      computed: boolean;
      optional: boolean;
      override: boolean;
      readonly: boolean;
      static: boolean;
      kind: "constructor" | "get" | "method" | "set";
      accessibility: Accessibility | undefined;
      decorators: Decorator[];
      key:
        | PrivateIdentifier
        | Identifier
        | NumberLiteral
        | StringLiteral
        | Expression;
      value: FunctionExpression | TSEmptyBodyFunctionExpression;
      parent: ClassBody;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface BlockStatement {
      type: "BlockStatement";
      range: Range;
      body: Statement[];
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * The `debugger;` statement.
     * @category Linter
     * @experimental
     */
    export interface DebuggerStatement {
      type: "DebuggerStatement";
      range: Range;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Legacy JavaScript feature, that's discouraged from being used today.
     * @deprecated
     * @category Linter
     * @experimental
     */
    export interface WithStatement {
      type: "WithStatement";
      range: Range;
      object: Expression;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Returns a value from a function.
     * @category Linter
     * @experimental
     */
    export interface ReturnStatement {
      type: "ReturnStatement";
      range: Range;
      argument: Expression | null;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Custom control flow based on labels.
     * @category Linter
     * @experimental
     */
    export interface LabeledStatement {
      type: "LabeledStatement";
      range: Range;
      label: Identifier;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Break any loop or labeled statement, example:
     *
     * ```ts
     * while (true) {
     *   break;
     * }
     *
     * for (let i = 0; i < 10; i++) {
     *   if (i > 5) break;
     * }
     * ```
     * @category Linter
     * @experimental
     */
    export interface BreakStatement {
      type: "BreakStatement";
      range: Range;
      label: Identifier | null;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Terminates the current loop and continues with the next iteration.
     * @category Linter
     * @experimental
     */
    export interface ContinueStatement {
      type: "ContinueStatement";
      range: Range;
      label: Identifier | null;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Execute a statement the test passes, otherwise the alternate
     * statement, if it was defined.
     * @category Linter
     * @experimental
     */
    export interface IfStatement {
      type: "IfStatement";
      range: Range;
      test: Expression;
      consequent: Statement;
      alternate: Statement | null;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Match an expression against a series of cases.
     * @category Linter
     * @experimental
     */
    export interface SwitchStatement {
      type: "SwitchStatement";
      range: Range;
      discriminant: Expression;
      cases: SwitchCase[];
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * A single case of a SwitchStatement.
     * @category Linter
     * @experimental
     */
    export interface SwitchCase {
      type: "SwitchCase";
      range: Range;
      test: Expression | null;
      consequent: Statement[];
      parent: SwitchStatement;
    }

    /**
     * Throw a user defined exception. Stops execution
     * of the current function.
     * @category Linter
     * @experimental
     */
    export interface ThrowStatement {
      type: "ThrowStatement";
      range: Range;
      argument: Expression;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Run a loop while the test expression is truthy.
     * @category Linter
     * @experimental
     */
    export interface WhileStatement {
      type: "WhileStatement";
      range: Range;
      test: Expression;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Re-run loop for as long as test expression is truthy.
     * @category Linter
     * @experimental
     */
    export interface DoWhileStatement {
      type: "DoWhileStatement";
      range: Range;
      test: Expression;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Classic for-loop.
     * @category Linter
     * @experimental
     */
    export interface ForStatement {
      type: "ForStatement";
      range: Range;
      init: Expression | VariableDeclaration | null;
      test: Expression | null;
      update: Expression | null;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Enumerate over all enumerable string properties of an object.
     * @category Linter
     * @experimental
     */
    export interface ForInStatement {
      type: "ForInStatement";
      range: Range;
      left: Expression | VariableDeclaration;
      right: Expression;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Iterate over sequence of values from an iterator.
     * @category Linter
     * @experimental
     */
    export interface ForOfStatement {
      type: "ForOfStatement";
      range: Range;
      await: boolean;
      left: Expression | VariableDeclaration;
      right: Expression;
      body: Statement;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Statement that holds an expression.
     * @category Linter
     * @experimental
     */
    export interface ExpressionStatement {
      type: "ExpressionStatement";
      range: Range;
      expression: Expression;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Try/catch statement
     * @category Linter
     * @experimental
     */
    export interface TryStatement {
      type: "TryStatement";
      range: Range;
      block: BlockStatement;
      handler: CatchClause | null;
      finalizer: BlockStatement | null;
      parent:
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * The catch clause of a try/catch statement
     * @category Linter
     * @experimental
     */
    export interface CatchClause {
      type: "CatchClause";
      range: Range;
      param: ArrayPattern | ObjectPattern | Identifier | null;
      body: BlockStatement;
      parent: TryStatement;
    }

    /**
     * An array literal
     * @category Linter
     * @experimental
     */
    export interface ArrayExpression {
      type: "ArrayExpression";
      range: Range;
      elements: Array<Expression | SpreadElement>;
      parent: Node;
    }

    /**
     * An object literal.
     * @category Linter
     * @experimental
     */
    export interface ObjectExpression {
      type: "ObjectExpression";
      range: Range;
      properties: Array<Property | SpreadElement>;
      parent: Node;
    }

    /**
     * Compare left and right value with the specifier operator.
     * @category Linter
     * @experimental
     */
    export interface BinaryExpression {
      type: "BinaryExpression";
      range: Range;
      operator:
        | "&"
        | "**"
        | "*"
        | "||"
        | "|"
        | "^"
        | "==="
        | "=="
        | "!=="
        | "!="
        | ">="
        | ">>>"
        | ">>"
        | ">"
        | "in"
        | "instanceof"
        | "<="
        | "<<"
        | "<"
        | "-"
        | "%"
        | "+"
        | "/";
      left: Expression | PrivateIdentifier;
      right: Expression;
      parent: Node;
    }

    /**
     * Chain expressions based on the operator specified
     * @category Linter
     * @experimental
     */
    export interface LogicalExpression {
      type: "LogicalExpression";
      range: Range;
      operator: "&&" | "??" | "||";
      left: Expression;
      right: Expression;
      parent: Node;
    }

    /**
     * Declare a function as an expression. Similar to `FunctionDeclaration`,
     * with an optional name (=id).
     * @category Linter
     * @experimental
     */
    export interface FunctionExpression {
      type: "FunctionExpression";
      range: Range;
      async: boolean;
      generator: boolean;
      id: Identifier | null;
      typeParameters: TSTypeParameterDeclaration | undefined;
      params: Parameter[];
      returnType: TSTypeAnnotation | undefined;
      body: BlockStatement;
      parent: Node;
    }

    /**
     * Arrow function expression
     * @category Linter
     * @experimental
     */
    export interface ArrowFunctionExpression {
      type: "ArrowFunctionExpression";
      range: Range;
      async: boolean;
      generator: boolean;
      id: null;
      typeParameters: TSTypeParameterDeclaration | undefined;
      params: Parameter[];
      returnType: TSTypeAnnotation | undefined;
      body: BlockStatement | Expression;
      parent: Node;
    }

    /**
     * The `this` keyword used in classes.
     * @category Linter
     * @experimental
     */
    export interface ThisExpression {
      type: "ThisExpression";
      range: Range;
      parent: Node;
    }

    /**
     * The `super` keyword used in classes.
     * @category Linter
     * @experimental
     */
    export interface Super {
      type: "Super";
      range: Range;
      parent: Node;
    }

    /**
     * Apply operand on value based on the specified operator.
     * @category Linter
     * @experimental
     */
    export interface UnaryExpression {
      type: "UnaryExpression";
      range: Range;
      operator: "!" | "+" | "~" | "-" | "delete" | "typeof" | "void";
      argument: Expression;
      parent: Node;
    }

    /**
     * Create a new instance of a class.
     * @category Linter
     * @experimental
     */
    export interface NewExpression {
      type: "NewExpression";
      range: Range;
      callee: Expression;
      typeArguments: TSTypeParameterInstantiation | undefined;
      arguments: Array<Expression | SpreadElement>;
      parent: Node;
    }

    /**
     * Dynamically import a module.
     * @category Linter
     * @experimental
     */
    export interface ImportExpression {
      type: "ImportExpression";
      range: Range;
      source: Expression;
      options: Expression | null;
      parent: Node;
    }

    /**
     * A function call.
     * @category Linter
     * @experimental
     */
    export interface CallExpression {
      type: "CallExpression";
      range: Range;
      optional: boolean;
      callee: Expression;
      typeArguments: TSTypeParameterInstantiation | null;
      arguments: Array<Expression | SpreadElement>;
      parent: Node;
    }

    /**
     * Syntactic sugar to increment or decrement a value.
     * @category Linter
     * @experimental
     */
    export interface UpdateExpression {
      type: "UpdateExpression";
      range: Range;
      prefix: boolean;
      operator: "++" | "--";
      argument: Expression;
      parent: Node;
    }

    /**
     * Updaate a variable or property.
     * @category Linter
     * @experimental
     */
    export interface AssignmentExpression {
      type: "AssignmentExpression";
      range: Range;
      operator:
        | "&&="
        | "&="
        | "**="
        | "*="
        | "||="
        | "|="
        | "^="
        | "="
        | ">>="
        | ">>>="
        | "<<="
        | "-="
        | "%="
        | "+="
        | "??="
        | "/=";
      left: Expression;
      right: Expression;
      parent: Node;
    }

    /**
     * Inline if-statement.
     * @category Linter
     * @experimental
     */
    export interface ConditionalExpression {
      type: "ConditionalExpression";
      range: Range;
      test: Expression;
      consequent: Expression;
      alternate: Expression;
      parent: Node;
    }

    /**
     * MemberExpression
     * @category Linter
     * @experimental
     */
    export interface MemberExpression {
      type: "MemberExpression";
      range: Range;
      optional: boolean;
      computed: boolean;
      object: Expression;
      property: Expression | Identifier | PrivateIdentifier;
      parent: Node;
    }

    /**
     * ChainExpression
     * @category Linter
     * @experimental
     */
    export interface ChainExpression {
      type: "ChainExpression";
      range: Range;
      expression:
        | CallExpression
        | MemberExpression
        | TSNonNullExpression;
      parent: Node;
    }

    /**
     * Execute multiple expressions in sequence.
     * @category Linter
     * @experimental
     */
    export interface SequenceExpression {
      type: "SequenceExpression";
      range: Range;
      expressions: Expression[];
      parent: Node;
    }

    /**
     * A template literal string.
     * @category Linter
     * @experimental
     */
    export interface TemplateLiteral {
      type: "TemplateLiteral";
      range: Range;
      quasis: TemplateElement[];
      expressions: Expression[];
      parent: Node;
    }

    /**
     * The static portion of a template literal.
     * @category Linter
     * @experimental
     */
    export interface TemplateElement {
      type: "TemplateElement";
      range: Range;
      tail: boolean;
      raw: string;
      cooked: string;
      parent: TemplateLiteral | TSTemplateLiteralType;
    }

    /**
     * Tagged template expression.
     * @category Linter
     * @experimental
     */
    export interface TaggedTemplateExpression {
      type: "TaggedTemplateExpression";
      range: Range;
      tag: Expression;
      typeArguments: TSTypeParameterInstantiation | undefined;
      quasi: TemplateLiteral;
      parent: Node;
    }

    /**
     * Pause or resume a generator function.
     * @category Linter
     * @experimental
     */
    export interface YieldExpression {
      type: "YieldExpression";
      range: Range;
      delegate: boolean;
      argument: Expression | null;
      parent: Node;
    }

    /**
     * Await a `Promise` and get its fulfilled value.
     * @category Linter
     * @experimental
     */
    export interface AwaitExpression {
      type: "AwaitExpression";
      range: Range;
      argument: Expression;
      parent: Node;
    }

    /**
     * Can either be `import.meta` or `new.target`.
     * @category Linter
     * @experimental
     */
    export interface MetaProperty {
      type: "MetaProperty";
      range: Range;
      meta: Identifier;
      property: Identifier;
      parent: Node;
    }

    /**
     * Custom named node by the developer. Can be a variable name,
     * a function name, parameter, etc.
     * @category Linter
     * @experimental
     */
    export interface Identifier {
      type: "Identifier";
      range: Range;
      name: string;
      optional: boolean;
      typeAnnotation: TSTypeAnnotation | undefined;
      parent: Node;
    }

    /**
     * Private members inside of classes, must start with `#`.
     * @category Linter
     * @experimental
     */
    export interface PrivateIdentifier {
      type: "PrivateIdentifier";
      range: Range;
      name: string;
      parent:
        | TSAbstractPropertyDefinition
        | TSPropertySignature
        | PropertyDefinition
        | MethodDefinition
        | BinaryExpression
        | MemberExpression;
    }

    /**
     * Assign default values in parameters.
     * @category Linter
     * @experimental
     */
    export interface AssignmentPattern {
      type: "AssignmentPattern";
      range: Range;
      left: ArrayPattern | ObjectPattern | Identifier;
      right: Expression;
      parent: Node;
    }

    /**
     * Destructure an array.
     * @category Linter
     * @experimental
     */
    export interface ArrayPattern {
      type: "ArrayPattern";
      range: Range;
      optional: boolean;
      typeAnnotation: TSTypeAnnotation | undefined;
      elements: Array<
        | ArrayPattern
        | AssignmentPattern
        | Identifier
        | MemberExpression
        | ObjectPattern
        | RestElement
        | null
      >;
      parent: Node;
    }

    /**
     * Destructure an object.
     * @category Linter
     * @experimental
     */
    export interface ObjectPattern {
      type: "ObjectPattern";
      range: Range;
      optional: boolean;
      typeAnnotation: TSTypeAnnotation | undefined;
      properties: Array<Property | RestElement>;
      parent: Node;
    }

    /**
     * The rest of function parameters.
     * @category Linter
     * @experimental
     */
    export interface RestElement {
      type: "RestElement";
      range: Range;
      typeAnnotation: TSTypeAnnotation | undefined;
      argument:
        | ArrayPattern
        | AssignmentPattern
        | Identifier
        | MemberExpression
        | ObjectPattern
        | RestElement;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface SpreadElement {
      type: "SpreadElement";
      range: Range;
      argument: Expression;
      parent:
        | ArrayExpression
        | CallExpression
        | NewExpression
        | ObjectExpression;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface Property {
      type: "Property";
      range: Range;
      shorthand: boolean;
      computed: boolean;
      method: boolean;
      kind: "get" | "init" | "set";
      key: Expression | Identifier | NumberLiteral | StringLiteral;
      value:
        | AssignmentPattern
        | ArrayPattern
        | ObjectPattern
        | Identifier
        | Expression
        | TSEmptyBodyFunctionExpression;
      parent: ObjectExpression | ObjectPattern;
    }

    /**
     * Represents numbers that are too high or too low to be represented
     * by the `number` type.
     *
     * ```ts
     * const a = 9007199254740991n;
     * ```
     * @category Linter
     * @experimental
     */
    export interface BigIntLiteral {
      type: "Literal";
      range: Range;
      raw: string;
      bigint: string;
      value: bigint;
      parent: Node;
    }

    /**
     * Either `true` or `false`
     * @category Linter
     * @experimental
     */
    export interface BooleanLiteral {
      type: "Literal";
      range: Range;
      raw: "false" | "true";
      value: boolean;
      parent: Node;
    }

    /**
     * A number literal
     *
     * ```ts
     * 1;
     * 1.2;
     * ```
     * @category Linter
     * @experimental
     */
    export interface NumberLiteral {
      type: "Literal";
      range: Range;
      raw: string;
      value: number;
      parent: Node;
    }

    /**
     * The `null` literal
     * @category Linter
     * @experimental
     */
    export interface NullLiteral {
      type: "Literal";
      range: Range;
      raw: "null";
      value: null;
      parent: Node;
    }

    /**
     * A string literal
     *
     * ```ts
     * "foo";
     * 'foo "bar"';
     * ```
     * @category Linter
     * @experimental
     */
    export interface StringLiteral {
      type: "Literal";
      range: Range;
      raw: string;
      value: string;
      parent: Node;
    }

    /**
     * A regex literal:
     *
     * ```ts
     * /foo(bar|baz)$/g
     * ```
     * @category Linter
     * @experimental
     */
    export interface RegExpLiteral {
      type: "Literal";
      range: Range;
      raw: string;
      regex: {
        flags: string;
        pattern: string;
      };
      value: RegExp | null;
      parent: Node;
    }

    /**
     * Union type of all Literals
     * @category Linter
     * @experimental
     */
    export type Literal =
      | BigIntLiteral
      | BooleanLiteral
      | NullLiteral
      | NumberLiteral
      | RegExpLiteral
      | StringLiteral;

    /**
     * User named identifier inside JSX.
     * @category Linter
     * @experimental
     */
    export interface JSXIdentifier {
      type: "JSXIdentifier";
      range: Range;
      name: string;
      parent:
        | JSXNamespacedName
        | JSXOpeningElement
        | JSXAttribute
        | JSXClosingElement
        | JSXMemberExpression;
    }

    /**
     * Namespaced name in JSX
     * @category Linter
     * @experimental
     */
    export interface JSXNamespacedName {
      type: "JSXNamespacedName";
      range: Range;
      namespace: JSXIdentifier;
      name: JSXIdentifier;
      parent:
        | JSXOpeningElement
        | JSXAttribute
        | JSXClosingElement
        | JSXMemberExpression;
    }

    /**
     * Empty JSX expression.
     * @category Linter
     * @experimental
     */
    export interface JSXEmptyExpression {
      type: "JSXEmptyExpression";
      range: Range;
      parent: JSXAttribute | JSXElement | JSXFragment;
    }

    /**
     * A JSX element.
     * @category Linter
     * @experimental
     */
    export interface JSXElement {
      type: "JSXElement";
      range: Range;
      openingElement: JSXOpeningElement;
      closingElement: JSXClosingElement | null;
      children: JSXChild[];
      parent: Node;
    }

    /**
     * The opening tag of a JSXElement
     * @category Linter
     * @experimental
     */
    export interface JSXOpeningElement {
      type: "JSXOpeningElement";
      range: Range;
      selfClosing: boolean;
      name:
        | JSXIdentifier
        | JSXMemberExpression
        | JSXNamespacedName;
      attributes: Array<JSXAttribute | JSXSpreadAttribute>;
      typeArguments: TSTypeParameterInstantiation | undefined;
      parent: JSXElement;
    }

    /**
     * A JSX attribute
     * @category Linter
     * @experimental
     */
    export interface JSXAttribute {
      type: "JSXAttribute";
      range: Range;
      name: JSXIdentifier | JSXNamespacedName;
      value:
        | JSXElement
        | JSXExpressionContainer
        | Literal
        | null;
      parent: JSXOpeningElement;
    }

    /**
     * Spreads an object as JSX attributes.
     * @category Linter
     * @experimental
     */
    export interface JSXSpreadAttribute {
      type: "JSXSpreadAttribute";
      range: Range;
      argument: Expression;
      parent: JSXOpeningElement;
    }

    /**
     * The closing tag of a JSXElement. Only used when the element
     * is not self-closing.
     * @category Linter
     * @experimental
     */
    export interface JSXClosingElement {
      type: "JSXClosingElement";
      range: Range;
      name:
        | JSXIdentifier
        | JSXMemberExpression
        | JSXNamespacedName;
      parent: JSXElement;
    }

    /**
     * Usually a passthrough node to pass multiple sibling elements as
     * the JSX syntax requires one root element.
     * @category Linter
     * @experimental
     */
    export interface JSXFragment {
      type: "JSXFragment";
      range: Range;
      openingFragment: JSXOpeningFragment;
      closingFragment: JSXClosingFragment;
      children: JSXChild[];
      parent: Node;
    }

    /**
     * The opening tag of a JSXFragment.
     * @category Linter
     * @experimental
     */
    export interface JSXOpeningFragment {
      type: "JSXOpeningFragment";
      range: Range;
      parent: JSXFragment;
    }

    /**
     * The closing tag of a JSXFragment.
     * @category Linter
     * @experimental
     */
    export interface JSXClosingFragment {
      type: "JSXClosingFragment";
      range: Range;
      parent: JSXFragment;
    }

    /**
     * Inserts a normal JS expression into JSX.
     * @category Linter
     * @experimental
     */
    export interface JSXExpressionContainer {
      type: "JSXExpressionContainer";
      range: Range;
      expression: Expression | JSXEmptyExpression;
      parent: JSXAttribute | JSXElement | JSXFragment;
    }

    /**
     * Plain text in JSX.
     * @category Linter
     * @experimental
     */
    export interface JSXText {
      type: "JSXText";
      range: Range;
      raw: string;
      value: string;
      parent: JSXElement | JSXFragment;
    }

    /**
     * JSX member expression.
     * @category Linter
     * @experimental
     */
    export interface JSXMemberExpression {
      type: "JSXMemberExpression";
      range: Range;
      object:
        | JSXIdentifier
        | JSXMemberExpression
        | JSXNamespacedName;
      property: JSXIdentifier;
      parent: JSXOpeningElement | JSXClosingElement;
    }

    /**
     * Union type of all possible child nodes in JSX
     * @category Linter
     * @experimental
     */
    export type JSXChild =
      | JSXElement
      | JSXExpressionContainer
      | JSXFragment
      | JSXText;

    /**
     * @category Linter
     * @experimental
     */
    export interface TSModuleDeclaration {
      type: "TSModuleDeclaration";
      range: Range;
      declare: boolean;
      kind: "global" | "module" | "namespace";
      id: Identifier | Literal | TSQualifiedName;
      body: TSModuleBlock | undefined;
      parent:
        | ExportDefaultDeclaration
        | ExportNamedDeclaration
        | Program
        | StaticBlock
        | BlockStatement
        | WithStatement
        | LabeledStatement
        | IfStatement
        | SwitchCase
        | WhileStatement
        | DoWhileStatement
        | ForStatement
        | ForInStatement
        | ForOfStatement
        | TSModuleBlock;
    }

    /**
     * Body of a `TSModuleDeclaration`
     * @category Linter
     * @experimental
     */
    export interface TSModuleBlock {
      type: "TSModuleBlock";
      range: Range;
      body: Array<
        | ExportAllDeclaration
        | ExportDefaultDeclaration
        | ExportNamedDeclaration
        | ImportDeclaration
        | Statement
        | TSImportEqualsDeclaration
        | TSNamespaceExportDeclaration
      >;
      parent: TSModuleDeclaration;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSClassImplements {
      type: "TSClassImplements";
      range: Range;
      expression: Expression;
      typeArguments: TSTypeParameterInstantiation | undefined;
      parent: ClassDeclaration | ClassExpression;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSAbstractMethodDefinition {
      type: "TSAbstractMethodDefinition";
      range: Range;
      computed: boolean;
      optional: boolean;
      override: boolean;
      static: boolean;
      accessibility: Accessibility | undefined;
      kind: "method";
      key: Expression | Identifier | NumberLiteral | StringLiteral;
      value: FunctionExpression | TSEmptyBodyFunctionExpression;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSAbstractPropertyDefinition {
      type: "TSAbstractPropertyDefinition";
      range: Range;
      computed: boolean;
      optional: boolean;
      override: boolean;
      static: boolean;
      definite: boolean;
      declare: boolean;
      readonly: boolean;
      accessibility: Accessibility | undefined;
      decorators: Decorator[];
      key:
        | Expression
        | PrivateIdentifier
        | Identifier
        | NumberLiteral
        | StringLiteral;
      typeAnnotation: TSTypeAnnotation | undefined;
      value: Expression | null;
      parent: ClassBody;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSEmptyBodyFunctionExpression {
      type: "TSEmptyBodyFunctionExpression";
      range: Range;
      declare: boolean;
      expression: boolean;
      async: boolean;
      generator: boolean;
      id: null;
      body: null;
      typeParameters: TSTypeParameterDeclaration | undefined;
      params: Parameter[];
      returnType: TSTypeAnnotation | undefined;
      parent:
        | MethodDefinition
        | Property
        | TSAbstractMethodDefinition
        | TSParameterProperty;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSParameterProperty {
      type: "TSParameterProperty";
      range: Range;
      override: boolean;
      readonly: boolean;
      static: boolean;
      accessibility: Accessibility | undefined;
      decorators: Decorator[];
      parameter:
        | AssignmentPattern
        | ArrayPattern
        | ObjectPattern
        | Identifier
        | RestElement;
      parent:
        | ArrowFunctionExpression
        | FunctionDeclaration
        | FunctionExpression
        | TSDeclareFunction
        | TSEmptyBodyFunctionExpression;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSCallSignatureDeclaration {
      type: "TSCallSignatureDeclaration";
      range: Range;
      typeParameters: TSTypeParameterDeclaration | undefined;
      params: Parameter[];
      returnType: TSTypeAnnotation | undefined;
      parent: TSInterfaceBody | TSTypeLiteral;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSPropertySignature {
      type: "TSPropertySignature";
      range: Range;
      computed: boolean;
      optional: boolean;
      readonly: boolean;
      static: boolean;
      key:
        | PrivateIdentifier
        | Expression
        | Identifier
        | NumberLiteral
        | StringLiteral;
      typeAnnotation: TSTypeAnnotation | undefined;
      parent: TSInterfaceBody | TSTypeLiteral;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSDeclareFunction {
      type: "TSDeclareFunction";
      range: Range;
      async: boolean;
      declare: boolean;
      generator: boolean;
      body: undefined;
      id: Identifier | null;
      params: Parameter[];
      returnType: TSTypeAnnotation | undefined;
      typeParameters: TSTypeParameterDeclaration | undefined;
      parent: Node;
    }

    /**
     * ```ts
     * enum Foo { A, B };
     * ```
     * @category Linter
     * @experimental
     */
    export interface TSEnumDeclaration {
      type: "TSEnumDeclaration";
      range: Range;
      declare: boolean;
      const: boolean;
      id: Identifier;
      body: TSEnumBody;
      parent: Node;
    }

    /**
     * The body of a `TSEnumDeclaration`
     * @category Linter
     * @experimental
     */
    export interface TSEnumBody {
      type: "TSEnumBody";
      range: Range;
      members: TSEnumMember[];
      parent: TSEnumDeclaration;
    }

    /**
     * A member of a `TSEnumDeclaration`
     * @category Linter
     * @experimental
     */
    export interface TSEnumMember {
      type: "TSEnumMember";
      range: Range;
      id:
        | Identifier
        | NumberLiteral
        | StringLiteral;
      initializer: Expression | undefined;
      parent: TSEnumBody;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeAssertion {
      type: "TSTypeAssertion";
      range: Range;
      expression: Expression;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeParameterInstantiation {
      type: "TSTypeParameterInstantiation";
      range: Range;
      params: TypeNode[];
      parent:
        | ClassExpression
        | NewExpression
        | CallExpression
        | TaggedTemplateExpression
        | JSXOpeningElement
        | TSClassImplements
        | TSInstantiationExpression
        | TSInterfaceHeritage
        | TSTypeQuery
        | TSTypeReference
        | TSImportType;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeAliasDeclaration {
      type: "TSTypeAliasDeclaration";
      range: Range;
      declare: boolean;
      id: Identifier;
      typeParameters: TSTypeParameterDeclaration | undefined;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSSatisfiesExpression {
      type: "TSSatisfiesExpression";
      range: Range;
      expression: Expression;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSAsExpression {
      type: "TSAsExpression";
      range: Range;
      expression: Expression;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSInstantiationExpression {
      type: "TSInstantiationExpression";
      range: Range;
      expression: Expression;
      typeArguments: TSTypeParameterInstantiation;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSNonNullExpression {
      type: "TSNonNullExpression";
      range: Range;
      expression: Expression;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSThisType {
      type: "TSThisType";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSInterfaceDeclaration {
      type: "TSInterfaceDeclaration";
      range: Range;
      declare: boolean;
      id: Identifier;
      extends: TSInterfaceHeritage[];
      typeParameters: TSTypeParameterDeclaration | undefined;
      body: TSInterfaceBody;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSInterfaceBody {
      type: "TSInterfaceBody";
      range: Range;
      body: Array<
        | TSCallSignatureDeclaration
        | TSConstructSignatureDeclaration
        | TSIndexSignature
        | TSMethodSignature
        | TSPropertySignature
      >;
      parent: TSInterfaceDeclaration;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSConstructSignatureDeclaration {
      type: "TSConstructSignatureDeclaration";
      range: Range;
      typeParameters: TSTypeParameterDeclaration | undefined;
      params: Parameter[];
      returnType: TSTypeAnnotation;
      parent: TSInterfaceBody | TSTypeLiteral;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSMethodSignature {
      type: "TSMethodSignature";
      range: Range;
      computed: boolean;
      optional: boolean;
      readonly: boolean;
      static: boolean;
      kind: "get" | "set" | "method";
      key: Expression | Identifier | NumberLiteral | StringLiteral;
      returnType: TSTypeAnnotation | undefined;
      params: Parameter[];
      typeParameters: TSTypeParameterDeclaration | undefined;
      parent: TSInterfaceBody | TSTypeLiteral;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSInterfaceHeritage {
      type: "TSInterfaceHeritage";
      range: Range;
      expression: Expression;
      typeArguments: TSTypeParameterInstantiation | undefined;
      parent: TSInterfaceBody;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSIndexSignature {
      type: "TSIndexSignature";
      range: Range;
      readonly: boolean;
      static: boolean;
      parameters: Parameter[];
      typeAnnotation: TSTypeAnnotation | undefined;
      parent: ClassBody | TSInterfaceBody | TSTypeLiteral;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSUnionType {
      type: "TSUnionType";
      range: Range;
      types: TypeNode[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSIntersectionType {
      type: "TSIntersectionType";
      range: Range;
      types: TypeNode[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSInferType {
      type: "TSInferType";
      range: Range;
      typeParameter: TSTypeParameter;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeOperator {
      type: "TSTypeOperator";
      range: Range;
      operator: "keyof" | "readonly" | "unique";
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSIndexedAccessType {
      type: "TSIndexedAccessType";
      range: Range;
      indexType: TypeNode;
      objectType: TypeNode;
      parent: Node;
    }

    /**
     * ```ts
     * const a: any = null;
     * ```
     * @category Linter
     * @experimental
     */
    export interface TSAnyKeyword {
      type: "TSAnyKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSUnknownKeyword {
      type: "TSUnknownKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSNumberKeyword {
      type: "TSNumberKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSObjectKeyword {
      type: "TSObjectKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSBooleanKeyword {
      type: "TSBooleanKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSBigIntKeyword {
      type: "TSBigIntKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSStringKeyword {
      type: "TSStringKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSSymbolKeyword {
      type: "TSSymbolKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSVoidKeyword {
      type: "TSVoidKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSUndefinedKeyword {
      type: "TSUndefinedKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSNullKeyword {
      type: "TSNullKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSNeverKeyword {
      type: "TSNeverKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSIntrinsicKeyword {
      type: "TSIntrinsicKeyword";
      range: Range;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSRestType {
      type: "TSRestType";
      range: Range;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSConditionalType {
      type: "TSConditionalType";
      range: Range;
      checkType: TypeNode;
      extendsType: TypeNode;
      trueType: TypeNode;
      falseType: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSMappedType {
      type: "TSMappedType";
      range: Range;
      readonly: boolean;
      optional: boolean;
      nameType: TypeNode | null;
      typeAnnotation: TypeNode | undefined;
      constraint: TypeNode;
      key: Identifier;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSLiteralType {
      type: "TSLiteralType";
      range: Range;
      literal: Literal | TemplateLiteral | UnaryExpression | UpdateExpression;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTemplateLiteralType {
      type: "TSTemplateLiteralType";
      range: Range;
      quasis: TemplateElement[];
      types: TypeNode[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeLiteral {
      type: "TSTypeLiteral";
      range: Range;
      members: Array<
        | TSCallSignatureDeclaration
        | TSConstructSignatureDeclaration
        | TSIndexSignature
        | TSMethodSignature
        | TSPropertySignature
      >;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSOptionalType {
      type: "TSOptionalType";
      range: Range;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeAnnotation {
      type: "TSTypeAnnotation";
      range: Range;
      typeAnnotation: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSArrayType {
      type: "TSArrayType";
      range: Range;
      elementType: TypeNode;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeQuery {
      type: "TSTypeQuery";
      range: Range;
      exprName: Identifier | ThisExpression | TSQualifiedName | TSImportType;
      typeArguments: TSTypeParameterInstantiation | undefined;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeReference {
      type: "TSTypeReference";
      range: Range;
      typeName: Identifier | ThisExpression | TSQualifiedName;
      typeArguments: TSTypeParameterInstantiation | undefined;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypePredicate {
      type: "TSTypePredicate";
      range: Range;
      asserts: boolean;
      parameterName: Identifier | TSThisType;
      typeAnnotation: TSTypeAnnotation | undefined;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTupleType {
      type: "TSTupleType";
      range: Range;
      elementTypes: TypeNode[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSNamedTupleMember {
      type: "TSNamedTupleMember";
      range: Range;
      label: Identifier;
      elementType: TypeNode;
      optional: boolean;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeParameterDeclaration {
      type: "TSTypeParameterDeclaration";
      range: Range;
      params: TSTypeParameter[];
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSTypeParameter {
      type: "TSTypeParameter";
      range: Range;
      in: boolean;
      out: boolean;
      const: boolean;
      name: Identifier;
      constraint: TypeNode | null;
      default: TypeNode | null;
      parent: TSInferType | TSMappedType | TSTypeParameterDeclaration;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSImportType {
      type: "TSImportType";
      range: Range;
      argument: TypeNode;
      qualifier: Identifier | ThisExpression | TSQualifiedName | null;
      typeArguments: TSTypeParameterInstantiation | null;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSExportAssignment {
      type: "TSExportAssignment";
      range: Range;
      expression: Expression;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSFunctionType {
      type: "TSFunctionType";
      range: Range;
      params: Parameter[];
      returnType: TSTypeAnnotation | undefined;
      typeParameters: TSTypeParameterDeclaration | undefined;
      parent: Node;
    }

    /**
     * @category Linter
     * @experimental
     */
    export interface TSQualifiedName {
      type: "TSQualifiedName";
      range: Range;
      left: Identifier | ThisExpression | TSQualifiedName;
      right: Identifier;
      parent: Node;
    }

    /**
     * Union type of all possible statement nodes
     * @category Linter
     * @experimental
     */
    export type Statement =
      | BlockStatement
      | BreakStatement
      | ClassDeclaration
      | ContinueStatement
      | DebuggerStatement
      | DoWhileStatement
      | ExportAllDeclaration
      | ExportDefaultDeclaration
      | ExportNamedDeclaration
      | ExpressionStatement
      | ForInStatement
      | ForOfStatement
      | ForStatement
      | FunctionDeclaration
      | IfStatement
      | ImportDeclaration
      | LabeledStatement
      | ReturnStatement
      | SwitchStatement
      | ThrowStatement
      | TryStatement
      | TSDeclareFunction
      | TSEnumDeclaration
      | TSExportAssignment
      | TSImportEqualsDeclaration
      | TSInterfaceDeclaration
      | TSModuleDeclaration
      | TSNamespaceExportDeclaration
      | TSTypeAliasDeclaration
      | VariableDeclaration
      | WhileStatement
      | WithStatement;

    /**
     * Union type of all possible expression nodes
     * @category Linter
     * @experimental
     */
    export type Expression =
      | ArrayExpression
      | ArrayPattern
      | ArrowFunctionExpression
      | AssignmentExpression
      | AwaitExpression
      | BinaryExpression
      | CallExpression
      | ChainExpression
      | ClassExpression
      | ConditionalExpression
      | FunctionExpression
      | Identifier
      | ImportExpression
      | JSXElement
      | JSXFragment
      | Literal
      | TemplateLiteral
      | LogicalExpression
      | MemberExpression
      | MetaProperty
      | NewExpression
      | ObjectExpression
      | ObjectPattern
      | SequenceExpression
      | Super
      | TaggedTemplateExpression
      | TemplateLiteral
      | ThisExpression
      | TSAsExpression
      | TSInstantiationExpression
      | TSNonNullExpression
      | TSSatisfiesExpression
      | TSTypeAssertion
      | UnaryExpression
      | UpdateExpression
      | YieldExpression;

    /**
     * Union type of all possible type nodes in TypeScript
     * @category Linter
     * @experimental
     */
    export type TypeNode =
      | TSAnyKeyword
      | TSArrayType
      | TSBigIntKeyword
      | TSBooleanKeyword
      | TSConditionalType
      | TSFunctionType
      | TSImportType
      | TSIndexedAccessType
      | TSInferType
      | TSIntersectionType
      | TSIntrinsicKeyword
      | TSLiteralType
      | TSMappedType
      | TSNamedTupleMember
      | TSNeverKeyword
      | TSNullKeyword
      | TSNumberKeyword
      | TSObjectKeyword
      | TSOptionalType
      | TSQualifiedName
      | TSRestType
      | TSStringKeyword
      | TSSymbolKeyword
      | TSTemplateLiteralType
      | TSThisType
      | TSTupleType
      | TSTypeLiteral
      | TSTypeOperator
      | TSTypePredicate
      | TSTypeQuery
      | TSTypeReference
      | TSUndefinedKeyword
      | TSUnionType
      | TSUnknownKeyword
      | TSVoidKeyword;

    /**
     * Union type of all possible AST nodes
     * @category Linter
     * @experimental
     */
    export type Node =
      | Program
      | Expression
      | Statement
      | TypeNode
      | ImportSpecifier
      | ImportDefaultSpecifier
      | ImportNamespaceSpecifier
      | ImportAttribute
      | TSExternalModuleReference
      | ExportSpecifier
      | VariableDeclarator
      | Decorator
      | ClassBody
      | StaticBlock
      | PropertyDefinition
      | MethodDefinition
      | SwitchCase
      | CatchClause
      | TemplateElement
      | PrivateIdentifier
      | AssignmentPattern
      | RestElement
      | SpreadElement
      | Property
      | JSXIdentifier
      | JSXNamespacedName
      | JSXEmptyExpression
      | JSXOpeningElement
      | JSXAttribute
      | JSXSpreadAttribute
      | JSXClosingElement
      | JSXOpeningFragment
      | JSXClosingFragment
      | JSXExpressionContainer
      | JSXText
      | JSXMemberExpression
      | TSModuleBlock
      | TSClassImplements
      | TSAbstractMethodDefinition
      | TSAbstractPropertyDefinition
      | TSEmptyBodyFunctionExpression
      | TSCallSignatureDeclaration
      | TSPropertySignature
      | TSEnumBody
      | TSEnumMember
      | TSTypeParameterInstantiation
      | TSInterfaceBody
      | TSConstructSignatureDeclaration
      | TSMethodSignature
      | TSInterfaceHeritage
      | TSIndexSignature
      | TSTypeAnnotation
      | TSTypeParameterDeclaration
      | TSTypeParameter;

    export {}; // only export exports
  }

  /**
   * The webgpu namespace provides additional APIs that the WebGPU specification
   * does not specify.
   *
   * @category GPU
   * @experimental
   */
  export namespace webgpu {
    /**
     * Starts a frame capture.
     *
     * This API is useful for debugging issues related to graphics, and makes
     * the captured data available to RenderDoc or XCode
     * (or other software for debugging frames)
     *
     * @category GPU
     * @experimental
     */
    export function deviceStartCapture(device: GPUDevice): void;
    /**
     * Stops a frame capture.
     *
     * This API is useful for debugging issues related to graphics, and makes
     * the captured data available to RenderDoc or XCode
     * (or other software for debugging frames)
     *
     * @category GPU
     * @experimental
     */
    export function deviceStopCapture(device: GPUDevice): void;

    export {}; // only export exports
  }

  export {}; // only export exports
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category Workers
 * @experimental
 */
interface WorkerOptions {
  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Configure permissions options to change the level of access the worker will
   * have. By default it will have no permissions. Note that the permissions
   * of a worker can't be extended beyond its parent's permissions reach.
   *
   * - `"inherit"` will take the permissions of the thread the worker is created
   *   in.
   * - `"none"` will use the default behavior and have no permission
   * - A list of routes can be provided that are relative to the file the worker
   *   is created in to limit the access of the worker (read/write permissions
   *   only)
   *
   * Example:
   *
   * ```ts
   * // mod.ts
   * const worker = new Worker(
   *   new URL("deno_worker.ts", import.meta.url).href, {
   *     type: "module",
   *     deno: {
   *       permissions: {
   *         read: true,
   *       },
   *     },
   *   }
   * );
   * ```
   */
  deno?: {
    /** Set to `"none"` to disable all the permissions in the worker. */
    permissions?: Deno.PermissionOptions;
  };
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category WebSockets
 * @experimental
 */
interface WebSocketStreamOptions {
  protocols?: string[];
  signal?: AbortSignal;
  headers?: HeadersInit;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category WebSockets
 * @experimental
 */
interface WebSocketConnection {
  readable: ReadableStream<string | Uint8Array<ArrayBuffer>>;
  writable: WritableStream<string | Uint8Array<ArrayBufferLike>>;
  extensions: string;
  protocol: string;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category WebSockets
 * @experimental
 */
interface WebSocketCloseInfo {
  code?: number;
  reason?: string;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @tags allow-net
 * @category WebSockets
 * @experimental
 */
interface WebSocketStream {
  url: string;
  opened: Promise<WebSocketConnection>;
  closed: Promise<WebSocketCloseInfo>;
  close(closeInfo?: WebSocketCloseInfo): void;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @tags allow-net
 * @category WebSockets
 * @experimental
 */
declare var WebSocketStream: {
  readonly prototype: WebSocketStream;
  new (url: string, options?: WebSocketStreamOptions): WebSocketStream;
};

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @tags allow-net
 * @category WebSockets
 * @experimental
 */
interface WebSocketError extends DOMException {
  readonly closeCode: number;
  readonly reason: string;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @tags allow-net
 * @category WebSockets
 * @experimental
 */
declare var WebSocketError: {
  readonly prototype: WebSocketError;
  new (message?: string, init?: WebSocketCloseInfo): WebSocketError;
};

// Adapted from `tc39/proposal-temporal`: https://github.com/tc39/proposal-temporal/blob/main/polyfill/index.d.ts

/**
 * [Specification](https://tc39.es/proposal-temporal/docs/index.html)
 *
 * @category Temporal
 * @experimental
 */
declare namespace Temporal {
  /**
   * @category Temporal
   * @experimental
   */
  export type ComparisonResult = -1 | 0 | 1;
  /**
   * @category Temporal
   * @experimental
   */
  export type RoundingMode =
    | "ceil"
    | "floor"
    | "expand"
    | "trunc"
    | "halfCeil"
    | "halfFloor"
    | "halfExpand"
    | "halfTrunc"
    | "halfEven";

  /**
   * Options for assigning fields using `with()` or entire objects with
   * `from()`.
   *
   * @category Temporal
   * @experimental
   */
  export type AssignmentOptions = {
    /**
     * How to deal with out-of-range values
     *
     * - In `'constrain'` mode, out-of-range values are clamped to the nearest
     *   in-range value.
     * - In `'reject'` mode, out-of-range values will cause the function to
     *   throw a RangeError.
     *
     * The default is `'constrain'`.
     */
    overflow?: "constrain" | "reject";
  };

  /**
   * Options for assigning fields using `Duration.prototype.with()` or entire
   * objects with `Duration.from()`, and for arithmetic with
   * `Duration.prototype.add()` and `Duration.prototype.subtract()`.
   *
   * @category Temporal
   * @experimental
   */
  export type DurationOptions = {
    /**
     * How to deal with out-of-range values
     *
     * - In `'constrain'` mode, out-of-range values are clamped to the nearest
     *   in-range value.
     * - In `'balance'` mode, out-of-range values are resolved by balancing them
     *   with the next highest unit.
     *
     * The default is `'constrain'`.
     */
    overflow?: "constrain" | "balance";
  };

  /**
   * Options for conversions of `Temporal.PlainDateTime` to `Temporal.Instant`
   *
   * @category Temporal
   * @experimental
   */
  export type ToInstantOptions = {
    /**
     * Controls handling of invalid or ambiguous times caused by time zone
     * offset changes like Daylight Saving time (DST) transitions.
     *
     * This option is only relevant if a `DateTime` value does not exist in the
     * destination time zone (e.g. near "Spring Forward" DST transitions), or
     * exists more than once (e.g. near "Fall Back" DST transitions).
     *
     * In case of ambiguous or nonexistent times, this option controls what
     * exact time to return:
     * - `'compatible'`: Equivalent to `'earlier'` for backward transitions like
     *   the start of DST in the Spring, and `'later'` for forward transitions
     *   like the end of DST in the Fall. This matches the behavior of legacy
     *   `Date`, of libraries like moment.js, Luxon, or date-fns, and of
     *   cross-platform standards like [RFC 5545
     *   (iCalendar)](https://tools.ietf.org/html/rfc5545).
     * - `'earlier'`: The earlier time of two possible times
     * - `'later'`: The later of two possible times
     * - `'reject'`: Throw a RangeError instead
     *
     * The default is `'compatible'`.
     */
    disambiguation?: "compatible" | "earlier" | "later" | "reject";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type OffsetDisambiguationOptions = {
    /**
     * Time zone definitions can change. If an application stores data about
     * events in the future, then stored data about future events may become
     * ambiguous, for example if a country permanently abolishes DST. The
     * `offset` option controls this unusual case.
     *
     * - `'use'` always uses the offset (if it's provided) to calculate the
     *   instant. This ensures that the result will match the instant that was
     *   originally stored, even if local clock time is different.
     * - `'prefer'` uses the offset if it's valid for the date/time in this time
     *   zone, but if it's not valid then the time zone will be used as a
     *   fallback to calculate the instant.
     * - `'ignore'` will disregard any provided offset. Instead, the time zone
     *    and date/time value are used to calculate the instant. This will keep
     *    local clock time unchanged but may result in a different real-world
     *    instant.
     * - `'reject'` acts like `'prefer'`, except it will throw a RangeError if
     *   the offset is not valid for the given time zone identifier and
     *   date/time value.
     *
     * If the ISO string ends in 'Z' then this option is ignored because there
     * is no possibility of ambiguity.
     *
     * If a time zone offset is not present in the input, then this option is
     * ignored because the time zone will always be used to calculate the
     * offset.
     *
     * If the offset is not used, and if the date/time and time zone don't
     * uniquely identify a single instant, then the `disambiguation` option will
     * be used to choose the correct instant. However, if the offset is used
     * then the `disambiguation` option will be ignored.
     */
    offset?: "use" | "prefer" | "ignore" | "reject";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type ZonedDateTimeAssignmentOptions = Partial<
    AssignmentOptions & ToInstantOptions & OffsetDisambiguationOptions
  >;

  /**
   * Options for arithmetic operations like `add()` and `subtract()`
   *
   * @category Temporal
   * @experimental
   */
  export type ArithmeticOptions = {
    /**
     * Controls handling of out-of-range arithmetic results.
     *
     * If a result is out of range, then `'constrain'` will clamp the result to
     * the allowed range, while `'reject'` will throw a RangeError.
     *
     * The default is `'constrain'`.
     */
    overflow?: "constrain" | "reject";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type DateUnit = "year" | "month" | "week" | "day";
  /**
   * @category Temporal
   * @experimental
   */
  export type TimeUnit =
    | "hour"
    | "minute"
    | "second"
    | "millisecond"
    | "microsecond"
    | "nanosecond";
  /**
   * @category Temporal
   * @experimental
   */
  export type DateTimeUnit = DateUnit | TimeUnit;

  /**
   * When the name of a unit is provided to a Temporal API as a string, it is
   * usually singular, e.g. 'day' or 'hour'. But plural unit names like 'days'
   * or 'hours' are also accepted.
   *
   * @category Temporal
   * @experimental
   */
  export type PluralUnit<T extends DateTimeUnit> = {
    year: "years";
    month: "months";
    week: "weeks";
    day: "days";
    hour: "hours";
    minute: "minutes";
    second: "seconds";
    millisecond: "milliseconds";
    microsecond: "microseconds";
    nanosecond: "nanoseconds";
  }[T];

  /**
   * @category Temporal
   * @experimental
   */
  export type LargestUnit<T extends DateTimeUnit> = "auto" | T | PluralUnit<T>;
  /**
   * @category Temporal
   * @experimental
   */
  export type SmallestUnit<T extends DateTimeUnit> = T | PluralUnit<T>;
  /**
   * @category Temporal
   * @experimental
   */
  export type TotalUnit<T extends DateTimeUnit> = T | PluralUnit<T>;

  /**
   * Options for outputting precision in toString() on types with seconds
   *
   * @category Temporal
   * @experimental
   */
  export type ToStringPrecisionOptions = {
    fractionalSecondDigits?: "auto" | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9;
    smallestUnit?: SmallestUnit<
      "minute" | "second" | "millisecond" | "microsecond" | "nanosecond"
    >;

    /**
     * Controls how rounding is performed:
     * - `halfExpand`: Round to the nearest of the values allowed by
     *   `roundingIncrement` and `smallestUnit`. When there is a tie, round up.
     *   This mode is the default.
     * - `ceil`: Always round up, towards the end of time.
     * - `trunc`: Always round down, towards the beginning of time.
     * - `floor`: Also round down, towards the beginning of time. This mode acts
     *   the same as `trunc`, but it's included for consistency with
     *   `Temporal.Duration.round()` where negative values are allowed and
     *   `trunc` rounds towards zero, unlike `floor` which rounds towards
     *   negative infinity which is usually unexpected. For this reason, `trunc`
     *   is recommended for most use cases.
     */
    roundingMode?: RoundingMode;
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type ShowCalendarOption = {
    calendarName?: "auto" | "always" | "never" | "critical";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type CalendarTypeToStringOptions = Partial<
    ToStringPrecisionOptions & ShowCalendarOption
  >;

  /**
   * @category Temporal
   * @experimental
   */
  export type ZonedDateTimeToStringOptions = Partial<
    CalendarTypeToStringOptions & {
      timeZoneName?: "auto" | "never" | "critical";
      offset?: "auto" | "never";
    }
  >;

  /**
   * @category Temporal
   * @experimental
   */
  export type InstantToStringOptions = Partial<
    ToStringPrecisionOptions & {
      timeZone: TimeZoneLike;
    }
  >;

  /**
   * Options to control the result of `until()` and `since()` methods in
   * `Temporal` types.
   *
   * @category Temporal
   * @experimental
   */
  export interface DifferenceOptions<T extends DateTimeUnit> {
    /**
     * The unit to round to. For example, to round to the nearest minute, use
     * `smallestUnit: 'minute'`. This property is optional for `until()` and
     * `since()`, because those methods default behavior is not to round.
     * However, the same property is required for `round()`.
     */
    smallestUnit?: SmallestUnit<T>;

    /**
     * The largest unit to allow in the resulting `Temporal.Duration` object.
     *
     * Larger units will be "balanced" into smaller units. For example, if
     * `largestUnit` is `'minute'` then a two-hour duration will be output as a
     * 120-minute duration.
     *
     * Valid values may include `'year'`, `'month'`, `'week'`, `'day'`,
     * `'hour'`, `'minute'`, `'second'`, `'millisecond'`, `'microsecond'`,
     * `'nanosecond'` and `'auto'`, although some types may throw an exception
     * if a value is used that would produce an invalid result. For example,
     * `hours` is not accepted by `Temporal.PlainDate.prototype.since()`.
     *
     * The default is always `'auto'`, though the meaning of this depends on the
     * type being used.
     */
    largestUnit?: LargestUnit<T>;

    /**
     * Allows rounding to an integer number of units. For example, to round to
     * increments of a half hour, use `{ smallestUnit: 'minute',
     * roundingIncrement: 30 }`.
     */
    roundingIncrement?: number;

    /**
     * Controls how rounding is performed:
     * - `halfExpand`: Round to the nearest of the values allowed by
     *   `roundingIncrement` and `smallestUnit`. When there is a tie, round away
     *   from zero like `ceil` for positive durations and like `floor` for
     *   negative durations.
     * - `ceil`: Always round up, towards the end of time.
     * - `trunc`: Always round down, towards the beginning of time. This mode is
     *   the default.
     * - `floor`: Also round down, towards the beginning of time. This mode acts the
     *   same as `trunc`, but it's included for consistency with
     *   `Temporal.Duration.round()` where negative values are allowed and
     *   `trunc` rounds towards zero, unlike `floor` which rounds towards
     *   negative infinity which is usually unexpected. For this reason, `trunc`
     *   is recommended for most use cases.
     */
    roundingMode?: RoundingMode;
  }

  /**
   * `round` methods take one required parameter. If a string is provided, the
   * resulting `Temporal.Duration` object will be rounded to that unit. If an
   * object is provided, its `smallestUnit` property is required while other
   * properties are optional. A string is treated the same as an object whose
   * `smallestUnit` property value is that string.
   *
   * @category Temporal
   * @experimental
   */
  export type RoundTo<T extends DateTimeUnit> =
    | SmallestUnit<T>
    | {
      /**
       * The unit to round to. For example, to round to the nearest minute,
       * use `smallestUnit: 'minute'`. This option is required. Note that the
       * same-named property is optional when passed to `until` or `since`
       * methods, because those methods do no rounding by default.
       */
      smallestUnit: SmallestUnit<T>;

      /**
       * Allows rounding to an integer number of units. For example, to round to
       * increments of a half hour, use `{ smallestUnit: 'minute',
       * roundingIncrement: 30 }`.
       */
      roundingIncrement?: number;

      /**
       * Controls how rounding is performed:
       * - `halfExpand`: Round to the nearest of the values allowed by
       *   `roundingIncrement` and `smallestUnit`. When there is a tie, round up.
       *   This mode is the default.
       * - `ceil`: Always round up, towards the end of time.
       * - `trunc`: Always round down, towards the beginning of time.
       * - `floor`: Also round down, towards the beginning of time. This mode acts
       *   the same as `trunc`, but it's included for consistency with
       *   `Temporal.Duration.round()` where negative values are allowed and
       *   `trunc` rounds towards zero, unlike `floor` which rounds towards
       *   negative infinity which is usually unexpected. For this reason, `trunc`
       *   is recommended for most use cases.
       */
      roundingMode?: RoundingMode;
    };

  /**
   * The `round` method of the `Temporal.Duration` accepts one required
   * parameter. If a string is provided, the resulting `Temporal.Duration`
   * object will be rounded to that unit. If an object is provided, the
   * `smallestUnit` and/or `largestUnit` property is required, while other
   * properties are optional. A string parameter is treated the same as an
   * object whose `smallestUnit` property value is that string.
   *
   * @category Temporal
   * @experimental
   */
  export type DurationRoundTo =
    | SmallestUnit<DateTimeUnit>
    | (
      & (
        | {
          /**
           * The unit to round to. For example, to round to the nearest
           * minute, use `smallestUnit: 'minute'`. This property is normally
           * required, but is optional if `largestUnit` is provided and not
           * undefined.
           */
          smallestUnit: SmallestUnit<DateTimeUnit>;

          /**
           * The largest unit to allow in the resulting `Temporal.Duration`
           * object.
           *
           * Larger units will be "balanced" into smaller units. For example,
           * if `largestUnit` is `'minute'` then a two-hour duration will be
           * output as a 120-minute duration.
           *
           * Valid values include `'year'`, `'month'`, `'week'`, `'day'`,
           * `'hour'`, `'minute'`, `'second'`, `'millisecond'`,
           * `'microsecond'`, `'nanosecond'` and `'auto'`.
           *
           * The default is `'auto'`, which means "the largest nonzero unit in
           * the input duration". This default prevents expanding durations to
           * larger units unless the caller opts into this behavior.
           *
           * If `smallestUnit` is larger, then `smallestUnit` will be used as
           * `largestUnit`, superseding a caller-supplied or default value.
           */
          largestUnit?: LargestUnit<DateTimeUnit>;
        }
        | {
          /**
           * The unit to round to. For example, to round to the nearest
           * minute, use `smallestUnit: 'minute'`. This property is normally
           * required, but is optional if `largestUnit` is provided and not
           * undefined.
           */
          smallestUnit?: SmallestUnit<DateTimeUnit>;

          /**
           * The largest unit to allow in the resulting `Temporal.Duration`
           * object.
           *
           * Larger units will be "balanced" into smaller units. For example,
           * if `largestUnit` is `'minute'` then a two-hour duration will be
           * output as a 120-minute duration.
           *
           * Valid values include `'year'`, `'month'`, `'week'`, `'day'`,
           * `'hour'`, `'minute'`, `'second'`, `'millisecond'`,
           * `'microsecond'`, `'nanosecond'` and `'auto'`.
           *
           * The default is `'auto'`, which means "the largest nonzero unit in
           * the input duration". This default prevents expanding durations to
           * larger units unless the caller opts into this behavior.
           *
           * If `smallestUnit` is larger, then `smallestUnit` will be used as
           * `largestUnit`, superseding a caller-supplied or default value.
           */
          largestUnit: LargestUnit<DateTimeUnit>;
        }
      )
      & {
        /**
         * Allows rounding to an integer number of units. For example, to round
         * to increments of a half hour, use `{ smallestUnit: 'minute',
         * roundingIncrement: 30 }`.
         */
        roundingIncrement?: number;

        /**
         * Controls how rounding is performed:
         * - `halfExpand`: Round to the nearest of the values allowed by
         *   `roundingIncrement` and `smallestUnit`. When there is a tie, round
         *   away from zero like `ceil` for positive durations and like `floor`
         *   for negative durations. This mode is the default.
         * - `ceil`: Always round towards positive infinity. For negative
         *   durations this option will decrease the absolute value of the
         *   duration which may be unexpected. To round away from zero, use
         *   `ceil` for positive durations and `floor` for negative durations.
         * - `trunc`: Always round down towards zero.
         * - `floor`: Always round towards negative infinity. This mode acts the
         *   same as `trunc` for positive durations but for negative durations
         *   it will increase the absolute value of the result which may be
         *   unexpected. For this reason, `trunc` is recommended for most "round
         *   down" use cases.
         */
        roundingMode?: RoundingMode;

        /**
         * The starting point to use for rounding and conversions when
         * variable-length units (years, months, weeks depending on the
         * calendar) are involved. This option is required if any of the
         * following are true:
         * - `unit` is `'week'` or larger units
         * - `this` has a nonzero value for `weeks` or larger units
         *
         * This value must be either a `Temporal.PlainDateTime`, a
         * `Temporal.ZonedDateTime`, or a string or object value that can be
         * passed to `from()` of those types. Examples:
         * - `'2020-01-01T00:00-08:00[America/Los_Angeles]'`
         * - `'2020-01-01'`
         * - `Temporal.PlainDate.from('2020-01-01')`
         *
         * `Temporal.ZonedDateTime` will be tried first because it's more
         * specific, with `Temporal.PlainDateTime` as a fallback.
         *
         * If the value resolves to a `Temporal.ZonedDateTime`, then operation
         * will adjust for DST and other time zone transitions. Otherwise
         * (including if this option is omitted), then the operation will ignore
         * time zone transitions and all days will be assumed to be 24 hours
         * long.
         */
        relativeTo?:
          | Temporal.PlainDateTime
          | Temporal.ZonedDateTime
          | PlainDateTimeLike
          | ZonedDateTimeLike
          | string;
      }
    );

  /**
   * Options to control behavior of `Duration.prototype.total()`
   *
   * @category Temporal
   * @experimental
   */
  export type DurationTotalOf =
    | TotalUnit<DateTimeUnit>
    | {
      /**
       * The unit to convert the duration to. This option is required.
       */
      unit: TotalUnit<DateTimeUnit>;

      /**
       * The starting point to use when variable-length units (years, months,
       * weeks depending on the calendar) are involved. This option is required if
       * any of the following are true:
       * - `unit` is `'week'` or larger units
       * - `this` has a nonzero value for `weeks` or larger units
       *
       * This value must be either a `Temporal.PlainDateTime`, a
       * `Temporal.ZonedDateTime`, or a string or object value that can be passed
       * to `from()` of those types. Examples:
       * - `'2020-01-01T00:00-08:00[America/Los_Angeles]'`
       * - `'2020-01-01'`
       * - `Temporal.PlainDate.from('2020-01-01')`
       *
       * `Temporal.ZonedDateTime` will be tried first because it's more
       * specific, with `Temporal.PlainDateTime` as a fallback.
       *
       * If the value resolves to a `Temporal.ZonedDateTime`, then operation will
       * adjust for DST and other time zone transitions. Otherwise (including if
       * this option is omitted), then the operation will ignore time zone
       * transitions and all days will be assumed to be 24 hours long.
       */
      relativeTo?:
        | Temporal.ZonedDateTime
        | Temporal.PlainDateTime
        | ZonedDateTimeLike
        | PlainDateTimeLike
        | string;
    };

  /**
   * Options to control behavior of `Duration.compare()`
   *
   * @category Temporal
   * @experimental
   */
  export interface DurationArithmeticOptions {
    /**
     * The starting point to use when variable-length units (years, months,
     * weeks depending on the calendar) are involved. This option is required if
     * either of the durations has a nonzero value for `weeks` or larger units.
     *
     * This value must be either a `Temporal.PlainDateTime`, a
     * `Temporal.ZonedDateTime`, or a string or object value that can be passed
     * to `from()` of those types. Examples:
     * - `'2020-01-01T00:00-08:00[America/Los_Angeles]'`
     * - `'2020-01-01'`
     * - `Temporal.PlainDate.from('2020-01-01')`
     *
     * `Temporal.ZonedDateTime` will be tried first because it's more
     * specific, with `Temporal.PlainDateTime` as a fallback.
     *
     * If the value resolves to a `Temporal.ZonedDateTime`, then operation will
     * adjust for DST and other time zone transitions. Otherwise (including if
     * this option is omitted), then the operation will ignore time zone
     * transitions and all days will be assumed to be 24 hours long.
     */
    relativeTo?:
      | Temporal.ZonedDateTime
      | Temporal.PlainDateTime
      | ZonedDateTimeLike
      | PlainDateTimeLike
      | string;
  }

  /**
   * Options to control behaviour of `ZonedDateTime.prototype.getTimeZoneTransition()`
   *
   * @category Temporal
   * @experimental
   */
  export type TransitionDirection = "next" | "previous" | {
    direction: "next" | "previous";
  };

  /**
   * @category Temporal
   * @experimental
   */
  export type DurationLike = {
    years?: number;
    months?: number;
    weeks?: number;
    days?: number;
    hours?: number;
    minutes?: number;
    seconds?: number;
    milliseconds?: number;
    microseconds?: number;
    nanoseconds?: number;
  };

  /**
   * A `Temporal.Duration` represents an immutable duration of time which can be
   * used in date/time arithmetic.
   *
   * See https://tc39.es/proposal-temporal/docs/duration.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class Duration {
    static from(
      item: Temporal.Duration | DurationLike | string,
    ): Temporal.Duration;
    static compare(
      one: Temporal.Duration | DurationLike | string,
      two: Temporal.Duration | DurationLike | string,
      options?: DurationArithmeticOptions,
    ): ComparisonResult;
    constructor(
      years?: number,
      months?: number,
      weeks?: number,
      days?: number,
      hours?: number,
      minutes?: number,
      seconds?: number,
      milliseconds?: number,
      microseconds?: number,
      nanoseconds?: number,
    );
    readonly sign: -1 | 0 | 1;
    readonly blank: boolean;
    readonly years: number;
    readonly months: number;
    readonly weeks: number;
    readonly days: number;
    readonly hours: number;
    readonly minutes: number;
    readonly seconds: number;
    readonly milliseconds: number;
    readonly microseconds: number;
    readonly nanoseconds: number;
    negated(): Temporal.Duration;
    abs(): Temporal.Duration;
    with(durationLike: DurationLike): Temporal.Duration;
    add(
      other: Temporal.Duration | DurationLike | string,
      options?: DurationArithmeticOptions,
    ): Temporal.Duration;
    subtract(
      other: Temporal.Duration | DurationLike | string,
      options?: DurationArithmeticOptions,
    ): Temporal.Duration;
    round(roundTo: DurationRoundTo): Temporal.Duration;
    total(totalOf: DurationTotalOf): number;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ToStringPrecisionOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.Duration";
  }

  /**
   * A `Temporal.Instant` is an exact point in time, with a precision in
   * nanoseconds. No time zone or calendar information is present. Therefore,
   * `Temporal.Instant` has no concept of days, months, or even hours.
   *
   * For convenience of interoperability, it internally uses nanoseconds since
   * the {@link https://en.wikipedia.org/wiki/Unix_time|Unix epoch} (midnight
   * UTC on January 1, 1970). However, a `Temporal.Instant` can be created from
   * any of several expressions that refer to a single point in time, including
   * an {@link https://en.wikipedia.org/wiki/ISO_8601|ISO 8601 string} with a
   * time zone offset such as '2020-01-23T17:04:36.491865121-08:00'.
   *
   * See https://tc39.es/proposal-temporal/docs/instant.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class Instant {
    static fromEpochMilliseconds(epochMilliseconds: number): Temporal.Instant;
    static fromEpochNanoseconds(epochNanoseconds: bigint): Temporal.Instant;
    static from(item: Temporal.Instant | string): Temporal.Instant;
    static compare(
      one: Temporal.Instant | string,
      two: Temporal.Instant | string,
    ): ComparisonResult;
    constructor(epochNanoseconds: bigint);
    readonly epochMilliseconds: number;
    readonly epochNanoseconds: bigint;
    equals(other: Temporal.Instant | string): boolean;
    add(
      durationLike:
        | Omit<
          Temporal.Duration | DurationLike,
          "years" | "months" | "weeks" | "days"
        >
        | string,
    ): Temporal.Instant;
    subtract(
      durationLike:
        | Omit<
          Temporal.Duration | DurationLike,
          "years" | "months" | "weeks" | "days"
        >
        | string,
    ): Temporal.Instant;
    until(
      other: Temporal.Instant | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.Instant | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Instant;
    toZonedDateTimeISO(tzLike: TimeZoneLike): Temporal.ZonedDateTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: InstantToStringOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.Instant";
  }

  /**
   * Any of these types can be passed to Temporal methods instead of a calendar ID.
   *
   * @category Temporal
   * @experimental
   */
  export type CalendarLike =
    | string
    | ZonedDateTime
    | PlainDateTime
    | PlainDate
    | PlainYearMonth
    | PlainMonthDay;

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainDateLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainDate` represents a calendar date. "Calendar date" refers to the
   * concept of a date as expressed in everyday usage, independent of any time
   * zone. For example, it could be used to represent an event on a calendar
   * which happens during the whole day no matter which time zone it's happening
   * in.
   *
   * See https://tc39.es/proposal-temporal/docs/date.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainDate {
    static from(
      item: Temporal.PlainDate | PlainDateLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainDate;
    static compare(
      one: Temporal.PlainDate | PlainDateLike | string,
      two: Temporal.PlainDate | PlainDateLike | string,
    ): ComparisonResult;
    constructor(
      isoYear: number,
      isoMonth: number,
      isoDay: number,
      calendar?: string,
    );
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly day: number;
    readonly calendarId: string;
    readonly dayOfWeek: number;
    readonly dayOfYear: number;
    readonly weekOfYear: number | undefined;
    readonly yearOfWeek: number | undefined;
    readonly daysInWeek: number;
    readonly daysInYear: number;
    readonly daysInMonth: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    equals(other: Temporal.PlainDate | PlainDateLike | string): boolean;
    with(
      dateLike: PlainDateLike,
      options?: AssignmentOptions,
    ): Temporal.PlainDate;
    withCalendar(calendar: CalendarLike): Temporal.PlainDate;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDate;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDate;
    until(
      other: Temporal.PlainDate | PlainDateLike | string,
      options?: DifferenceOptions<"year" | "month" | "week" | "day">,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainDate | PlainDateLike | string,
      options?: DifferenceOptions<"year" | "month" | "week" | "day">,
    ): Temporal.Duration;
    toPlainDateTime(
      temporalTime?: Temporal.PlainTime | PlainTimeLike | string,
    ): Temporal.PlainDateTime;
    toZonedDateTime(
      timeZoneAndTime:
        | string
        | {
          timeZone: TimeZoneLike;
          plainTime?: Temporal.PlainTime | PlainTimeLike | string;
        },
    ): Temporal.ZonedDateTime;
    toPlainYearMonth(): Temporal.PlainYearMonth;
    toPlainMonthDay(): Temporal.PlainMonthDay;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ShowCalendarOption): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainDate";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainDateTimeLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    hour?: number;
    minute?: number;
    second?: number;
    millisecond?: number;
    microsecond?: number;
    nanosecond?: number;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainDateTime` represents a calendar date and wall-clock time, with
   * a precision in nanoseconds, and without any time zone. Of the Temporal
   * classes carrying human-readable time information, it is the most general
   * and complete one. `Temporal.PlainDate`, `Temporal.PlainTime`, `Temporal.PlainYearMonth`,
   * and `Temporal.PlainMonthDay` all carry less information and should be used when
   * complete information is not required.
   *
   * See https://tc39.es/proposal-temporal/docs/datetime.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainDateTime {
    static from(
      item: Temporal.PlainDateTime | PlainDateTimeLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainDateTime;
    static compare(
      one: Temporal.PlainDateTime | PlainDateTimeLike | string,
      two: Temporal.PlainDateTime | PlainDateTimeLike | string,
    ): ComparisonResult;
    constructor(
      isoYear: number,
      isoMonth: number,
      isoDay: number,
      hour?: number,
      minute?: number,
      second?: number,
      millisecond?: number,
      microsecond?: number,
      nanosecond?: number,
      calendar?: string,
    );
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly day: number;
    readonly hour: number;
    readonly minute: number;
    readonly second: number;
    readonly millisecond: number;
    readonly microsecond: number;
    readonly nanosecond: number;
    readonly calendarId: string;
    readonly dayOfWeek: number;
    readonly dayOfYear: number;
    readonly weekOfYear: number | undefined;
    readonly yearOfWeek: number | undefined;
    readonly daysInWeek: number;
    readonly daysInYear: number;
    readonly daysInMonth: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    equals(other: Temporal.PlainDateTime | PlainDateTimeLike | string): boolean;
    with(
      dateTimeLike: PlainDateTimeLike,
      options?: AssignmentOptions,
    ): Temporal.PlainDateTime;
    withPlainTime(
      timeLike?: Temporal.PlainTime | PlainTimeLike | string,
    ): Temporal.PlainDateTime;
    withCalendar(calendar: CalendarLike): Temporal.PlainDateTime;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDateTime;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainDateTime;
    until(
      other: Temporal.PlainDateTime | PlainDateTimeLike | string,
      options?: DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainDateTime | PlainDateTimeLike | string,
      options?: DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.PlainDateTime;
    toZonedDateTime(
      tzLike: TimeZoneLike,
      options?: ToInstantOptions,
    ): Temporal.ZonedDateTime;
    toPlainDate(): Temporal.PlainDate;
    toPlainTime(): Temporal.PlainTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: CalendarTypeToStringOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainDateTime";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainMonthDayLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainMonthDay` represents a particular day on the calendar, but
   * without a year. For example, it could be used to represent a yearly
   * recurring event, like "Bastille Day is on the 14th of July."
   *
   * See https://tc39.es/proposal-temporal/docs/monthday.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainMonthDay {
    static from(
      item: Temporal.PlainMonthDay | PlainMonthDayLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainMonthDay;
    constructor(
      isoMonth: number,
      isoDay: number,
      calendar?: string,
      referenceISOYear?: number,
    );
    readonly monthCode: string;
    readonly day: number;
    readonly calendarId: string;
    equals(other: Temporal.PlainMonthDay | PlainMonthDayLike | string): boolean;
    with(
      monthDayLike: PlainMonthDayLike,
      options?: AssignmentOptions,
    ): Temporal.PlainMonthDay;
    toPlainDate(year: { year: number }): Temporal.PlainDate;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ShowCalendarOption): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainMonthDay";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainTimeLike = {
    hour?: number;
    minute?: number;
    second?: number;
    millisecond?: number;
    microsecond?: number;
    nanosecond?: number;
  };

  /**
   * A `Temporal.PlainTime` represents a wall-clock time, with a precision in
   * nanoseconds, and without any time zone. "Wall-clock time" refers to the
   * concept of a time as expressed in everyday usage — the time that you read
   * off the clock on the wall. For example, it could be used to represent an
   * event that happens daily at a certain time, no matter what time zone.
   *
   * `Temporal.PlainTime` refers to a time with no associated calendar date; if you
   * need to refer to a specific time on a specific day, use
   * `Temporal.PlainDateTime`. A `Temporal.PlainTime` can be converted into a
   * `Temporal.PlainDateTime` by combining it with a `Temporal.PlainDate` using the
   * `toPlainDateTime()` method.
   *
   * See https://tc39.es/proposal-temporal/docs/plaintime.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainTime {
    static from(
      item: Temporal.PlainTime | PlainTimeLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainTime;
    static compare(
      one: Temporal.PlainTime | PlainTimeLike | string,
      two: Temporal.PlainTime | PlainTimeLike | string,
    ): ComparisonResult;
    constructor(
      hour?: number,
      minute?: number,
      second?: number,
      millisecond?: number,
      microsecond?: number,
      nanosecond?: number,
    );
    readonly hour: number;
    readonly minute: number;
    readonly second: number;
    readonly millisecond: number;
    readonly microsecond: number;
    readonly nanosecond: number;
    equals(other: Temporal.PlainTime | PlainTimeLike | string): boolean;
    with(
      timeLike: Temporal.PlainTime | PlainTimeLike,
      options?: AssignmentOptions,
    ): Temporal.PlainTime;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainTime;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainTime;
    until(
      other: Temporal.PlainTime | PlainTimeLike | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainTime | PlainTimeLike | string,
      options?: DifferenceOptions<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.PlainTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ToStringPrecisionOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainTime";
  }

  /**
   * Any of these types can be passed to Temporal methods instead of a time zone ID.
   *
   * @category Temporal
   * @experimental
   */
  export type TimeZoneLike = string | ZonedDateTime;

  /**
   * @category Temporal
   * @experimental
   */
  export type PlainYearMonthLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    calendar?: CalendarLike;
  };

  /**
   * A `Temporal.PlainYearMonth` represents a particular month on the calendar. For
   * example, it could be used to represent a particular instance of a monthly
   * recurring event, like "the June 2019 meeting".
   *
   * See https://tc39.es/proposal-temporal/docs/yearmonth.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export class PlainYearMonth {
    static from(
      item: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      options?: AssignmentOptions,
    ): Temporal.PlainYearMonth;
    static compare(
      one: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      two: Temporal.PlainYearMonth | PlainYearMonthLike | string,
    ): ComparisonResult;
    constructor(
      isoYear: number,
      isoMonth: number,
      calendar?: string,
      referenceISODay?: number,
    );
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly calendarId: string;
    readonly daysInMonth: number;
    readonly daysInYear: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    equals(
      other: Temporal.PlainYearMonth | PlainYearMonthLike | string,
    ): boolean;
    with(
      yearMonthLike: PlainYearMonthLike,
      options?: AssignmentOptions,
    ): Temporal.PlainYearMonth;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainYearMonth;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.PlainYearMonth;
    until(
      other: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      options?: DifferenceOptions<"year" | "month">,
    ): Temporal.Duration;
    since(
      other: Temporal.PlainYearMonth | PlainYearMonthLike | string,
      options?: DifferenceOptions<"year" | "month">,
    ): Temporal.Duration;
    toPlainDate(day: { day: number }): Temporal.PlainDate;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ShowCalendarOption): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.PlainYearMonth";
  }

  /**
   * @category Temporal
   * @experimental
   */
  export type ZonedDateTimeLike = {
    era?: string | undefined;
    eraYear?: number | undefined;
    year?: number;
    month?: number;
    monthCode?: string;
    day?: number;
    hour?: number;
    minute?: number;
    second?: number;
    millisecond?: number;
    microsecond?: number;
    nanosecond?: number;
    offset?: string;
    timeZone?: TimeZoneLike;
    calendar?: CalendarLike;
  };

  /**
   * @category Temporal
   * @experimental
   */
  export class ZonedDateTime {
    static from(
      item: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      options?: ZonedDateTimeAssignmentOptions,
    ): ZonedDateTime;
    static compare(
      one: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      two: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
    ): ComparisonResult;
    constructor(epochNanoseconds: bigint, timeZone: string, calendar?: string);
    readonly era: string | undefined;
    readonly eraYear: number | undefined;
    readonly year: number;
    readonly month: number;
    readonly monthCode: string;
    readonly day: number;
    readonly hour: number;
    readonly minute: number;
    readonly second: number;
    readonly millisecond: number;
    readonly microsecond: number;
    readonly nanosecond: number;
    readonly timeZoneId: string;
    readonly calendarId: string;
    readonly dayOfWeek: number;
    readonly dayOfYear: number;
    readonly weekOfYear: number | undefined;
    readonly yearOfWeek: number | undefined;
    readonly hoursInDay: number;
    readonly daysInWeek: number;
    readonly daysInMonth: number;
    readonly daysInYear: number;
    readonly monthsInYear: number;
    readonly inLeapYear: boolean;
    readonly offsetNanoseconds: number;
    readonly offset: string;
    readonly epochMilliseconds: number;
    readonly epochNanoseconds: bigint;
    equals(other: Temporal.ZonedDateTime | ZonedDateTimeLike | string): boolean;
    with(
      zonedDateTimeLike: ZonedDateTimeLike,
      options?: ZonedDateTimeAssignmentOptions,
    ): Temporal.ZonedDateTime;
    withPlainTime(
      timeLike?: Temporal.PlainTime | PlainTimeLike | string,
    ): Temporal.ZonedDateTime;
    withCalendar(calendar: CalendarLike): Temporal.ZonedDateTime;
    withTimeZone(timeZone: TimeZoneLike): Temporal.ZonedDateTime;
    add(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.ZonedDateTime;
    subtract(
      durationLike: Temporal.Duration | DurationLike | string,
      options?: ArithmeticOptions,
    ): Temporal.ZonedDateTime;
    until(
      other: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      options?: Temporal.DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    since(
      other: Temporal.ZonedDateTime | ZonedDateTimeLike | string,
      options?: Temporal.DifferenceOptions<
        | "year"
        | "month"
        | "week"
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.Duration;
    round(
      roundTo: RoundTo<
        | "day"
        | "hour"
        | "minute"
        | "second"
        | "millisecond"
        | "microsecond"
        | "nanosecond"
      >,
    ): Temporal.ZonedDateTime;
    startOfDay(): Temporal.ZonedDateTime;
    getTimeZoneTransition(
      direction: TransitionDirection,
    ): Temporal.ZonedDateTime | null;
    toInstant(): Temporal.Instant;
    toPlainDateTime(): Temporal.PlainDateTime;
    toPlainDate(): Temporal.PlainDate;
    toPlainTime(): Temporal.PlainTime;
    toLocaleString(
      locales?: string | string[],
      options?: Intl.DateTimeFormatOptions,
    ): string;
    toJSON(): string;
    toString(options?: ZonedDateTimeToStringOptions): string;
    valueOf(): never;
    readonly [Symbol.toStringTag]: "Temporal.ZonedDateTime";
  }

  /**
   * The `Temporal.Now` object has several methods which give information about
   * the current date, time, and time zone.
   *
   * See https://tc39.es/proposal-temporal/docs/now.html for more details.
   *
   * @category Temporal
   * @experimental
   */
  export const Now: {
    /**
     * Get the exact system date and time as a `Temporal.Instant`.
     *
     * This method gets the current exact system time, without regard to
     * calendar or time zone. This is a good way to get a timestamp for an
     * event, for example. It works like the old-style JavaScript `Date.now()`,
     * but with nanosecond precision instead of milliseconds.
     *
     * Note that a `Temporal.Instant` doesn't know about time zones. For the
     * exact time in a specific time zone, use `Temporal.Now.zonedDateTimeISO`
     * or `Temporal.Now.zonedDateTime`.
     */
    instant: () => Temporal.Instant;

    /**
     * Get the current calendar date and clock time in a specific time zone,
     * using the ISO 8601 calendar.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    zonedDateTimeISO: (tzLike?: TimeZoneLike) => Temporal.ZonedDateTime;

    /**
     * Get the current date and clock time in a specific time zone, using the
     * ISO 8601 calendar.
     *
     * Note that the `Temporal.PlainDateTime` type does not persist the time zone,
     * but retaining the time zone is required for most time-zone-related use
     * cases. Therefore, it's usually recommended to use
     * `Temporal.Now.zonedDateTimeISO` instead of this function.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    plainDateTimeISO: (tzLike?: TimeZoneLike) => Temporal.PlainDateTime;

    /**
     * Get the current date in a specific time zone, using the ISO 8601
     * calendar.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    plainDateISO: (tzLike?: TimeZoneLike) => Temporal.PlainDate;

    /**
     * Get the current clock time in a specific time zone, using the ISO 8601 calendar.
     *
     * @param {TimeZoneLike} [tzLike] -
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone identifier}
     * string (e.g. `'Europe/London'`). If omitted, the environment's
     * current time zone will be used.
     */
    plainTimeISO: (tzLike?: TimeZoneLike) => Temporal.PlainTime;

    /**
     * Get the identifier of the environment's current time zone.
     *
     * This method gets the identifier of the current system time zone. This
     * will usually be a named
     * {@link https://en.wikipedia.org/wiki/List_of_tz_database_time_zones|IANA time zone}.
     */
    timeZoneId: () => string;

    readonly [Symbol.toStringTag]: "Temporal.Now";
  };
}

/**
 * @category Temporal
 * @experimental
 */
interface Date {
  toTemporalInstant(): Temporal.Instant;
}

/**
 * @category Intl
 * @experimental
 */
declare namespace Intl {
  /**
   * Types that can be formatted using Intl.DateTimeFormat methods.
   *
   * This type defines what values can be passed to Intl.DateTimeFormat methods
   * for internationalized date and time formatting. It includes standard Date objects
   * and Temporal API date/time types.
   *
   * @example
   * ```ts
   * // Using with Date object
   * const date = new Date();
   * const formatter = new Intl.DateTimeFormat('en-US');
   * console.log(formatter.format(date));
   *
   * // Using with Temporal types (when available)
   * const instant = Temporal.Now.instant();
   * console.log(formatter.format(instant));
   * ```
   *
   * @category Intl
   * @experimental
   */
  export type Formattable =
    | Date
    | Temporal.Instant
    | Temporal.ZonedDateTime
    | Temporal.PlainDate
    | Temporal.PlainTime
    | Temporal.PlainDateTime
    | Temporal.PlainYearMonth
    | Temporal.PlainMonthDay;

  /**
   * Represents a part of a formatted date range produced by Intl.DateTimeFormat.formatRange().
   *
   * Each part has a type and value that describes its role within the formatted string.
   * The source property indicates whether the part comes from the start date, end date, or
   * is shared between them.
   *
   * @example
   * ```ts
   * const dtf = new Intl.DateTimeFormat('en', {
   *   dateStyle: 'long',
   *   timeStyle: 'short'
   * });
   * const parts = dtf.formatRangeToParts(
   *   new Date(2023, 0, 1, 12, 0),
   *   new Date(2023, 0, 3, 15, 30)
   * );
   * console.log(parts);
   * // Parts might include elements like:
   * // { type: 'month', value: 'January', source: 'startRange' }
   * // { type: 'day', value: '1', source: 'startRange' }
   * // { type: 'literal', value: ' - ', source: 'shared' }
   * // { type: 'day', value: '3', source: 'endRange' }
   * // ...
   * ```
   *
   * @category Intl
   * @experimental
   */
  export interface DateTimeFormatRangePart {
    /**
     * The type of date or time component this part represents.
     * Possible values: 'day', 'dayPeriod', 'era', 'fractionalSecond', 'hour',
     * 'literal', 'minute', 'month', 'relatedYear', 'second', 'timeZoneName',
     * 'weekday', 'year', etc.
     */
    type: string;

    /** The string value of this part. */
    value: string;

    /**
     * Indicates which date in the range this part comes from.
     * - 'startRange': The part is from the start date
     * - 'endRange': The part is from the end date
     * - 'shared': The part is shared between both dates (like separators)
     */
    source: "shared" | "startRange" | "endRange";
  }

  /**
   * @category Intl
   * @experimental
   */
  export interface DateTimeFormat {
    /**
     * Format a date into a string according to the locale and formatting
     * options of this `Intl.DateTimeFormat` object.
     *
     * @example
     * ```ts
     * const formatter = new Intl.DateTimeFormat('en-US', { dateStyle: 'full' });
     * const date = new Date(2023, 0, 1);
     * console.log(formatter.format(date)); // Output: "Sunday, January 1, 2023"
     * ```
     */
    format(date?: Formattable | number): string;

    /**
     * Allow locale-aware formatting of strings produced by
     * `Intl.DateTimeFormat` formatters.
     *
     * @example
     * ```ts
     * const formatter = new Intl.DateTimeFormat('en-US', { dateStyle: 'full' });
     * const date = new Date(2023, 0, 1);
     * console.log(formatter.format(date)); // Output: "Sunday, January 1, 2023"
     * ```
     */
    formatToParts(
      date?: Formattable | number,
    ): globalThis.Intl.DateTimeFormatPart[];

    /**
     * Format a date range in the most concise way based on the locale and
     * options provided when instantiating this `Intl.DateTimeFormat` object.
     *
     * @param startDate The start date of the range to format.
     * @param endDate The start date of the range to format. Must be the same
     * type as `startRange`.
     *
     * @example
     * ```ts
     * const formatter = new Intl.DateTimeFormat('en-US', { dateStyle: 'long' });
     * const startDate = new Date(2023, 0, 1);
     * const endDate = new Date(2023, 0, 5);
     * console.log(formatter.formatRange(startDate, endDate));
     * // Output: "January 1 – 5, 2023"
     * ```
     */
    formatRange<T extends Formattable>(startDate: T, endDate: T): string;
    formatRange(startDate: Date | number, endDate: Date | number): string;

    /**
     * Allow locale-aware formatting of tokens representing each part of the
     * formatted date range produced by `Intl.DateTimeFormat` formatters.
     *
     * @param startDate The start date of the range to format.
     * @param endDate The start date of the range to format. Must be the same
     * type as `startRange`.
     *
     * @example
     * ```ts
     * const formatter = new Intl.DateTimeFormat('en-US', { dateStyle: 'long' });
     * const startDate = new Date(2023, 0, 1);
     * const endDate = new Date(2023, 0, 5);
     * const parts = formatter.formatRangeToParts(startDate, endDate);
     * console.log(parts);
     * // Output might include:
     * // [
     * //   { type: 'month', value: 'January', source: 'startRange' },
     * //   { type: 'literal', value: ' ', source: 'shared' },
     * //   { type: 'day', value: '1', source: 'startRange' },
     * //   { type: 'literal', value: ' – ', source: 'shared' },
     * //   { type: 'day', value: '5', source: 'endRange' },
     * //   { type: 'literal', value: ', ', source: 'shared' },
     * //   { type: 'year', value: '2023', source: 'shared' }
     * // ]
     * ```
     */
    formatRangeToParts<T extends Formattable>(
      startDate: T,
      endDate: T,
    ): DateTimeFormatRangePart[];
    formatRangeToParts(
      startDate: Date | number,
      endDate: Date | number,
    ): DateTimeFormatRangePart[];
  }

  /**
   * @category Intl
   * @experimental
   */
  export interface DateTimeFormatOptions {
    // TODO: remove the props below after TS lib declarations are updated
    dayPeriod?: "narrow" | "short" | "long";
    dateStyle?: "full" | "long" | "medium" | "short";
    timeStyle?: "full" | "long" | "medium" | "short";
  }
}

/**
 * A typed array of 16-bit float values. The contents are initialized to 0. If the requested number
 * of bytes could not be allocated an exception is raised.
 *
 * @category Platform
 * @experimental
 */
interface Float16Array<TArrayBuffer extends ArrayBufferLike = ArrayBufferLike> {
  /**
   * The size in bytes of each element in the array.
   */
  readonly BYTES_PER_ELEMENT: number;

  /**
   * The ArrayBuffer instance referenced by the array.
   */
  readonly buffer: TArrayBuffer;

  /**
   * The length in bytes of the array.
   */
  readonly byteLength: number;

  /**
   * The offset in bytes of the array.
   */
  readonly byteOffset: number;

  /**
   * Returns the item located at the specified index.
   * @param index The zero-based index of the desired code unit. A negative index will count back from the last item.
   */
  at(index: number): number | undefined;

  /**
   * Returns the this object after copying a section of the array identified by start and end
   * to the same array starting at position target
   * @param target If target is negative, it is treated as length+target where length is the
   * length of the array.
   * @param start If start is negative, it is treated as length+start. If end is negative, it
   * is treated as length+end.
   * @param end If not specified, length of the this object is used as its default value.
   */
  copyWithin(target: number, start: number, end?: number): this;

  /**
   * Determines whether all the members of an array satisfy the specified test.
   * @param predicate A function that accepts up to three arguments. The every method calls
   * the predicate function for each element in the array until the predicate returns a value
   * which is coercible to the Boolean value false, or until the end of the array.
   * @param thisArg An object to which the this keyword can refer in the predicate function.
   * If thisArg is omitted, undefined is used as the this value.
   */
  every(
    predicate: (value: number, index: number, array: this) => unknown,
    thisArg?: any,
  ): boolean;

  /**
   * Changes all array elements from `start` to `end` index to a static `value` and returns the modified array
   * @param value value to fill array section with
   * @param start index to start filling the array at. If start is negative, it is treated as
   * length+start where length is the length of the array.
   * @param end index to stop filling the array at. If end is negative, it is treated as
   * length+end.
   */
  fill(value: number, start?: number, end?: number): this;

  /**
   * Returns the elements of an array that meet the condition specified in a callback function.
   * @param predicate A function that accepts up to three arguments. The filter method calls
   * the predicate function one time for each element in the array.
   * @param thisArg An object to which the this keyword can refer in the predicate function.
   * If thisArg is omitted, undefined is used as the this value.
   */
  filter(
    predicate: (value: number, index: number, array: this) => any,
    thisArg?: any,
  ): Float16Array<ArrayBuffer>;

  /**
   * Returns the value of the first element in the array where predicate is true, and undefined
   * otherwise.
   * @param predicate find calls predicate once for each element of the array, in ascending
   * order, until it finds one where predicate returns true. If such an element is found, find
   * immediately returns that element value. Otherwise, find returns undefined.
   * @param thisArg If provided, it will be used as the this value for each invocation of
   * predicate. If it is not provided, undefined is used instead.
   */
  find(
    predicate: (value: number, index: number, obj: this) => boolean,
    thisArg?: any,
  ): number | undefined;

  /**
   * Returns the index of the first element in the array where predicate is true, and -1
   * otherwise.
   * @param predicate find calls predicate once for each element of the array, in ascending
   * order, until it finds one where predicate returns true. If such an element is found,
   * findIndex immediately returns that element index. Otherwise, findIndex returns -1.
   * @param thisArg If provided, it will be used as the this value for each invocation of
   * predicate. If it is not provided, undefined is used instead.
   */
  findIndex(
    predicate: (value: number, index: number, obj: this) => boolean,
    thisArg?: any,
  ): number;

  /**
   * Returns the value of the last element in the array where predicate is true, and undefined
   * otherwise.
   * @param predicate findLast calls predicate once for each element of the array, in descending
   * order, until it finds one where predicate returns true. If such an element is found, findLast
   * immediately returns that element value. Otherwise, findLast returns undefined.
   * @param thisArg If provided, it will be used as the this value for each invocation of
   * predicate. If it is not provided, undefined is used instead.
   */
  findLast<S extends number>(
    predicate: (
      value: number,
      index: number,
      array: this,
    ) => value is S,
    thisArg?: any,
  ): S | undefined;
  findLast(
    predicate: (
      value: number,
      index: number,
      array: this,
    ) => unknown,
    thisArg?: any,
  ): number | undefined;

  /**
   * Returns the index of the last element in the array where predicate is true, and -1
   * otherwise.
   * @param predicate findLastIndex calls predicate once for each element of the array, in descending
   * order, until it finds one where predicate returns true. If such an element is found,
   * findLastIndex immediately returns that element index. Otherwise, findLastIndex returns -1.
   * @param thisArg If provided, it will be used as the this value for each invocation of
   * predicate. If it is not provided, undefined is used instead.
   */
  findLastIndex(
    predicate: (
      value: number,
      index: number,
      array: this,
    ) => unknown,
    thisArg?: any,
  ): number;

  /**
   * Performs the specified action for each element in an array.
   * @param callbackfn  A function that accepts up to three arguments. forEach calls the
   * callbackfn function one time for each element in the array.
   * @param thisArg  An object to which the this keyword can refer in the callbackfn function.
   * If thisArg is omitted, undefined is used as the this value.
   */
  forEach(
    callbackfn: (value: number, index: number, array: this) => void,
    thisArg?: any,
  ): void;

  /**
   * Determines whether an array includes a certain element, returning true or false as appropriate.
   * @param searchElement The element to search for.
   * @param fromIndex The position in this array at which to begin searching for searchElement.
   */
  includes(searchElement: number, fromIndex?: number): boolean;

  /**
   * Returns the index of the first occurrence of a value in an array.
   * @param searchElement The value to locate in the array.
   * @param fromIndex The array index at which to begin the search. If fromIndex is omitted, the
   *  search starts at index 0.
   */
  indexOf(searchElement: number, fromIndex?: number): number;

  /**
   * Adds all the elements of an array separated by the specified separator string.
   * @param separator A string used to separate one element of an array from the next in the
   * resulting String. If omitted, the array elements are separated with a comma.
   */
  join(separator?: string): string;

  /**
   * Returns the index of the last occurrence of a value in an array.
   * @param searchElement The value to locate in the array.
   * @param fromIndex The array index at which to begin the search. If fromIndex is omitted, the
   * search starts at index 0.
   */
  lastIndexOf(searchElement: number, fromIndex?: number): number;

  /**
   * The length of the array.
   */
  readonly length: number;

  /**
   * Calls a defined callback function on each element of an array, and returns an array that
   * contains the results.
   * @param callbackfn A function that accepts up to three arguments. The map method calls the
   * callbackfn function one time for each element in the array.
   * @param thisArg An object to which the this keyword can refer in the callbackfn function.
   * If thisArg is omitted, undefined is used as the this value.
   */
  map(
    callbackfn: (value: number, index: number, array: this) => number,
    thisArg?: any,
  ): Float16Array<ArrayBuffer>;

  /**
   * Calls the specified callback function for all the elements in an array. The return value of
   * the callback function is the accumulated result, and is provided as an argument in the next
   * call to the callback function.
   * @param callbackfn A function that accepts up to four arguments. The reduce method calls the
   * callbackfn function one time for each element in the array.
   * @param initialValue If initialValue is specified, it is used as the initial value to start
   * the accumulation. The first call to the callbackfn function provides this value as an argument
   * instead of an array value.
   */
  reduce(
    callbackfn: (
      previousValue: number,
      currentValue: number,
      currentIndex: number,
      array: this,
    ) => number,
  ): number;
  reduce(
    callbackfn: (
      previousValue: number,
      currentValue: number,
      currentIndex: number,
      array: this,
    ) => number,
    initialValue: number,
  ): number;

  /**
   * Calls the specified callback function for all the elements in an array. The return value of
   * the callback function is the accumulated result, and is provided as an argument in the next
   * call to the callback function.
   * @param callbackfn A function that accepts up to four arguments. The reduce method calls the
   * callbackfn function one time for each element in the array.
   * @param initialValue If initialValue is specified, it is used as the initial value to start
   * the accumulation. The first call to the callbackfn function provides this value as an argument
   * instead of an array value.
   */
  reduce<U>(
    callbackfn: (
      previousValue: U,
      currentValue: number,
      currentIndex: number,
      array: this,
    ) => U,
    initialValue: U,
  ): U;

  /**
   * Calls the specified callback function for all the elements in an array, in descending order.
   * The return value of the callback function is the accumulated result, and is provided as an
   * argument in the next call to the callback function.
   * @param callbackfn A function that accepts up to four arguments. The reduceRight method calls
   * the callbackfn function one time for each element in the array.
   * @param initialValue If initialValue is specified, it is used as the initial value to start
   * the accumulation. The first call to the callbackfn function provides this value as an
   * argument instead of an array value.
   */
  reduceRight(
    callbackfn: (
      previousValue: number,
      currentValue: number,
      currentIndex: number,
      array: this,
    ) => number,
  ): number;
  reduceRight(
    callbackfn: (
      previousValue: number,
      currentValue: number,
      currentIndex: number,
      array: this,
    ) => number,
    initialValue: number,
  ): number;

  /**
   * Calls the specified callback function for all the elements in an array, in descending order.
   * The return value of the callback function is the accumulated result, and is provided as an
   * argument in the next call to the callback function.
   * @param callbackfn A function that accepts up to four arguments. The reduceRight method calls
   * the callbackfn function one time for each element in the array.
   * @param initialValue If initialValue is specified, it is used as the initial value to start
   * the accumulation. The first call to the callbackfn function provides this value as an argument
   * instead of an array value.
   */
  reduceRight<U>(
    callbackfn: (
      previousValue: U,
      currentValue: number,
      currentIndex: number,
      array: this,
    ) => U,
    initialValue: U,
  ): U;

  /**
   * Reverses the elements in an Array.
   */
  reverse(): this;

  /**
   * Sets a value or an array of values.
   * @param array A typed or untyped array of values to set.
   * @param offset The index in the current array at which the values are to be written.
   */
  set(array: ArrayLike<number>, offset?: number): void;

  /**
   * Returns a section of an array.
   * @param start The beginning of the specified portion of the array.
   * @param end The end of the specified portion of the array. This is exclusive of the element at the index 'end'.
   */
  slice(start?: number, end?: number): Float16Array<ArrayBuffer>;

  /**
   * Determines whether the specified callback function returns true for any element of an array.
   * @param predicate A function that accepts up to three arguments. The some method calls
   * the predicate function for each element in the array until the predicate returns a value
   * which is coercible to the Boolean value true, or until the end of the array.
   * @param thisArg An object to which the this keyword can refer in the predicate function.
   * If thisArg is omitted, undefined is used as the this value.
   */
  some(
    predicate: (value: number, index: number, array: this) => unknown,
    thisArg?: any,
  ): boolean;

  /**
   * Sorts an array.
   * @param compareFn Function used to determine the order of the elements. It is expected to return
   * a negative value if first argument is less than second argument, zero if they're equal and a positive
   * value otherwise. If omitted, the elements are sorted in ascending order.
   * ```ts
   * [11,2,22,1].sort((a, b) => a - b)
   * ```
   */
  sort(compareFn?: (a: number, b: number) => number): this;

  /**
   * Gets a new Float16Array view of the ArrayBuffer store for this array, referencing the elements
   * at begin, inclusive, up to end, exclusive.
   * @param begin The index of the beginning of the array.
   * @param end The index of the end of the array.
   */
  subarray(begin?: number, end?: number): Float16Array<TArrayBuffer>;

  /**
   * Converts a number to a string by using the current locale.
   */
  toLocaleString(
    locales?: string | string[],
    options?: Intl.NumberFormatOptions,
  ): string;

  /**
   * Copies the array and returns the copy with the elements in reverse order.
   */
  toReversed(): Float16Array<ArrayBuffer>;

  /**
   * Copies and sorts the array.
   * @param compareFn Function used to determine the order of the elements. It is expected to return
   * a negative value if the first argument is less than the second argument, zero if they're equal, and a positive
   * value otherwise. If omitted, the elements are sorted in ascending order.
   * ```ts
   * const myNums = Float16Array.from([11.25, 2, -22.5, 1]);
   * myNums.toSorted((a, b) => a - b) // Float16Array<Buffer>(4) [-22.5, 1, 2, 11.5]
   * ```
   */
  toSorted(
    compareFn?: (a: number, b: number) => number,
  ): Float16Array<ArrayBuffer>;

  /**
   * Returns a string representation of an array.
   */
  toString(): string;

  /** Returns the primitive value of the specified object. */
  valueOf(): this;

  /**
   * Copies the array and inserts the given number at the provided index.
   * @param index The index of the value to overwrite. If the index is
   * negative, then it replaces from the end of the array.
   * @param value The value to insert into the copied array.
   * @returns A copy of the original array with the inserted value.
   */
  with(index: number, value: number): Float16Array<ArrayBuffer>;

  [index: number]: number;

  [Symbol.iterator](): ArrayIterator<number>;

  /**
   * Returns an array of key, value pairs for every entry in the array
   */
  entries(): ArrayIterator<[number, number]>;

  /**
   * Returns an list of keys in the array
   */
  keys(): ArrayIterator<number>;

  /**
   * Returns an list of values in the array
   */
  values(): ArrayIterator<number>;

  readonly [Symbol.toStringTag]: "Float16Array";
}

/**
 * @category Platform
 * @experimental
 */
interface Float16ArrayConstructor {
  readonly prototype: Float16Array<ArrayBufferLike>;
  new (length?: number): Float16Array<ArrayBuffer>;
  new (array: ArrayLike<number> | Iterable<number>): Float16Array<ArrayBuffer>;
  new <TArrayBuffer extends ArrayBufferLike = ArrayBuffer>(
    buffer: TArrayBuffer,
    byteOffset?: number,
    length?: number,
  ): Float16Array<TArrayBuffer>;

  /**
   * The size in bytes of each element in the array.
   */
  readonly BYTES_PER_ELEMENT: number;

  /**
   * Returns a new array from a set of elements.
   * @param items A set of elements to include in the new array object.
   */
  of(...items: number[]): Float16Array<ArrayBuffer>;

  /**
   * Creates an array from an array-like or iterable object.
   * @param arrayLike An array-like object to convert to an array.
   */
  from(arrayLike: ArrayLike<number>): Float16Array<ArrayBuffer>;

  /**
   * Creates an array from an array-like or iterable object.
   * @param arrayLike An array-like object to convert to an array.
   * @param mapfn A mapping function to call on every element of the array.
   * @param thisArg Value of 'this' used to invoke the mapfn.
   */
  from<T>(
    arrayLike: ArrayLike<T>,
    mapfn: (v: T, k: number) => number,
    thisArg?: any,
  ): Float16Array<ArrayBuffer>;

  /**
   * Creates an array from an array-like or iterable object.
   * @param elements An iterable object to convert to an array.
   */
  from(elements: Iterable<number>): Float16Array<ArrayBuffer>;

  /**
   * Creates an array from an array-like or iterable object.
   * @param elements An iterable object to convert to an array.
   * @param mapfn A mapping function to call on every element of the array.
   * @param thisArg Value of 'this' used to invoke the mapfn.
   */
  from<T>(
    elements: Iterable<T>,
    mapfn?: (v: T, k: number) => number,
    thisArg?: any,
  ): Float16Array<ArrayBuffer>;
}

/**
 * @category Platform
 * @experimental
 */
declare var Float16Array: Float16ArrayConstructor;

/**
 * @category Platform
 * @experimental
 */
interface Math {
  /**
   * Returns the nearest half precision float representation of a number.
   * @param x A numeric expression.
   *
   * @category Platform
   * @experimental
   */
  f16round(x: number): number;
}

/**
 * @category Platform
 * @experimental
 */
interface DataView<TArrayBuffer extends ArrayBufferLike> {
  /**
   * Gets the Float16 value at the specified byte offset from the start of the view. There is
   * no alignment constraint; multi-byte values may be fetched from any offset.
   * @param byteOffset The place in the buffer at which the value should be retrieved.
   * @param littleEndian If false or undefined, a big-endian value should be read.
   *
   * @category Platform
   * @experimental
   */
  getFloat16(byteOffset: number, littleEndian?: boolean): number;

  /**
   * Stores an Float16 value at the specified byte offset from the start of the view.
   * @param byteOffset The place in the buffer at which the value should be set.
   * @param value The value to set.
   * @param littleEndian If false or undefined, a big-endian value should be written.
   *
   * @category Platform
   * @experimental
   */
  setFloat16(byteOffset: number, value: number, littleEndian?: boolean): void;
}

/**
 * @category Platform
 * @experimental
 */
interface ErrorConstructor {
  /**
   * Indicates whether the argument provided is a built-in Error instance or not.
   */
  isError(error: unknown): error is Error;
}
