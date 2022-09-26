// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="deno.net" />

/** Deno provides extra properties on `import.meta`.  These are included here
 * to ensure that these are still available when using the Deno namespace in
 * conjunction with other type libs, like `dom`.
 *
 * @category ES Modules
 */
declare interface ImportMeta {
  /** A string representation of the fully qualified module URL. */
  url: string;

  /** A flag that indicates if the current module is the main module that was
   * called when starting the program under Deno.
   *
   * ```ts
   * if (import.meta.main) {
   *   // this was loaded as the main module, maybe do some bootstrapping
   * }
   * ```
   */
  main: boolean;

  /** A function that returns resolved specifier as if it would be imported
   * using `import(specifier)`.
   *
   * ```ts
   * console.log(import.meta.resolve("./foo.js"));
   * // file:///dev/foo.js
   * ```
   */
  resolve(specifier: string): string;
}

/** Deno supports user timing Level 3 (see: https://w3c.github.io/user-timing)
 * which is not widely supported yet in other runtimes.  These types are here
 * so that these features are still available when using the Deno namespace
 * in conjunction with other type libs, like `dom`.
 *
 * @category Performance API
 */
declare interface Performance {
  /** Stores a timestamp with the associated name (a "mark"). */
  mark(markName: string, options?: PerformanceMarkOptions): PerformanceMark;

  /** Stores the `DOMHighResTimeStamp` duration between two marks along with the
   * associated name (a "measure"). */
  measure(
    measureName: string,
    options?: PerformanceMeasureOptions,
  ): PerformanceMeasure;
}

/**
 * @category Performance API
 */
declare interface PerformanceMarkOptions {
  /** Metadata to be included in the mark. */
  // deno-lint-ignore no-explicit-any
  detail?: any;

  /** Timestamp to be used as the mark time. */
  startTime?: number;
}

/**
 * @category Performance API
 */
declare interface PerformanceMeasureOptions {
  /** Metadata to be included in the measure. */
  // deno-lint-ignore no-explicit-any
  detail?: any;

  /** Timestamp to be used as the start time or string to be used as start
   * mark. */
  start?: string | number;

  /** Duration between the start and end times. */
  duration?: number;

  /** Timestamp to be used as the end time or string to be used as end mark. */
  end?: string | number;
}

declare namespace Deno {
  /** A set of error constructors that are raised by Deno APIs.
   *
   * @category Errors
   */
  export namespace errors {
    /** @category Errors */
    export class NotFound extends Error {}
    /** @category Errors */
    export class PermissionDenied extends Error {}
    /** @category Errors */
    export class ConnectionRefused extends Error {}
    /** @category Errors */
    export class ConnectionReset extends Error {}
    /** @category Errors */
    export class ConnectionAborted extends Error {}
    /** @category Errors */
    export class NotConnected extends Error {}
    /** @category Errors */
    export class AddrInUse extends Error {}
    /** @category Errors */
    export class AddrNotAvailable extends Error {}
    /** @category Errors */
    export class BrokenPipe extends Error {}
    /** @category Errors */
    export class AlreadyExists extends Error {}
    /** @category Errors */
    export class InvalidData extends Error {}
    /** @category Errors */
    export class TimedOut extends Error {}
    /** @category Errors */
    export class Interrupted extends Error {}
    /** @category Errors */
    export class WriteZero extends Error {}
    /** @category Errors */
    export class UnexpectedEof extends Error {}
    /** @category Errors */
    export class BadResource extends Error {}
    /** @category Errors */
    export class Http extends Error {}
    /** @category Errors */
    export class Busy extends Error {}
    /** @category Errors */
    export class NotSupported extends Error {}
  }

  /** The current process id of the runtime.
   *
   * @category Runtime Environment
   */
  export const pid: number;

  /**
   * The pid of the current process's parent.
   *
   * @category Runtime Environment
   */
  export const ppid: number;

  /** @category Runtime Environment */
  export interface MemoryUsage {
    rss: number;
    heapTotal: number;
    heapUsed: number;
    external: number;
  }

  /**
   * Returns an object describing the memory usage of the Deno process measured
   * in bytes.
   *
   * @category Runtime Environment
   */
  export function memoryUsage(): MemoryUsage;

  /** Reflects the `NO_COLOR` environment variable at program start.
   *
   * See: https://no-color.org/
   *
   * @category Runtime Environment
   */
  export const noColor: boolean;

  /** @category Permissions */
  export type PermissionOptions = "inherit" | "none" | PermissionOptionsObject;

  /** @category Permissions */
  export interface PermissionOptionsObject {
    /** Specifies if the `env` permission should be requested or revoked.
     * If set to `"inherit"`, the current `env` permission will be inherited.
     * If set to `true`, the global `env` permission will be requested.
     * If set to `false`, the global `env` permission will be revoked.
     *
     * Defaults to `false`.
     */
    env?: "inherit" | boolean | string[];

    /** Specifies if the `hrtime` permission should be requested or revoked.
     * If set to `"inherit"`, the current `hrtime` permission will be inherited.
     * If set to `true`, the global `hrtime` permission will be requested.
     * If set to `false`, the global `hrtime` permission will be revoked.
     *
     * Defaults to `false`.
     */
    hrtime?: "inherit" | boolean;

    /** Specifies if the `net` permission should be requested or revoked.
     * if set to `"inherit"`, the current `net` permission will be inherited.
     * if set to `true`, the global `net` permission will be requested.
     * if set to `false`, the global `net` permission will be revoked.
     * if set to `string[]`, the `net` permission will be requested with the
     * specified host strings with the format `"<host>[:<port>]`.
     *
     * Defaults to `false`.
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
     * Defaults to `false`.
     */
    ffi?: "inherit" | boolean | Array<string | URL>;

    /** Specifies if the `read` permission should be requested or revoked.
     * If set to `"inherit"`, the current `read` permission will be inherited.
     * If set to `true`, the global `read` permission will be requested.
     * If set to `false`, the global `read` permission will be revoked.
     * If set to `Array<string | URL>`, the `read` permission will be requested with the
     * specified file paths.
     *
     * Defaults to `false`.
     */
    read?: "inherit" | boolean | Array<string | URL>;

    /** Specifies if the `run` permission should be requested or revoked.
     * If set to `"inherit"`, the current `run` permission will be inherited.
     * If set to `true`, the global `run` permission will be requested.
     * If set to `false`, the global `run` permission will be revoked.
     *
     * Defaults to `false`.
     */
    run?: "inherit" | boolean | Array<string | URL>;

    /** Specifies if the `write` permission should be requested or revoked.
     * If set to `"inherit"`, the current `write` permission will be inherited.
     * If set to `true`, the global `write` permission will be requested.
     * If set to `false`, the global `write` permission will be revoked.
     * If set to `Array<string | URL>`, the `write` permission will be requested with the
     * specified file paths.
     *
     * Defaults to `false`.
     */
    write?: "inherit" | boolean | Array<string | URL>;
  }

  /** @category Testing */
  export interface TestContext {
    /**
     * The current test name.
     */
    name: string;
    /**
     * File Uri of the current test code.
     */
    origin: string;
    /**
     * Parent test context.
     */
    parent?: TestContext;

    /** Run a sub step of the parent test or step. Returns a promise
     * that resolves to a boolean signifying if the step completed successfully.
     * The returned promise never rejects unless the arguments are invalid.
     * If the test was ignored the promise returns `false`.
     */
    step(t: TestStepDefinition): Promise<boolean>;

    /** Run a sub step of the parent test or step. Returns a promise
     * that resolves to a boolean signifying if the step completed successfully.
     * The returned promise never rejects unless the arguments are invalid.
     * If the test was ignored the promise returns `false`.
     */
    step(
      name: string,
      fn: (t: TestContext) => void | Promise<void>,
    ): Promise<boolean>;
  }

  /** @category Testing */
  export interface TestStepDefinition {
    fn: (t: TestContext) => void | Promise<void>;
    /**
     * The current test name.
     */
    name: string;
    ignore?: boolean;
    /** Check that the number of async completed ops after the test step is the same
     * as number of dispatched ops. Defaults to the parent test or step's value. */
    sanitizeOps?: boolean;
    /** Ensure the test step does not "leak" resources - ie. the resource table
     * after the test has exactly the same contents as before the test. Defaults
     * to the parent test or step's value. */
    sanitizeResources?: boolean;
    /** Ensure the test step does not prematurely cause the process to exit,
     * for example via a call to `Deno.exit`. Defaults to the parent test or
     * step's value. */
    sanitizeExit?: boolean;
  }

  /** @category Testing */
  export interface TestDefinition {
    fn: (t: TestContext) => void | Promise<void>;
    /**
     * The current test name.
     */
    name: string;
    ignore?: boolean;
    /** If at least one test has `only` set to true, only run tests that have
     * `only` set to true and fail the test suite. */
    only?: boolean;
    /** Check that the number of async completed ops after the test is the same
     * as number of dispatched ops. Defaults to true. */
    sanitizeOps?: boolean;
    /** Ensure the test case does not "leak" resources - ie. the resource table
     * after the test has exactly the same contents as before the test. Defaults
     * to true. */
    sanitizeResources?: boolean;
    /** Ensure the test case does not prematurely cause the process to exit,
     * for example via a call to `Deno.exit`. Defaults to true. */
    sanitizeExit?: boolean;

    /** Specifies the permissions that should be used to run the test.
     * Set this to "inherit" to keep the calling thread's permissions.
     * Set this to "none" to revoke all permissions.
     *
     * Defaults to "inherit".
     */
    permissions?: PermissionOptions;
  }

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   * `fn` can be async if required.
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test({
   *   name: "example test",
   *   fn(): void {
   *     assertEquals("world", "world");
   *   },
   * });
   *
   * Deno.test({
   *   name: "example ignored test",
   *   ignore: Deno.build.os === "windows",
   *   fn(): void {
   *     // This test is ignored only on Windows machines
   *   },
   * });
   *
   * Deno.test({
   *   name: "example async test",
   *   async fn() {
   *     const decoder = new TextDecoder("utf-8");
   *     const data = await Deno.readFile("hello_world.txt");
   *     assertEquals(decoder.decode(data), "Hello world");
   *   }
   * });
   * ```
   *
   * @category Testing
   */
  export function test(t: TestDefinition): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   * `fn` can be async if required.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test("My test description", (): void => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test("My async test description", async (): Promise<void> => {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function test(
    name: string,
    fn: (t: TestContext) => void | Promise<void>,
  ): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   * `fn` can be async if required. Declared function must have a name.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test(function myTestName(): void {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test(async function myOtherTestName(): Promise<void> {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function test(fn: (t: TestContext) => void | Promise<void>): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   * `fn` can be async if required.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test("My test description", { permissions: { read: true } }, (): void => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test("My async test description", { permissions: { read: false } }, async (): Promise<void> => {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function test(
    name: string,
    options: Omit<TestDefinition, "fn" | "name">,
    fn: (t: TestContext) => void | Promise<void>,
  ): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   * `fn` can be async if required.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test({ name: "My test description", permissions: { read: true } }, (): void => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test({ name: "My async test description", permissions: { read: false } }, async (): Promise<void> => {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function test(
    options: Omit<TestDefinition, "fn">,
    fn: (t: TestContext) => void | Promise<void>,
  ): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   * `fn` can be async if required. Declared function must have a name.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test({ permissions: { read: true } }, function myTestName(): void {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test({ permissions: { read: false } }, async function myOtherTestName(): Promise<void> {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function test(
    options: Omit<TestDefinition, "fn" | "name">,
    fn: (t: TestContext) => void | Promise<void>,
  ): void;

  /** Exit the Deno process with optional exit code. If no exit code is supplied
   * then Deno will exit with return code of 0.
   *
   * In worker contexts this is an alias to `self.close();`.
   *
   * ```ts
   * Deno.exit(5);
   * ```
   *
   * @category Runtime Environment
   */
  export function exit(code?: number): never;

  /**
   * @tags allow-env
   * @category Runtime Environment
   */
  export const env: {
    /** Retrieve the value of an environment variable. Returns `undefined` if that
     * key doesn't exist.
     *
     * ```ts
     * console.log(Deno.env.get("HOME"));  // e.g. outputs "/home/alice"
     * console.log(Deno.env.get("MADE_UP_VAR"));  // outputs "undefined"
     * ```
     * Requires `allow-env` permission.
     *
     * @tags allow-env
     */
    get(key: string): string | undefined;

    /** Set the value of an environment variable.
     *
     * ```ts
     * Deno.env.set("SOME_VAR", "Value");
     * Deno.env.get("SOME_VAR");  // outputs "Value"
     * ```
     *
     * Requires `allow-env` permission.
     *
     * @tags allow-env
     */
    set(key: string, value: string): void;

    /** Delete the value of an environment variable.
     *
     * ```ts
     * Deno.env.set("SOME_VAR", "Value");
     * Deno.env.delete("SOME_VAR");  // outputs "undefined"
     * ```
     *
     * Requires `allow-env` permission.
     *
     * @tags allow-env
     */
    delete(key: string): void;

    /** Returns a snapshot of the environment variables at invocation.
     *
     * ```ts
     * Deno.env.set("TEST_VAR", "A");
     * const myEnv = Deno.env.toObject();
     * console.log(myEnv.SHELL);
     * Deno.env.set("TEST_VAR", "B");
     * console.log(myEnv.TEST_VAR);  // outputs "A"
     * ```
     *
     * Requires `allow-env` permission.
     *
     * @tags allow-env
     */
    toObject(): { [index: string]: string };
  };

  /**
   * Returns the path to the current deno executable.
   *
   * ```ts
   * console.log(Deno.execPath());  // e.g. "/home/alice/.local/bin/deno"
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category Runtime Environment
   */
  export function execPath(): string;

  /**
   * Change the current working directory to the specified path.
   *
   * ```ts
   * Deno.chdir("/home/userA");
   * Deno.chdir("../userB");
   * Deno.chdir("C:\\Program Files (x86)\\Java");
   * ```
   *
   * Throws `Deno.errors.NotFound` if directory not found.
   * Throws `Deno.errors.PermissionDenied` if the user does not have access
   * rights
   *
   * Requires --allow-read.
   *
   * @tags allow-read
   * @category Runtime Environment
   */
  export function chdir(directory: string | URL): void;

  /**
   * Return a string representing the current working directory.
   *
   * If the current directory can be reached via multiple paths (due to symbolic
   * links), `cwd()` may return any one of them.
   *
   * ```ts
   * const currentWorkingDirectory = Deno.cwd();
   * ```
   *
   * Throws `Deno.errors.NotFound` if directory not available.
   *
   * Requires --allow-read
   *
   * @tags allow-read
   * @category Runtime Environment
   */
  export function cwd(): string;

  /**
   * Synchronously creates `newpath` as a hard link to `oldpath`.
   *
   * ```ts
   * Deno.linkSync("old/name", "new/name");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read
   * @category File System
   */
  export function linkSync(oldpath: string, newpath: string): void;

  /**
   * Creates `newpath` as a hard link to `oldpath`.
   *
   * ```ts
   * await Deno.link("old/name", "new/name");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read
   * @category File System
   */
  export function link(oldpath: string, newpath: string): Promise<void>;

  /** @category I/O */
  export enum SeekMode {
    Start = 0,
    Current = 1,
    End = 2,
  }

  /** @category I/O */
  export interface Reader {
    /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number of
     * bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
     * encountered. Even if `read()` resolves to `n` < `p.byteLength`, it may
     * use all of `p` as scratch space during the call. If some data is
     * available but not `p.byteLength` bytes, `read()` conventionally resolves
     * to what is available instead of waiting for more.
     *
     * When `read()` encounters end-of-file condition, it resolves to EOF
     * (`null`).
     *
     * When `read()` encounters an error, it rejects with an error.
     *
     * Callers should always process the `n` > `0` bytes returned before
     * considering the EOF (`null`). Doing so correctly handles I/O errors that
     * happen after reading some bytes and also both of the allowed EOF
     * behaviors.
     *
     * Implementations should not retain a reference to `p`.
     *
     * Use `itereateReader` from from https://deno.land/std/streams/conversion.ts to
     * turn a Reader into an AsyncIterator.
     */
    read(p: Uint8Array): Promise<number | null>;
  }

  /** @category I/O */
  export interface ReaderSync {
    /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number
     * of bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
     * encountered. Even if `readSync()` returns `n` < `p.byteLength`, it may use
     * all of `p` as scratch space during the call. If some data is available
     * but not `p.byteLength` bytes, `readSync()` conventionally returns what is
     * available instead of waiting for more.
     *
     * When `readSync()` encounters end-of-file condition, it returns EOF
     * (`null`).
     *
     * When `readSync()` encounters an error, it throws with an error.
     *
     * Callers should always process the `n` > `0` bytes returned before
     * considering the EOF (`null`). Doing so correctly handles I/O errors that happen
     * after reading some bytes and also both of the allowed EOF behaviors.
     *
     * Implementations should not retain a reference to `p`.
     *
     * Use `iterateReaderSync()` from from https://deno.land/std/streams/conversion.ts
     * to turn a ReaderSync into an Iterator.
     */
    readSync(p: Uint8Array): number | null;
  }

  /** @category I/O */
  export interface Writer {
    /** Writes `p.byteLength` bytes from `p` to the underlying data stream. It
     * resolves to the number of bytes written from `p` (`0` <= `n` <=
     * `p.byteLength`) or reject with the error encountered that caused the
     * write to stop early. `write()` must reject with a non-null error if
     * would resolve to `n` < `p.byteLength`. `write()` must not modify the
     * slice data, even temporarily.
     *
     * Implementations should not retain a reference to `p`.
     */
    write(p: Uint8Array): Promise<number>;
  }

  /** @category I/O */
  export interface WriterSync {
    /** Writes `p.byteLength` bytes from `p` to the underlying data
     * stream. It returns the number of bytes written from `p` (`0` <= `n`
     * <= `p.byteLength`) and any error encountered that caused the write to
     * stop early. `writeSync()` must throw a non-null error if it returns `n` <
     * `p.byteLength`. `writeSync()` must not modify the slice data, even
     * temporarily.
     *
     * Implementations should not retain a reference to `p`.
     */
    writeSync(p: Uint8Array): number;
  }

  /** @category I/O */
  export interface Closer {
    close(): void;
  }

  /** @category I/O */
  export interface Seeker {
    /** Seek sets the offset for the next `read()` or `write()` to offset,
     * interpreted according to `whence`: `Start` means relative to the
     * start of the file, `Current` means relative to the current offset,
     * and `End` means relative to the end. Seek resolves to the new offset
     * relative to the start of the file.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O
     * operations on the underlying object is implementation-dependent.
     * It returns the number of cursor position.
     */
    seek(offset: number, whence: SeekMode): Promise<number>;
  }

  /** @category I/O */
  export interface SeekerSync {
    /** Seek sets the offset for the next `readSync()` or `writeSync()` to
     * offset, interpreted according to `whence`: `Start` means relative
     * to the start of the file, `Current` means relative to the current
     * offset, and `End` means relative to the end.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O
     * operations on the underlying object is implementation-dependent.
     */
    seekSync(offset: number, whence: SeekMode): number;
  }

  /**
   * Copies from `src` to `dst` until either EOF (`null`) is read from `src` or
   * an error occurs. It resolves to the number of bytes copied or rejects with
   * the first error encountered while copying.
   *
   * ```ts
   * const source = await Deno.open("my_file.txt");
   * const bytesCopied1 = await Deno.copy(source, Deno.stdout);
   * const destination = await Deno.create("my_file_2.txt");
   * const bytesCopied2 = await Deno.copy(source, destination);
   * ```
   *
   * @deprecated Use `copy` from https://deno.land/std/streams/conversion.ts
   * instead. `Deno.copy` will be removed in Deno 2.0.
   *
   * @category I/O
   *
   * @param src The source to copy from
   * @param dst The destination to copy to
   * @param options Can be used to tune size of the buffer. Default size is 32kB
   */
  export function copy(
    src: Reader,
    dst: Writer,
    options?: {
      bufSize?: number;
    },
  ): Promise<number>;

  /**
   * Turns a Reader, `r`, into an async iterator.
   *
   * ```ts
   * let f = await Deno.open("/etc/passwd");
   * for await (const chunk of Deno.iter(f)) {
   *   console.log(chunk);
   * }
   * f.close();
   * ```
   *
   * Second argument can be used to tune size of a buffer.
   * Default size of the buffer is 32kB.
   *
   * ```ts
   * let f = await Deno.open("/etc/passwd");
   * const iter = Deno.iter(f, {
   *   bufSize: 1024 * 1024
   * });
   * for await (const chunk of iter) {
   *   console.log(chunk);
   * }
   * f.close();
   * ```
   *
   * Iterator uses an internal buffer of fixed size for efficiency; it returns
   * a view on that buffer on each iteration. It is therefore caller's
   * responsibility to copy contents of the buffer if needed; otherwise the
   * next iteration will overwrite contents of previously returned chunk.
   *
   * @deprecated Use `iterateReader` from
   * https://deno.land/std/streams/conversion.ts instead. `Deno.iter` will be
   * removed in Deno 2.0.
   *
   * @category I/O
   */
  export function iter(
    r: Reader,
    options?: {
      bufSize?: number;
    },
  ): AsyncIterableIterator<Uint8Array>;

  /**
   * Turns a ReaderSync, `r`, into an iterator.
   *
   * ```ts
   * let f = Deno.openSync("/etc/passwd");
   * for (const chunk of Deno.iterSync(f)) {
   *   console.log(chunk);
   * }
   * f.close();
   * ```
   *
   * Second argument can be used to tune size of a buffer.
   * Default size of the buffer is 32kB.
   *
   * ```ts
   * let f = await Deno.open("/etc/passwd");
   * const iter = Deno.iterSync(f, {
   *   bufSize: 1024 * 1024
   * });
   * for (const chunk of iter) {
   *   console.log(chunk);
   * }
   * f.close();
   * ```
   *
   * Iterator uses an internal buffer of fixed size for efficiency; it returns
   * a view on that buffer on each iteration. It is therefore caller's
   * responsibility to copy contents of the buffer if needed; otherwise the
   * next iteration will overwrite contents of previously returned chunk.
   *
   * @deprecated Use `iterateReaderSync` from
   * https://deno.land/std/streams/conversion.ts instead. `Deno.iterSync` will
   * be removed in Deno 2.0.
   *
   * @category I/O
   */
  export function iterSync(
    r: ReaderSync,
    options?: {
      bufSize?: number;
    },
  ): IterableIterator<Uint8Array>;

  /** Synchronously open a file and return an instance of `Deno.FsFile`.  The
   * file does not need to previously exist if using the `create` or `createNew`
   * open options.  It is the callers responsibility to close the file when finished
   * with it.
   *
   * ```ts
   * const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
   * // Do work with file
   * Deno.close(file.rid);
   * ```
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on options.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function openSync(path: string | URL, options?: OpenOptions): FsFile;

  /** Open a file and resolve to an instance of `Deno.FsFile`.  The
   * file does not need to previously exist if using the `create` or `createNew`
   * open options.  It is the callers responsibility to close the file when finished
   * with it.
   *
   * ```ts
   * const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
   * // Do work with file
   * Deno.close(file.rid);
   * ```
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on options.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function open(
    path: string | URL,
    options?: OpenOptions,
  ): Promise<FsFile>;

  /** Creates a file if none exists or truncates an existing file and returns
   *  an instance of `Deno.FsFile`.
   *
   * ```ts
   * const file = Deno.createSync("/foo/bar.txt");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function createSync(path: string | URL): FsFile;

  /** Creates a file if none exists or truncates an existing file and resolves to
   *  an instance of `Deno.FsFile`.
   *
   * ```ts
   * const file = await Deno.create("/foo/bar.txt");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function create(path: string | URL): Promise<FsFile>;

  /** Synchronously read from a resource ID (`rid`) into an array buffer (`buffer`).
   *
   * Returns either the number of bytes read during the operation or EOF
   * (`null`) if there was nothing more to read.
   *
   * It is possible for a read to successfully return with `0` bytes. This does
   * not indicate EOF.
   *
   * This function is one of the lowest level APIs and most users should not
   * work with this directly, but rather use
   * `readAllSync()` from https://deno.land/std/streams/conversion.ts instead.
   *
   * **It is not guaranteed that the full buffer will be read in a single call.**
   *
   * ```ts
   * // if "/foo/bar.txt" contains the text "hello world":
   * const file = Deno.openSync("/foo/bar.txt");
   * const buf = new Uint8Array(100);
   * const numberOfBytesRead = Deno.readSync(file.rid, buf); // 11 bytes
   * const text = new TextDecoder().decode(buf);  // "hello world"
   * Deno.close(file.rid);
   * ```
   *
   * @category I/O
   */
  export function readSync(rid: number, buffer: Uint8Array): number | null;

  /** Read from a resource ID (`rid`) into an array buffer (`buffer`).
   *
   * Resolves to either the number of bytes read during the operation or EOF
   * (`null`) if there was nothing more to read.
   *
   * It is possible for a read to successfully return with `0` bytes. This does
   * not indicate EOF.
   *
   * This function is one of the lowest level APIs and most users should not
   * work with this directly, but rather use
   * `readAll()` from https://deno.land/std/streams/conversion.ts instead.
   *
   * **It is not guaranteed that the full buffer will be read in a single call.**
   *
   * ```ts
   * // if "/foo/bar.txt" contains the text "hello world":
   * const file = await Deno.open("/foo/bar.txt");
   * const buf = new Uint8Array(100);
   * const numberOfBytesRead = await Deno.read(file.rid, buf); // 11 bytes
   * const text = new TextDecoder().decode(buf);  // "hello world"
   * Deno.close(file.rid);
   * ```
   *
   * @category I/O
   */
  export function read(rid: number, buffer: Uint8Array): Promise<number | null>;

  /** Synchronously write to the resource ID (`rid`) the contents of the array
   * buffer (`data`).
   *
   * Returns the number of bytes written.  This function is one of the lowest
   * level APIs and most users should not work with this directly, but rather use
   * `writeAllSync()` from https://deno.land/std/streams/conversion.ts instead.
   *
   * **It is not guaranteed that the full buffer will be written in a single
   * call.**
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world");
   * const file = Deno.openSync("/foo/bar.txt", {write: true});
   * const bytesWritten = Deno.writeSync(file.rid, data); // 11
   * Deno.close(file.rid);
   * ```
   *
   * @category I/O
   */
  export function writeSync(rid: number, data: Uint8Array): number;

  /** Write to the resource ID (`rid`) the contents of the array buffer (`data`).
   *
   * Resolves to the number of bytes written.  This function is one of the lowest
   * level APIs and most users should not work with this directly, but rather use
   * `writeAll()` from https://deno.land/std/streams/conversion.ts instead.
   *
   * **It is not guaranteed that the full buffer will be written in a single
   * call.**
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world");
   * const file = await Deno.open("/foo/bar.txt", { write: true });
   * const bytesWritten = await Deno.write(file.rid, data); // 11
   * Deno.close(file.rid);
   * ```
   *
   * @category I/O
   */
  export function write(rid: number, data: Uint8Array): Promise<number>;

  /** Synchronously seek a resource ID (`rid`) to the given `offset` under mode
   * given by `whence`.  The new position within the resource (bytes from the
   * start) is returned.
   *
   * ```ts
   * const file = Deno.openSync('hello.txt', {read: true, write: true, truncate: true, create: true});
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // advance cursor 6 bytes
   * const cursorPosition = Deno.seekSync(file.rid, 6, Deno.SeekMode.Start);
   * console.log(cursorPosition);  // 6
   * const buf = new Uint8Array(100);
   * file.readSync(buf);
   * console.log(new TextDecoder().decode(buf)); // "world"
   * ```
   *
   * The seek modes work as follows:
   *
   * ```ts
   * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   * const file = Deno.openSync('hello.txt', {read: true, write: true, truncate: true, create: true});
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // Seek 6 bytes from the start of the file
   * console.log(Deno.seekSync(file.rid, 6, Deno.SeekMode.Start)); // "6"
   * // Seek 2 more bytes from the current position
   * console.log(Deno.seekSync(file.rid, 2, Deno.SeekMode.Current)); // "8"
   * // Seek backwards 2 bytes from the end of the file
   * console.log(Deno.seekSync(file.rid, -2, Deno.SeekMode.End)); // "9" (e.g. 11-2)
   * ```
   *
   * @category I/O
   */
  export function seekSync(
    rid: number,
    offset: number,
    whence: SeekMode,
  ): number;

  /** Seek a resource ID (`rid`) to the given `offset` under mode given by `whence`.
   * The call resolves to the new position within the resource (bytes from the start).
   *
   * ```ts
   * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   * const file = await Deno.open('hello.txt', {read: true, write: true, truncate: true, create: true});
   * await Deno.write(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // advance cursor 6 bytes
   * const cursorPosition = await Deno.seek(file.rid, 6, Deno.SeekMode.Start);
   * console.log(cursorPosition);  // 6
   * const buf = new Uint8Array(100);
   * await file.read(buf);
   * console.log(new TextDecoder().decode(buf)); // "world"
   * ```
   *
   * The seek modes work as follows:
   *
   * ```ts
   * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   * const file = await Deno.open('hello.txt', {read: true, write: true, truncate: true, create: true});
   * await Deno.write(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // Seek 6 bytes from the start of the file
   * console.log(await Deno.seek(file.rid, 6, Deno.SeekMode.Start)); // "6"
   * // Seek 2 more bytes from the current position
   * console.log(await Deno.seek(file.rid, 2, Deno.SeekMode.Current)); // "8"
   * // Seek backwards 2 bytes from the end of the file
   * console.log(await Deno.seek(file.rid, -2, Deno.SeekMode.End)); // "9" (e.g. 11-2)
   * ```
   *
   * @category I/O
   */
  export function seek(
    rid: number,
    offset: number,
    whence: SeekMode,
  ): Promise<number>;

  /**
   * Synchronously flushes any pending data and metadata operations of the given
   * file stream to disk.
   *
   * ```ts
   * const file = Deno.openSync("my_file.txt", { read: true, write: true, create: true });
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.ftruncateSync(file.rid, 1);
   * Deno.fsyncSync(file.rid);
   * console.log(new TextDecoder().decode(Deno.readFileSync("my_file.txt"))); // H
   * ```
   *
   * @category I/O
   */
  export function fsyncSync(rid: number): void;

  /**
   * Flushes any pending data and metadata operations of the given file stream
   * to disk.
   *
   * ```ts
   * const file = await Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.ftruncate(file.rid, 1);
   * await Deno.fsync(file.rid);
   * console.log(new TextDecoder().decode(await Deno.readFile("my_file.txt"))); // H
   * ```
   *
   * @category I/O
   */
  export function fsync(rid: number): Promise<void>;

  /**
   * Synchronously flushes any pending data operations of the given file stream
   * to disk.
   *
   *  ```ts
   * const file = Deno.openSync("my_file.txt", { read: true, write: true, create: true });
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.fdatasyncSync(file.rid);
   * console.log(new TextDecoder().decode(Deno.readFileSync("my_file.txt"))); // Hello World
   * ```
   *
   * @category I/O
   */
  export function fdatasyncSync(rid: number): void;

  /**
   * Flushes any pending data operations of the given file stream to disk.
   *  ```ts
   * const file = await Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.fdatasync(file.rid);
   * console.log(new TextDecoder().decode(await Deno.readFile("my_file.txt"))); // Hello World
   * ```
   *
   * @category I/O
   */
  export function fdatasync(rid: number): Promise<void>;

  /** Close the given resource ID (rid) which has been previously opened, such
   * as via opening or creating a file.  Closing a file when you are finished
   * with it is important to avoid leaking resources.
   *
   * ```ts
   * const file = await Deno.open("my_file.txt");
   * // do work with "file" object
   * Deno.close(file.rid);
   * ```
   *
   * @category I/O
   */
  export function close(rid: number): void;

  /** The Deno abstraction for reading and writing files.
   *
   * @category File System
   */
  export class FsFile
    implements
      Reader,
      ReaderSync,
      Writer,
      WriterSync,
      Seeker,
      SeekerSync,
      Closer {
    readonly rid: number;
    constructor(rid: number);
    write(p: Uint8Array): Promise<number>;
    writeSync(p: Uint8Array): number;
    truncate(len?: number): Promise<void>;
    truncateSync(len?: number): void;
    read(p: Uint8Array): Promise<number | null>;
    readSync(p: Uint8Array): number | null;
    seek(offset: number, whence: SeekMode): Promise<number>;
    seekSync(offset: number, whence: SeekMode): number;
    stat(): Promise<FileInfo>;
    statSync(): FileInfo;
    close(): void;

    readonly readable: ReadableStream<Uint8Array>;
    readonly writable: WritableStream<Uint8Array>;
  }

  /**
   * The Deno abstraction for reading and writing files.
   *
   * @deprecated Use `Deno.FsFile` instead. `Deno.File` will be removed in Deno 2.0.
   * @category File System
   */
  export class File
    implements
      Reader,
      ReaderSync,
      Writer,
      WriterSync,
      Seeker,
      SeekerSync,
      Closer {
    readonly rid: number;
    constructor(rid: number);
    write(p: Uint8Array): Promise<number>;
    writeSync(p: Uint8Array): number;
    truncate(len?: number): Promise<void>;
    truncateSync(len?: number): void;
    read(p: Uint8Array): Promise<number | null>;
    readSync(p: Uint8Array): number | null;
    seek(offset: number, whence: SeekMode): Promise<number>;
    seekSync(offset: number, whence: SeekMode): number;
    stat(): Promise<FileInfo>;
    statSync(): FileInfo;
    close(): void;

    readonly readable: ReadableStream<Uint8Array>;
    readonly writable: WritableStream<Uint8Array>;
  }

  /** A handle for `stdin`.
   *
   * @category I/O
   */
  export const stdin: Reader & ReaderSync & Closer & {
    readonly rid: number;
    readonly readable: ReadableStream<Uint8Array>;
  };
  /** A handle for `stdout`.
   *
   * @category I/O
   */
  export const stdout: Writer & WriterSync & Closer & {
    readonly rid: number;
    readonly writable: WritableStream<Uint8Array>;
  };
  /** A handle for `stderr`.
   *
   * @category I/O
   */
  export const stderr: Writer & WriterSync & Closer & {
    readonly rid: number;
    readonly writable: WritableStream<Uint8Array>;
  };

  /** @category File System */
  export interface OpenOptions {
    /** Sets the option for read access. This option, when `true`, means that the
     * file should be read-able if opened. */
    read?: boolean;
    /** Sets the option for write access. This option, when `true`, means that
     * the file should be write-able if opened. If the file already exists,
     * any write calls on it will overwrite its contents, by default without
     * truncating it. */
    write?: boolean;
    /** Sets the option for the append mode. This option, when `true`, means that
     * writes will append to a file instead of overwriting previous contents.
     * Note that setting `{ write: true, append: true }` has the same effect as
     * setting only `{ append: true }`. */
    append?: boolean;
    /** Sets the option for truncating a previous file. If a file is
     * successfully opened with this option set it will truncate the file to `0`
     * size if it already exists. The file must be opened with write access
     * for truncate to work. */
    truncate?: boolean;
    /** Sets the option to allow creating a new file, if one doesn't already
     * exist at the specified path. Requires write or append access to be
     * used. */
    create?: boolean;
    /** Defaults to `false`. If set to `true`, no file, directory, or symlink is
     * allowed to exist at the target location. Requires write or append
     * access to be used. When createNew is set to `true`, create and truncate
     * are ignored. */
    createNew?: boolean;
    /** Permissions to use if creating the file (defaults to `0o666`, before
     * the process's umask).
     * Ignored on Windows. */
    mode?: number;
  }

  /** @category File System */
  export interface ReadFileOptions {
    /**
     * An abort signal to allow cancellation of the file read operation.
     * If the signal becomes aborted the readFile operation will be stopped
     * and the promise returned will be rejected with an AbortError.
     */
    signal?: AbortSignal;
  }

  /**
   *  Check if a given resource id (`rid`) is a TTY.
   *
   * ```ts
   * // This example is system and context specific
   * const nonTTYRid = Deno.openSync("my_file.txt").rid;
   * const ttyRid = Deno.openSync("/dev/tty6").rid;
   * console.log(Deno.isatty(nonTTYRid)); // false
   * console.log(Deno.isatty(ttyRid)); // true
   * Deno.close(nonTTYRid);
   * Deno.close(ttyRid);
   * ```
   *
   * @category I/O
   */
  export function isatty(rid: number): boolean;

  /**
   * A variable-sized buffer of bytes with `read()` and `write()` methods.
   *
   * Deno.Buffer is almost always used with some I/O like files and sockets. It
   * allows one to buffer up a download from a socket. Buffer grows and shrinks
   * as necessary.
   *
   * Deno.Buffer is NOT the same thing as Node's Buffer. Node's Buffer was
   * created in 2009 before JavaScript had the concept of ArrayBuffers. It's
   * simply a non-standard ArrayBuffer.
   *
   * ArrayBuffer is a fixed memory allocation. Deno.Buffer is implemented on top
   * of ArrayBuffer.
   *
   * Based on [Go Buffer](https://golang.org/pkg/bytes/#Buffer).
   *
   * @deprecated Use Buffer from https://deno.land/std/io/buffer.ts instead. Deno.Buffer will be removed in Deno 2.0.
   * @category I/O
   */
  export class Buffer implements Reader, ReaderSync, Writer, WriterSync {
    constructor(ab?: ArrayBuffer);
    /** Returns a slice holding the unread portion of the buffer.
     *
     * The slice is valid for use only until the next buffer modification (that
     * is, only until the next call to a method like `read()`, `write()`,
     * `reset()`, or `truncate()`). If `options.copy` is false the slice aliases the buffer content at
     * least until the next buffer modification, so immediate changes to the
     * slice will affect the result of future reads.
     * @param options Defaults to `{ copy: true }`
     */
    bytes(options?: { copy?: boolean }): Uint8Array;
    /** Returns whether the unread portion of the buffer is empty. */
    empty(): boolean;
    /** A read only number of bytes of the unread portion of the buffer. */
    readonly length: number;
    /** The read only capacity of the buffer's underlying byte slice, that is,
     * the total space allocated for the buffer's data. */
    readonly capacity: number;
    /** Discards all but the first `n` unread bytes from the buffer but
     * continues to use the same allocated storage. It throws if `n` is
     * negative or greater than the length of the buffer. */
    truncate(n: number): void;
    /** Resets the buffer to be empty, but it retains the underlying storage for
     * use by future writes. `.reset()` is the same as `.truncate(0)`. */
    reset(): void;
    /** Reads the next `p.length` bytes from the buffer or until the buffer is
     * drained. Returns the number of bytes read. If the buffer has no data to
     * return, the return is EOF (`null`). */
    readSync(p: Uint8Array): number | null;
    /** Reads the next `p.length` bytes from the buffer or until the buffer is
     * drained. Resolves to the number of bytes read. If the buffer has no
     * data to return, resolves to EOF (`null`).
     *
     * NOTE: This methods reads bytes synchronously; it's provided for
     * compatibility with `Reader` interfaces.
     */
    read(p: Uint8Array): Promise<number | null>;
    writeSync(p: Uint8Array): number;
    /** NOTE: This methods writes bytes synchronously; it's provided for
     * compatibility with `Writer` interface. */
    write(p: Uint8Array): Promise<number>;
    /** Grows the buffer's capacity, if necessary, to guarantee space for
     * another `n` bytes. After `.grow(n)`, at least `n` bytes can be written to
     * the buffer without another allocation. If `n` is negative, `.grow()` will
     * throw. If the buffer can't grow it will throw an error.
     *
     * Based on Go Lang's
     * [Buffer.Grow](https://golang.org/pkg/bytes/#Buffer.Grow). */
    grow(n: number): void;
    /** Reads data from `r` until EOF (`null`) and appends it to the buffer,
     * growing the buffer as needed. It resolves to the number of bytes read.
     * If the buffer becomes too large, `.readFrom()` will reject with an error.
     *
     * Based on Go Lang's
     * [Buffer.ReadFrom](https://golang.org/pkg/bytes/#Buffer.ReadFrom). */
    readFrom(r: Reader): Promise<number>;
    /** Reads data from `r` until EOF (`null`) and appends it to the buffer,
     * growing the buffer as needed. It returns the number of bytes read. If the
     * buffer becomes too large, `.readFromSync()` will throw an error.
     *
     * Based on Go Lang's
     * [Buffer.ReadFrom](https://golang.org/pkg/bytes/#Buffer.ReadFrom). */
    readFromSync(r: ReaderSync): number;
  }

  /**
   * Read Reader `r` until EOF (`null`) and resolve to the content as
   * Uint8Array`.
   *
   * ```ts
   * // Example from stdin
   * const stdinContent = await Deno.readAll(Deno.stdin);
   *
   * // Example from file
   * const file = await Deno.open("my_file.txt", {read: true});
   * const myFileContent = await Deno.readAll(file);
   * Deno.close(file.rid);
   *
   * // Example from buffer
   * const myData = new Uint8Array(100);
   * // ... fill myData array with data
   * const reader = new Deno.Buffer(myData.buffer as ArrayBuffer);
   * const bufferContent = await Deno.readAll(reader);
   * ```
   *
   * @deprecated Use `readAll` from https://deno.land/std/streams/conversion.ts
   * instead. `Deno.readAll` will be removed in Deno 2.0.
   *
   * @category I/O
   */
  export function readAll(r: Reader): Promise<Uint8Array>;

  /**
   * Synchronously reads Reader `r` until EOF (`null`) and returns the content
   * as `Uint8Array`.
   *
   * ```ts
   * // Example from stdin
   * const stdinContent = Deno.readAllSync(Deno.stdin);
   *
   * // Example from file
   * const file = Deno.openSync("my_file.txt", {read: true});
   * const myFileContent = Deno.readAllSync(file);
   * Deno.close(file.rid);
   *
   * // Example from buffer
   * const myData = new Uint8Array(100);
   * // ... fill myData array with data
   * const reader = new Deno.Buffer(myData.buffer as ArrayBuffer);
   * const bufferContent = Deno.readAllSync(reader);
   * ```
   *
   * @deprecated Use `readAllSync` from
   * https://deno.land/std/streams/conversion.ts instead. `Deno.readAllSync`
   * will be removed in Deno 2.0.
   *
   * @category I/O
   */
  export function readAllSync(r: ReaderSync): Uint8Array;

  /**
   * Write all the content of the array buffer (`arr`) to the writer (`w`).
   *
   * ```ts
   * // Example writing to stdout
   * const contentBytes = new TextEncoder().encode("Hello World");
   * await Deno.writeAll(Deno.stdout, contentBytes);
   * ```
   *
   * ```ts
   * // Example writing to file
   * const contentBytes = new TextEncoder().encode("Hello World");
   * const file = await Deno.open('test.file', {write: true});
   * await Deno.writeAll(file, contentBytes);
   * Deno.close(file.rid);
   * ```
   *
   * ```ts
   * // Example writing to buffer
   * const contentBytes = new TextEncoder().encode("Hello World");
   * const writer = new Deno.Buffer();
   * await Deno.writeAll(writer, contentBytes);
   * console.log(writer.bytes().length);  // 11
   * ```
   *
   * @deprecated Use `writeAll` from https://deno.land/std/streams/conversion.ts
   * instead. `Deno.writeAll` will be removed in Deno 2.0.
   *
   * @category I/O
   */
  export function writeAll(w: Writer, arr: Uint8Array): Promise<void>;

  /**
   * Synchronously write all the content of the array buffer (`arr`) to the
   * writer (`w`).
   *
   * ```ts
   * // Example writing to stdout
   * const contentBytes = new TextEncoder().encode("Hello World");
   * Deno.writeAllSync(Deno.stdout, contentBytes);
   * ```
   *
   * ```ts
   * // Example writing to file
   * const contentBytes = new TextEncoder().encode("Hello World");
   * const file = Deno.openSync('test.file', {write: true});
   * Deno.writeAllSync(file, contentBytes);
   * Deno.close(file.rid);
   * ```
   *
   * ```ts
   * // Example writing to buffer
   * const contentBytes = new TextEncoder().encode("Hello World");
   * const writer = new Deno.Buffer();
   * Deno.writeAllSync(writer, contentBytes);
   * console.log(writer.bytes().length);  // 11
   * ```
   *
   * @deprecated Use `writeAllSync` from
   * https://deno.land/std/streams/conversion.ts instead. `Deno.writeAllSync`
   * will be removed in Deno 2.0.
   *
   * @category I/O
   */
  export function writeAllSync(w: WriterSync, arr: Uint8Array): void;

  /** @category File System */
  export interface MkdirOptions {
    /** Defaults to `false`. If set to `true`, means that any intermediate
     * directories will also be created (as with the shell command `mkdir -p`).
     * Intermediate directories are created with the same permissions.
     * When recursive is set to `true`, succeeds silently (without changing any
     * permissions) if a directory already exists at the path, or if the path
     * is a symlink to an existing directory. */
    recursive?: boolean;
    /** Permissions to use when creating the directory (defaults to `0o777`,
     * before the process's umask).
     * Ignored on Windows. */
    mode?: number;
  }

  /** Synchronously creates a new directory with the specified path.
   *
   * ```ts
   * Deno.mkdirSync("new_dir");
   * Deno.mkdirSync("nested/directories", { recursive: true });
   * Deno.mkdirSync("restricted_access_dir", { mode: 0o700 });
   * ```
   *
   * Defaults to throwing error if the directory already exists.
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function mkdirSync(path: string | URL, options?: MkdirOptions): void;

  /** Creates a new directory with the specified path.
   *
   * ```ts
   * await Deno.mkdir("new_dir");
   * await Deno.mkdir("nested/directories", { recursive: true });
   * await Deno.mkdir("restricted_access_dir", { mode: 0o700 });
   * ```
   *
   * Defaults to throwing error if the directory already exists.
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function mkdir(
    path: string | URL,
    options?: MkdirOptions,
  ): Promise<void>;

  /** @category File System */
  export interface MakeTempOptions {
    /** Directory where the temporary directory should be created (defaults to
     * the env variable TMPDIR, or the system's default, usually /tmp).
     *
     * Note that if the passed `dir` is relative, the path returned by
     * makeTempFile() and makeTempDir() will also be relative. Be mindful of
     * this when changing working directory. */
    dir?: string;
    /** String that should precede the random portion of the temporary
     * directory's name. */
    prefix?: string;
    /** String that should follow the random portion of the temporary
     * directory's name. */
    suffix?: string;
  }

  /** Synchronously creates a new temporary directory in the default directory
   * for temporary files, unless `dir` is specified. Other optional options
   * include prefixing and suffixing the directory name with `prefix` and
   * `suffix` respectively.
   *
   * The full path to the newly created directory is returned.
   *
   * Multiple programs calling this function simultaneously will create different
   * directories. It is the caller's responsibility to remove the directory when
   * no longer needed.
   *
   * ```ts
   * const tempDirName0 = Deno.makeTempDirSync();  // e.g. /tmp/2894ea76
   * const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });  // e.g. /tmp/my_temp339c944d
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDirSync(options?: MakeTempOptions): string;

  /** Creates a new temporary directory in the default directory for temporary
   * files, unless `dir` is specified. Other optional options include
   * prefixing and suffixing the directory name with `prefix` and `suffix`
   * respectively.
   *
   * This call resolves to the full path to the newly created directory.
   *
   * Multiple programs calling this function simultaneously will create different
   * directories. It is the caller's responsibility to remove the directory when
   * no longer needed.
   *
   * ```ts
   * const tempDirName0 = await Deno.makeTempDir();  // e.g. /tmp/2894ea76
   * const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' }); // e.g. /tmp/my_temp339c944d
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDir(options?: MakeTempOptions): Promise<string>;

  /** Synchronously creates a new temporary file in the default directory for
   * temporary files, unless `dir` is specified.
   * Other optional options include prefixing and suffixing the directory name
   * with `prefix` and `suffix` respectively.
   *
   * The full path to the newly created file is returned.
   *
   * Multiple programs calling this function simultaneously will create different
   * files. It is the caller's responsibility to remove the file when no longer
   * needed.
   *
   * ```ts
   * const tempFileName0 = Deno.makeTempFileSync(); // e.g. /tmp/419e0bf2
   * const tempFileName1 = Deno.makeTempFileSync({ prefix: 'my_temp' });  // e.g. /tmp/my_temp754d3098
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function makeTempFileSync(options?: MakeTempOptions): string;

  /** Creates a new temporary file in the default directory for temporary
   * files, unless `dir` is specified.  Other
   * optional options include prefixing and suffixing the directory name with
   * `prefix` and `suffix` respectively.
   *
   * This call resolves to the full path to the newly created file.
   *
   * Multiple programs calling this function simultaneously will create different
   * files. It is the caller's responsibility to remove the file when no longer
   * needed.
   *
   * ```ts
   * const tmpFileName0 = await Deno.makeTempFile();  // e.g. /tmp/419e0bf2
   * const tmpFileName1 = await Deno.makeTempFile({ prefix: 'my_temp' });  // e.g. /tmp/my_temp754d3098
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function makeTempFile(options?: MakeTempOptions): Promise<string>;

  /** Synchronously changes the permission of a specific file/directory of
   * specified path.  Ignores the process's umask.
   *
   * ```ts
   * Deno.chmodSync("/path/to/file", 0o666);
   * ```
   *
   * For a full description, see [chmod](#Deno.chmod)
   *
   * NOTE: This API currently throws on Windows
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function chmodSync(path: string | URL, mode: number): void;

  /** Changes the permission of a specific file/directory of specified path.
   * Ignores the process's umask.
   *
   * ```ts
   * await Deno.chmod("/path/to/file", 0o666);
   * ```
   *
   * The mode is a sequence of 3 octal numbers.  The first/left-most number
   * specifies the permissions for the owner.  The second number specifies the
   * permissions for the group. The last/right-most number specifies the
   * permissions for others.  For example, with a mode of 0o764, the owner (7) can
   * read/write/execute, the group (6) can read/write and everyone else (4) can
   * read only.
   *
   * | Number | Description |
   * | ------ | ----------- |
   * | 7      | read, write, and execute |
   * | 6      | read and write |
   * | 5      | read and execute |
   * | 4      | read only |
   * | 3      | write and execute |
   * | 2      | write only |
   * | 1      | execute only |
   * | 0      | no permission |
   *
   * NOTE: This API currently throws on Windows
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function chmod(path: string | URL, mode: number): Promise<void>;

  /** Synchronously change owner of a regular file or directory. This functionality
   * is not available on Windows.
   *
   * ```ts
   * Deno.chownSync("myFile.txt", 1000, 1002);
   * ```
   *
   * Requires `allow-write` permission.
   *
   * Throws Error (not implemented) if executed on Windows
   *
   * @tags allow-write
   * @category File System
   *
   * @param path path to the file
   * @param uid user id (UID) of the new owner, or `null` for no change
   * @param gid group id (GID) of the new owner, or `null` for no change
   */
  export function chownSync(
    path: string | URL,
    uid: number | null,
    gid: number | null,
  ): void;

  /** Change owner of a regular file or directory. This functionality
   * is not available on Windows.
   *
   * ```ts
   * await Deno.chown("myFile.txt", 1000, 1002);
   * ```
   *
   * Requires `allow-write` permission.
   *
   * Throws Error (not implemented) if executed on Windows
   *
   * @tags allow-write
   * @category File System
   *
   * @param path path to the file
   * @param uid user id (UID) of the new owner, or `null` for no change
   * @param gid group id (GID) of the new owner, or `null` for no change
   */
  export function chown(
    path: string | URL,
    uid: number | null,
    gid: number | null,
  ): Promise<void>;

  /** @category File System */
  export interface RemoveOptions {
    /** Defaults to `false`. If set to `true`, path will be removed even if
     * it's a non-empty directory. */
    recursive?: boolean;
  }

  /** Synchronously removes the named file or directory.
   *
   * ```ts
   * Deno.removeSync("/path/to/empty_dir/or/file");
   * Deno.removeSync("/path/to/populated_dir/or/file", { recursive: true });
   * ```
   *
   * Throws error if permission denied, path not found, or path is a non-empty
   * directory and the `recursive` option isn't set to `true`.
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function removeSync(path: string | URL, options?: RemoveOptions): void;

  /** Removes the named file or directory.
   *
   * ```ts
   * await Deno.remove("/path/to/empty_dir/or/file");
   * await Deno.remove("/path/to/populated_dir/or/file", { recursive: true });
   * ```
   *
   * Throws error if permission denied, path not found, or path is a non-empty
   * directory and the `recursive` option isn't set to `true`.
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function remove(
    path: string | URL,
    options?: RemoveOptions,
  ): Promise<void>;

  /** Synchronously renames (moves) `oldpath` to `newpath`. Paths may be files or
   * directories.  If `newpath` already exists and is not a directory,
   * `renameSync()` replaces it. OS-specific restrictions may apply when
   * `oldpath` and `newpath` are in different directories.
   *
   * ```ts
   * Deno.renameSync("old/path", "new/path");
   * ```
   *
   * On Unix, this operation does not follow symlinks at either path.
   *
   * It varies between platforms when the operation throws errors, and if so what
   * they are. It's always an error to rename anything to a non-empty directory.
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function renameSync(
    oldpath: string | URL,
    newpath: string | URL,
  ): void;

  /** Renames (moves) `oldpath` to `newpath`.  Paths may be files or directories.
   * If `newpath` already exists and is not a directory, `rename()` replaces it.
   * OS-specific restrictions may apply when `oldpath` and `newpath` are in
   * different directories.
   *
   * ```ts
   * await Deno.rename("old/path", "new/path");
   * ```
   *
   * On Unix, this operation does not follow symlinks at either path.
   *
   * It varies between platforms when the operation throws errors, and if so what
   * they are. It's always an error to rename anything to a non-empty directory.
   *
   * Requires `allow-read` and `allow-write` permission.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function rename(
    oldpath: string | URL,
    newpath: string | URL,
  ): Promise<void>;

  /** Synchronously reads and returns the entire contents of a file as utf8
   *  encoded string. Reading a directory throws an error.
   *
   * ```ts
   * const data = Deno.readTextFileSync("hello.txt");
   * console.log(data);
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readTextFileSync(path: string | URL): string;

  /** Asynchronously reads and returns the entire contents of a file as utf8
   *  encoded string. Reading a directory throws an error.
   *
   * ```ts
   * const data = await Deno.readTextFile("hello.txt");
   * console.log(data);
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readTextFile(
    path: string | URL,
    options?: ReadFileOptions,
  ): Promise<string>;

  /** Synchronously reads and returns the entire contents of a file as an array
   * of bytes. `TextDecoder` can be used to transform the bytes to string if
   * required.  Reading a directory returns an empty data array.
   *
   * ```ts
   * const decoder = new TextDecoder("utf-8");
   * const data = Deno.readFileSync("hello.txt");
   * console.log(decoder.decode(data));
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readFileSync(path: string | URL): Uint8Array;

  /** Reads and resolves to the entire contents of a file as an array of bytes.
   * `TextDecoder` can be used to transform the bytes to string if required.
   * Reading a directory returns an empty data array.
   *
   * ```ts
   * const decoder = new TextDecoder("utf-8");
   * const data = await Deno.readFile("hello.txt");
   * console.log(decoder.decode(data));
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readFile(
    path: string | URL,
    options?: ReadFileOptions,
  ): Promise<Uint8Array>;

  /** A FileInfo describes a file and is returned by `stat`, `lstat`,
   * `statSync`, `lstatSync`.
   *
   * @category File System
   */
  export interface FileInfo {
    /** True if this is info for a regular file. Mutually exclusive to
     * `FileInfo.isDirectory` and `FileInfo.isSymlink`. */
    isFile: boolean;
    /** True if this is info for a regular directory. Mutually exclusive to
     * `FileInfo.isFile` and `FileInfo.isSymlink`. */
    isDirectory: boolean;
    /** True if this is info for a symlink. Mutually exclusive to
     * `FileInfo.isFile` and `FileInfo.isDirectory`. */
    isSymlink: boolean;
    /** The size of the file, in bytes. */
    size: number;
    /** The last modification time of the file. This corresponds to the `mtime`
     * field from `stat` on Linux/Mac OS and `ftLastWriteTime` on Windows. This
     * may not be available on all platforms. */
    mtime: Date | null;
    /** The last access time of the file. This corresponds to the `atime`
     * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
     * be available on all platforms. */
    atime: Date | null;
    /** The creation time of the file. This corresponds to the `birthtime`
     * field from `stat` on Mac/BSD and `ftCreationTime` on Windows. This may
     * not be available on all platforms. */
    birthtime: Date | null;
    /** ID of the device containing the file.
     *
     * _Linux/Mac OS only._ */
    dev: number | null;
    /** Inode number.
     *
     * _Linux/Mac OS only._ */
    ino: number | null;
    /** **UNSTABLE**: Match behavior with Go on Windows for `mode`.
     *
     * The underlying raw `st_mode` bits that contain the standard Unix
     * permissions for this file/directory. */
    mode: number | null;
    /** Number of hard links pointing to this file.
     *
     * _Linux/Mac OS only._ */
    nlink: number | null;
    /** User ID of the owner of this file.
     *
     * _Linux/Mac OS only._ */
    uid: number | null;
    /** Group ID of the owner of this file.
     *
     * _Linux/Mac OS only._ */
    gid: number | null;
    /** Device ID of this file.
     *
     * _Linux/Mac OS only._ */
    rdev: number | null;
    /** Blocksize for filesystem I/O.
     *
     * _Linux/Mac OS only._ */
    blksize: number | null;
    /** Number of blocks allocated to the file, in 512-byte units.
     *
     * _Linux/Mac OS only._ */
    blocks: number | null;
  }

  /** Returns absolute normalized path, with symbolic links resolved.
   *
   * ```ts
   * // e.g. given /home/alice/file.txt and current directory /home/alice
   * Deno.symlinkSync("file.txt", "symlink_file.txt");
   * const realPath = Deno.realPathSync("./file.txt");
   * const realSymLinkPath = Deno.realPathSync("./symlink_file.txt");
   * console.log(realPath);  // outputs "/home/alice/file.txt"
   * console.log(realSymLinkPath);  // outputs "/home/alice/file.txt"
   * ```
   *
   * Requires `allow-read` permission for the target path.
   * Also requires `allow-read` permission for the CWD if the target path is
   * relative.
   *
   * @tags allow-read
   * @category File System
   */
  export function realPathSync(path: string | URL): string;

  /** Resolves to the absolute normalized path, with symbolic links resolved.
   *
   * ```ts
   * // e.g. given /home/alice/file.txt and current directory /home/alice
   * await Deno.symlink("file.txt", "symlink_file.txt");
   * const realPath = await Deno.realPath("./file.txt");
   * const realSymLinkPath = await Deno.realPath("./symlink_file.txt");
   * console.log(realPath);  // outputs "/home/alice/file.txt"
   * console.log(realSymLinkPath);  // outputs "/home/alice/file.txt"
   * ```
   *
   * Requires `allow-read` permission for the target path.
   * Also requires `allow-read` permission for the CWD if the target path is
   * relative.
   *
   * @tags allow-read
   * @category File System
   */
  export function realPath(path: string | URL): Promise<string>;

  /** @category File System */
  export interface DirEntry {
    name: string;
    isFile: boolean;
    isDirectory: boolean;
    isSymlink: boolean;
  }

  /** Synchronously reads the directory given by `path` and returns an iterable
   * of `Deno.DirEntry`.
   *
   * ```ts
   * for (const dirEntry of Deno.readDirSync("/")) {
   *   console.log(dirEntry.name);
   * }
   * ```
   *
   * Throws error if `path` is not a directory.
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readDirSync(path: string | URL): Iterable<DirEntry>;

  /** Reads the directory given by `path` and returns an async iterable of
   * `Deno.DirEntry`.
   *
   * ```ts
   * for await (const dirEntry of Deno.readDir("/")) {
   *   console.log(dirEntry.name);
   * }
   * ```
   *
   * Throws error if `path` is not a directory.
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readDir(path: string | URL): AsyncIterable<DirEntry>;

  /** Synchronously copies the contents and permissions of one file to another
   * specified path, by default creating a new file if needed, else overwriting.
   * Fails if target path is a directory or is unwritable.
   *
   * ```ts
   * Deno.copyFileSync("from.txt", "to.txt");
   * ```
   *
   * Requires `allow-read` permission on fromPath.
   * Requires `allow-write` permission on toPath.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function copyFileSync(
    fromPath: string | URL,
    toPath: string | URL,
  ): void;

  /** Copies the contents and permissions of one file to another specified path,
   * by default creating a new file if needed, else overwriting. Fails if target
   * path is a directory or is unwritable.
   *
   * ```ts
   * await Deno.copyFile("from.txt", "to.txt");
   * ```
   *
   * Requires `allow-read` permission on fromPath.
   * Requires `allow-write` permission on toPath.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function copyFile(
    fromPath: string | URL,
    toPath: string | URL,
  ): Promise<void>;

  /** Returns the full path destination of the named symbolic link.
   *
   * ```ts
   * Deno.symlinkSync("./test.txt", "./test_link.txt");
   * const target = Deno.readLinkSync("./test_link.txt"); // full path of ./test.txt
   * ```
   *
   * Throws TypeError if called with a hard link
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readLinkSync(path: string | URL): string;

  /** Resolves to the full path destination of the named symbolic link.
   *
   * ```ts
   * await Deno.symlink("./test.txt", "./test_link.txt");
   * const target = await Deno.readLink("./test_link.txt"); // full path of ./test.txt
   * ```
   *
   * Throws TypeError if called with a hard link
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readLink(path: string | URL): Promise<string>;

  /** Resolves to a `Deno.FileInfo` for the specified `path`. If `path` is a
   * symlink, information for the symlink will be returned instead of what it
   * points to.
   *
   * ```ts
   * import { assert } from "https://deno.land/std/testing/asserts.ts";
   * const fileInfo = await Deno.lstat("hello.txt");
   * assert(fileInfo.isFile);
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function lstat(path: string | URL): Promise<FileInfo>;

  /** Synchronously returns a `Deno.FileInfo` for the specified `path`. If
   * `path` is a symlink, information for the symlink will be returned instead of
   * what it points to..
   *
   * ```ts
   * import { assert } from "https://deno.land/std/testing/asserts.ts";
   * const fileInfo = Deno.lstatSync("hello.txt");
   * assert(fileInfo.isFile);
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function lstatSync(path: string | URL): FileInfo;

  /** Resolves to a `Deno.FileInfo` for the specified `path`. Will always
   * follow symlinks.
   *
   * ```ts
   * import { assert } from "https://deno.land/std/testing/asserts.ts";
   * const fileInfo = await Deno.stat("hello.txt");
   * assert(fileInfo.isFile);
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function stat(path: string | URL): Promise<FileInfo>;

  /** Synchronously returns a `Deno.FileInfo` for the specified `path`. Will
   * always follow symlinks.
   *
   * ```ts
   * import { assert } from "https://deno.land/std/testing/asserts.ts";
   * const fileInfo = Deno.statSync("hello.txt");
   * assert(fileInfo.isFile);
   * ```
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function statSync(path: string | URL): FileInfo;

  /** Options for writing to a file.
   *
   * @category File System
   */
  export interface WriteFileOptions {
    /** Defaults to `false`. If set to `true`, will append to a file instead of
     * overwriting previous contents. */
    append?: boolean;
    /** Sets the option to allow creating a new file, if one doesn't already
     * exist at the specified path (defaults to `true`). */
    create?: boolean;
    /** Permissions always applied to file. */
    mode?: number;
    /**
     * An abort signal to allow cancellation of the file write operation.
     * If the signal becomes aborted the writeFile operation will be stopped
     * and the promise returned will be rejected with an AbortError.
     */
    signal?: AbortSignal;
  }

  /** Synchronously write `data` to the given `path`, by default creating a new
   * file if needed, else overwriting.
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world\n");
   * Deno.writeFileSync("hello1.txt", data);  // overwrite "hello1.txt" or create it
   * Deno.writeFileSync("hello2.txt", data, {create: false});  // only works if "hello2.txt" exists
   * Deno.writeFileSync("hello3.txt", data, {mode: 0o777});  // set permissions on new file
   * Deno.writeFileSync("hello4.txt", data, {append: true});  // add data to the end of the file
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is
   * `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeFileSync(
    path: string | URL,
    data: Uint8Array,
    options?: WriteFileOptions,
  ): void;

  /** Write `data` to the given `path`, by default creating a new file if needed,
   * else overwriting.
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world\n");
   * await Deno.writeFile("hello1.txt", data);  // overwrite "hello1.txt" or create it
   * await Deno.writeFile("hello2.txt", data, {create: false});  // only works if "hello2.txt" exists
   * await Deno.writeFile("hello3.txt", data, {mode: 0o777});  // set permissions on new file
   * await Deno.writeFile("hello4.txt", data, {append: true});  // add data to the end of the file
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeFile(
    path: string | URL,
    data: Uint8Array,
    options?: WriteFileOptions,
  ): Promise<void>;

  /** Synchronously write string `data` to the given `path`, by default creating a new file if needed,
   * else overwriting.
   *
   * ```ts
   * Deno.writeTextFileSync("hello1.txt", "Hello world\n");  // overwrite "hello1.txt" or create it
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeTextFileSync(
    path: string | URL,
    data: string,
    options?: WriteFileOptions,
  ): void;

  /** Asynchronously write string `data` to the given `path`, by default creating a new file if needed,
   * else overwriting.
   *
   * ```ts
   * await Deno.writeTextFile("hello1.txt", "Hello world\n");  // overwrite "hello1.txt" or create it
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeTextFile(
    path: string | URL,
    data: string,
    options?: WriteFileOptions,
  ): Promise<void>;

  /** Synchronously truncates or extends the specified file, to reach the
   * specified `len`.  If `len` is not specified then the entire file contents
   * are truncated.
   *
   * ```ts
   * // truncate the entire file
   * Deno.truncateSync("my_file.txt");
   *
   * // truncate part of the file
   * const file = Deno.makeTempFileSync();
   * Deno.writeFileSync(file, new TextEncoder().encode("Hello World"));
   * Deno.truncateSync(file, 7);
   * const data = Deno.readFileSync(file);
   * console.log(new TextDecoder().decode(data));
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function truncateSync(name: string, len?: number): void;

  /** Truncates or extends the specified file, to reach the specified `len`. If
   * `len` is not specified then the entire file contents are truncated.
   *
   * ```ts
   * // truncate the entire file
   * await Deno.truncate("my_file.txt");
   *
   * // truncate part of the file
   * const file = await Deno.makeTempFile();
   * await Deno.writeFile(file, new TextEncoder().encode("Hello World"));
   * await Deno.truncate(file, 7);
   * const data = await Deno.readFile(file);
   * console.log(new TextDecoder().decode(data));  // "Hello W"
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function truncate(name: string, len?: number): Promise<void>;

  /** @category Observability */
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

  /** @category Observability */
  export interface Metrics extends OpMetrics {
    ops: Record<string, OpMetrics>;
  }

  /** Receive metrics from the privileged side of Deno. This is primarily used
   * in the development of Deno. 'Ops', also called 'bindings', are the go-between
   * between Deno JavaScript and Deno Rust.
   *
   *      > console.table(Deno.metrics())
   *      ┌─────────────────────────┬────────┐
   *      │         (index)         │ Values │
   *      ├─────────────────────────┼────────┤
   *      │      opsDispatched      │   3    │
   *      │    opsDispatchedSync    │   2    │
   *      │   opsDispatchedAsync    │   1    │
   *      │ opsDispatchedAsyncUnref │   0    │
   *      │      opsCompleted       │   3    │
   *      │    opsCompletedSync     │   2    │
   *      │    opsCompletedAsync    │   1    │
   *      │ opsCompletedAsyncUnref  │   0    │
   *      │    bytesSentControl     │   73   │
   *      │      bytesSentData      │   0    │
   *      │      bytesReceived      │  375   │
   *      └─────────────────────────┴────────┘
   *
   * @category Observability
   */
  export function metrics(): Metrics;

  /** @category Observability */
  interface ResourceMap {
    // deno-lint-ignore no-explicit-any
    [rid: number]: any;
  }

  /** Returns a map of open resource ids (rid) along with their string
   * representations. This is an internal API and as such resource
   * representation has `any` type; that means it can change any time.
   *
   * ```ts
   * console.log(Deno.resources());
   * // { 0: "stdin", 1: "stdout", 2: "stderr" }
   * Deno.openSync('../test.file');
   * console.log(Deno.resources());
   * // { 0: "stdin", 1: "stdout", 2: "stderr", 3: "fsFile" }
   * ```
   *
   * @category Observability
   */
  export function resources(): ResourceMap;

  /**
   * Additional information for FsEvent objects with the "other" kind.
   *
   * - "rescan": rescan notices indicate either a lapse in the events or a
   *    change in the filesystem such that events received so far can no longer
   *    be relied on to represent the state of the filesystem now. An
   *    application that simply reacts to file changes may not care about this.
   *    An application that keeps an in-memory representation of the filesystem
   *    will need to care, and will need to refresh that representation directly
   *    from the filesystem.
   *
   * @category File System
   */
  export type FsEventFlag = "rescan";

  /** @category File System */
  export interface FsEvent {
    kind: "any" | "access" | "create" | "modify" | "remove" | "other";
    paths: string[];
    flag?: FsEventFlag;
  }

  /**
   * FsWatcher is returned by `Deno.watchFs` function when you start watching
   * the file system. You can iterate over this interface to get the file
   * system events, and also you can stop watching the file system by calling
   * `.close()` method.
   *
   * @category File System
   */
  export interface FsWatcher extends AsyncIterable<FsEvent> {
    /** The resource id of the `FsWatcher`. */
    readonly rid: number;
    /** Stops watching the file system and closes the watcher resource. */
    close(): void;
    /**
     * Stops watching the file system and closes the watcher resource.
     *
     * @deprecated Will be removed at 2.0.
     */
    return?(value?: any): Promise<IteratorResult<FsEvent>>;
    [Symbol.asyncIterator](): AsyncIterableIterator<FsEvent>;
  }

  /** Watch for file system events against one or more `paths`, which can be files
   * or directories.  These paths must exist already.  One user action (e.g.
   * `touch test.file`) can  generate multiple file system events.  Likewise,
   * one user action can result in multiple file paths in one event (e.g. `mv
   * old_name.txt new_name.txt`).  Recursive option is `true` by default and,
   * for directories, will watch the specified directory and all sub directories.
   * Note that the exact ordering of the events can vary between operating systems.
   *
   * ```ts
   * const watcher = Deno.watchFs("/");
   * for await (const event of watcher) {
   *    console.log(">>>> event", event);
   *    // { kind: "create", paths: [ "/foo.txt" ] }
   * }
   * ```
   *
   * Requires `allow-read` permission.
   *
   * Call `watcher.close()` to stop watching.
   *
   * ```ts
   * const watcher = Deno.watchFs("/");
   *
   * setTimeout(() => {
   *   watcher.close();
   * }, 5000);
   *
   * for await (const event of watcher) {
   *    console.log(">>>> event", event);
   * }
   * ```
   *
   * @tags allow-read
   * @category File System
   */
  export function watchFs(
    paths: string | string[],
    options?: { recursive: boolean },
  ): FsWatcher;

  /** @category Sub Process */
  export class Process<T extends RunOptions = RunOptions> {
    readonly rid: number;
    readonly pid: number;
    readonly stdin: T["stdin"] extends "piped" ? Writer & Closer & {
        writable: WritableStream<Uint8Array>;
      }
      : (Writer & Closer & { writable: WritableStream<Uint8Array> }) | null;
    readonly stdout: T["stdout"] extends "piped" ? Reader & Closer & {
        readable: ReadableStream<Uint8Array>;
      }
      : (Reader & Closer & { readable: ReadableStream<Uint8Array> }) | null;
    readonly stderr: T["stderr"] extends "piped" ? Reader & Closer & {
        readable: ReadableStream<Uint8Array>;
      }
      : (Reader & Closer & { readable: ReadableStream<Uint8Array> }) | null;
    /** Wait for the process to exit and return its exit status.
     *
     * Calling this function multiple times will return the same status.
     *
     * Stdin handle to the process will be closed before waiting to avoid
     * a deadlock.
     *
     * If `stdout` and/or `stderr` were set to `"piped"`, they must be closed
     * manually before the process can exit.
     *
     * To run process to completion and collect output from both `stdout` and
     * `stderr` use:
     *
     * ```ts
     * const p = Deno.run({ cmd: [ "echo", "hello world" ], stderr: 'piped', stdout: 'piped' });
     * const [status, stdout, stderr] = await Promise.all([
     *   p.status(),
     *   p.output(),
     *   p.stderrOutput()
     * ]);
     * p.close();
     * ```
     */
    status(): Promise<ProcessStatus>;
    /** Buffer the stdout until EOF and return it as `Uint8Array`.
     *
     * You must set stdout to `"piped"` when creating the process.
     *
     * This calls `close()` on stdout after its done. */
    output(): Promise<Uint8Array>;
    /** Buffer the stderr until EOF and return it as `Uint8Array`.
     *
     * You must set stderr to `"piped"` when creating the process.
     *
     * This calls `close()` on stderr after its done. */
    stderrOutput(): Promise<Uint8Array>;
    close(): void;

    /** Send a signal to process.
     *
     * ```ts
     * const p = Deno.run({ cmd: [ "sleep", "20" ]});
     * p.kill("SIGTERM");
     * p.close();
     * ```
     */
    kill(signo: Signal): void;
  }

  /** @category Runtime Environment */
  export type Signal =
    | "SIGABRT"
    | "SIGALRM"
    | "SIGBREAK"
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

  /** Registers the given function as a listener of the given signal event.
   *
   * ```ts
   * Deno.addSignalListener("SIGTERM", () => {
   *   console.log("SIGTERM!")
   * });
   * ```
   *
   * NOTE: On Windows only SIGINT (ctrl+c) and SIGBREAK (ctrl+break) are supported.
   *
   * @category Runtime Environment
   */
  export function addSignalListener(signal: Signal, handler: () => void): void;

  /** Removes the given signal listener that has been registered with
   * Deno.addSignalListener.
   *
   * ```ts
   * const listener = () => {
   *   console.log("SIGTERM!")
   * };
   * Deno.addSignalListener("SIGTERM", listener);
   * Deno.removeSignalListener("SIGTERM", listener);
   * ```
   *
   * NOTE: On Windows only SIGINT (ctrl+c) and SIGBREAK (ctrl+break) are supported.
   *
   * @category Runtime Environment
   */
  export function removeSignalListener(
    signal: Signal,
    handler: () => void,
  ): void;

  /** @category Sub Process */
  export type ProcessStatus =
    | {
      success: true;
      code: 0;
      signal?: undefined;
    }
    | {
      success: false;
      code: number;
      signal?: number;
    };

  /** @category Sub Process */
  export interface RunOptions {
    /** Arguments to pass. Note, the first element needs to be a path to the
     * binary */
    cmd: readonly string[] | [URL, ...string[]];
    cwd?: string;
    env?: {
      [key: string]: string;
    };
    stdout?: "inherit" | "piped" | "null" | number;
    stderr?: "inherit" | "piped" | "null" | number;
    stdin?: "inherit" | "piped" | "null" | number;
  }

  /** Spawns new subprocess. RunOptions must contain at a minimum the `opt.cmd`,
   * an array of program arguments, the first of which is the binary.
   *
   * ```ts
   * const p = Deno.run({
   *   cmd: ["curl", "https://example.com"],
   * });
   * const status = await p.status();
   * ```
   *
   * Subprocess uses same working directory as parent process unless `opt.cwd`
   * is specified.
   *
   * Environmental variables from parent process can be cleared using `opt.clearEnv`.
   * Doesn't guarantee that only `opt.env` variables are present,
   * as the OS may set environmental variables for processes.
   *
   * Environmental variables for subprocess can be specified using `opt.env`
   * mapping.
   *
   * `opt.uid` sets the child process’s user ID. This translates to a setuid call
   * in the child process. Failure in the setuid call will cause the spawn to fail.
   *
   * `opt.gid` is similar to `opt.uid`, but sets the group ID of the child process.
   * This has the same semantics as the uid field.
   *
   * By default subprocess inherits stdio of parent process. To change that
   * `opt.stdout`, `opt.stderr` and `opt.stdin` can be specified independently -
   * they can be set to either an rid of open file or set to "inherit" "piped"
   * or "null":
   *
   * `"inherit"` The default if unspecified. The child inherits from the
   * corresponding parent descriptor.
   *
   * `"piped"` A new pipe should be arranged to connect the parent and child
   * sub-processes.
   *
   * `"null"` This stream will be ignored. This is the equivalent of attaching
   * the stream to `/dev/null`.
   *
   * Details of the spawned process are returned.
   *
   * Requires `allow-run` permission.
   *
   * @tags allow-run
   * @category Sub Process
   */
  export function run<T extends RunOptions = RunOptions>(opt: T): Process<T>;

  /** @category Console and Debugging */
  export interface InspectOptions {
    /** Stylize output with ANSI colors. Defaults to false. */
    colors?: boolean;
    /** Try to fit more than one entry of a collection on the same line.
     * Defaults to true. */
    compact?: boolean;
    /** Traversal depth for nested objects. Defaults to 4. */
    depth?: number;
    /** The maximum number of iterable entries to print. Defaults to 100. */
    iterableLimit?: number;
    /** Show a Proxy's target and handler. Defaults to false. */
    showProxy?: boolean;
    /** Sort Object, Set and Map entries by key. Defaults to false. */
    sorted?: boolean;
    /** Add a trailing comma for multiline collections. Defaults to false. */
    trailingComma?: boolean;
    /*** Evaluate the result of calling getters. Defaults to false. */
    getters?: boolean;
    /** Show an object's non-enumerable properties. Defaults to false. */
    showHidden?: boolean;
    /** The maximum length of a string before it is truncated with an ellipsis */
    strAbbreviateSize?: number;
  }

  /** Converts the input into a string that has the same format as printed by
   * `console.log()`.
   *
   * ```ts
   * const obj = {
   *   a: 10,
   *   b: "hello",
   * };
   * const objAsString = Deno.inspect(obj); // { a: 10, b: "hello" }
   * console.log(obj);  // prints same value as objAsString, e.g. { a: 10, b: "hello" }
   * ```
   *
   * You can also register custom inspect functions, via the symbol `Symbol.for("Deno.customInspect")`,
   * on objects, to control and customize the output.
   *
   * ```ts
   * class A {
   *   x = 10;
   *   y = "hello";
   *   [Symbol.for("Deno.customInspect")](): string {
   *     return "x=" + this.x + ", y=" + this.y;
   *   }
   * }
   *
   * const inStringFormat = Deno.inspect(new A()); // "x=10, y=hello"
   * console.log(inStringFormat);  // prints "x=10, y=hello"
   * ```
   *
   * Finally, you can also specify the depth to which it will format.
   *
   * ```ts
   * Deno.inspect({a: {b: {c: {d: 'hello'}}}}, {depth: 2}); // { a: { b: [Object] } }
   * ```
   *
   * @category Console and Debugging
   */
  export function inspect(value: unknown, options?: InspectOptions): string;

  /** The name of a "powerful feature" which needs permission.
   *
   * @category Permissions
   */
  export type PermissionName =
    | "run"
    | "read"
    | "write"
    | "net"
    | "env"
    | "ffi"
    | "hrtime";

  /** The current status of the permission.
   *
   * @category Permissions
   */
  export type PermissionState = "granted" | "denied" | "prompt";

  /** @category Permissions */
  export interface RunPermissionDescriptor {
    name: "run";
    command?: string | URL;
  }

  /** @category Permissions */
  export interface ReadPermissionDescriptor {
    name: "read";
    path?: string | URL;
  }

  /** @category Permissions */
  export interface WritePermissionDescriptor {
    name: "write";
    path?: string | URL;
  }

  /** @category Permissions */
  export interface NetPermissionDescriptor {
    name: "net";
    /** Optional host string of the form `"<hostname>[:<port>]"`. Examples:
     *
     *      "github.com"
     *      "deno.land:8080"
     */
    host?: string;
  }

  /** @category Permissions */
  export interface EnvPermissionDescriptor {
    name: "env";
    variable?: string;
  }

  /** @category Permissions */
  export interface FfiPermissionDescriptor {
    name: "ffi";
    path?: string | URL;
  }

  /** @category Permissions */
  export interface HrtimePermissionDescriptor {
    name: "hrtime";
  }

  /** Permission descriptors which define a permission and can be queried,
   * requested, or revoked.
   *
   * @category Permissions
   */
  export type PermissionDescriptor =
    | RunPermissionDescriptor
    | ReadPermissionDescriptor
    | WritePermissionDescriptor
    | NetPermissionDescriptor
    | EnvPermissionDescriptor
    | FfiPermissionDescriptor
    | HrtimePermissionDescriptor;

  /** @category Permissions */
  export interface PermissionStatusEventMap {
    "change": Event;
  }

  /** @category Permissions */
  export class PermissionStatus extends EventTarget {
    // deno-lint-ignore no-explicit-any
    onchange: ((this: PermissionStatus, ev: Event) => any) | null;
    readonly state: PermissionState;
    addEventListener<K extends keyof PermissionStatusEventMap>(
      type: K,
      listener: (
        this: PermissionStatus,
        ev: PermissionStatusEventMap[K],
      ) => any,
      options?: boolean | AddEventListenerOptions,
    ): void;
    addEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | AddEventListenerOptions,
    ): void;
    removeEventListener<K extends keyof PermissionStatusEventMap>(
      type: K,
      listener: (
        this: PermissionStatus,
        ev: PermissionStatusEventMap[K],
      ) => any,
      options?: boolean | EventListenerOptions,
    ): void;
    removeEventListener(
      type: string,
      listener: EventListenerOrEventListenerObject,
      options?: boolean | EventListenerOptions,
    ): void;
  }

  /** @category Permissions */
  export class Permissions {
    /** Resolves to the current status of a permission.
     *
     * ```ts
     * const status = await Deno.permissions.query({ name: "read", path: "/etc" });
     * console.log(status.state);
     * ```
     */
    query(desc: PermissionDescriptor): Promise<PermissionStatus>;

    /** Revokes a permission, and resolves to the state of the permission.
     *
     * ```ts
     * import { assert } from "https://deno.land/std/testing/asserts.ts";
     *
     * const status = await Deno.permissions.revoke({ name: "run" });
     * assert(status.state !== "granted")
     * ```
     */
    revoke(desc: PermissionDescriptor): Promise<PermissionStatus>;

    /** Requests the permission, and resolves to the state of the permission.
     *
     * ```ts
     * const status = await Deno.permissions.request({ name: "env" });
     * if (status.state === "granted") {
     *   console.log("'env' permission is granted.");
     * } else {
     *   console.log("'env' permission is denied.");
     * }
     * ```
     */
    request(desc: PermissionDescriptor): Promise<PermissionStatus>;
  }

  /** Deno's permission management API.
   *
   * @category Permissions
   */
  export const permissions: Permissions;

  /** Build related information.
   *
   * @category Runtime Environment
   */
  export const build: {
    /** The LLVM target triple */
    target: string;
    /** Instruction set architecture */
    arch: "x86_64" | "aarch64";
    /** Operating system */
    os: "darwin" | "linux" | "windows";
    /** Computer vendor */
    vendor: string;
    /** Optional environment */
    env?: string;
  };

  /** Version related information.
   *
   * @category Runtime Environment
   */
  export const version: {
    /** Deno's version. For example: `"1.0.0"` */
    deno: string;
    /** The V8 version used by Deno. For example: `"8.0.0.0"` */
    v8: string;
    /** The TypeScript version used by Deno. For example: `"4.0.0"` */
    typescript: string;
  };

  /** Returns the script arguments to the program. If for example we run a
   * program:
   *
   * deno run --allow-read https://deno.land/std/examples/cat.ts /etc/passwd
   *
   * Then `Deno.args` will contain:
   *
   * [ "/etc/passwd" ]
   *
   * @category Runtime Environment
   */
  export const args: string[];

  /**
   * A symbol which can be used as a key for a custom method which will be
   * called when `Deno.inspect()` is called, or when the object is logged to
   * the console.
   *
   * @deprecated This symbol is deprecated since 1.9. Use
   * `Symbol.for("Deno.customInspect")` instead.
   *
   * @category Console and Debugging
   */
  export const customInspect: unique symbol;

  /** The URL of the entrypoint module entered from the command-line.
   *
   * @category Runtime Environment
   */
  export const mainModule: string;

  /** @category File System */
  export type SymlinkOptions = {
    type: "file" | "dir";
  };

  /**
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The options.type parameter can be set to `file` or `dir`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * ```ts
   * Deno.symlinkSync("old/name", "new/name");
   * ```
   *
   * Requires full `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function symlinkSync(
    oldpath: string | URL,
    newpath: string | URL,
    options?: SymlinkOptions,
  ): void;

  /**
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The options.type parameter can be set to `file` or `dir`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * ```ts
   * await Deno.symlink("old/name", "new/name");
   * ```
   *
   * Requires full `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function symlink(
    oldpath: string | URL,
    newpath: string | URL,
    options?: SymlinkOptions,
  ): Promise<void>;

  /**
   * Synchronously truncates or extends the specified file stream, to reach the
   * specified `len`.
   *
   * If `len` is not specified then the entire file contents are truncated as if len was set to 0.
   *
   * if the file previously was larger than this new length, the extra  data  is  lost.
   *
   * if  the  file  previously  was shorter, it is extended, and the extended part reads as null bytes ('\0').
   *
   * ```ts
   * // truncate the entire file
   * const file = Deno.openSync("my_file.txt", { read: true, write: true, truncate: true, create: true });
   * Deno.ftruncateSync(file.rid);
   * ```
   *
   * ```ts
   * // truncate part of the file
   * const file = Deno.openSync("my_file.txt", { read: true, write: true, create: true });
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.ftruncateSync(file.rid, 7);
   * Deno.seekSync(file.rid, 0, Deno.SeekMode.Start);
   * const data = new Uint8Array(32);
   * Deno.readSync(file.rid, data);
   * console.log(new TextDecoder().decode(data)); // Hello W
   * ```
   *
   * @category File System
   */
  export function ftruncateSync(rid: number, len?: number): void;

  /**
   * Truncates or extends the specified file stream, to reach the specified `len`.
   *
   * If `len` is not specified then the entire file contents are truncated as if len was set to 0.
   *
   * If the file previously was larger than this new length, the extra  data  is  lost.
   *
   * If  the  file  previously  was shorter, it is extended, and the extended part reads as null bytes ('\0').
   *
   * ```ts
   * // truncate the entire file
   * const file = await Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.ftruncate(file.rid);
   * ```
   *
   * ```ts
   * // truncate part of the file
   * const file = await Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.ftruncate(file.rid, 7);
   * const data = new Uint8Array(32);
   * await Deno.read(file.rid, data);
   * console.log(new TextDecoder().decode(data)); // Hello W
   * ```
   *
   * @category File System
   */
  export function ftruncate(rid: number, len?: number): Promise<void>;

  /**
   * Synchronously returns a `Deno.FileInfo` for the given file stream.
   *
   * ```ts
   * import { assert } from "https://deno.land/std/testing/asserts.ts";
   * const file = Deno.openSync("file.txt", { read: true });
   * const fileInfo = Deno.fstatSync(file.rid);
   * assert(fileInfo.isFile);
   * ```
   *
   * @category File System
   */
  export function fstatSync(rid: number): FileInfo;

  /**
   * Returns a `Deno.FileInfo` for the given file stream.
   *
   * ```ts
   * import { assert } from "https://deno.land/std/testing/asserts.ts";
   * const file = await Deno.open("file.txt", { read: true });
   * const fileInfo = await Deno.fstat(file.rid);
   * assert(fileInfo.isFile);
   * ```
   *
   * @category File System
   */
  export function fstat(rid: number): Promise<FileInfo>;

  /** @category HTTP Server */
  export interface RequestEvent {
    readonly request: Request;
    respondWith(r: Response | Promise<Response>): Promise<void>;
  }

  /** @category HTTP Server */
  export interface HttpConn extends AsyncIterable<RequestEvent> {
    readonly rid: number;

    nextRequest(): Promise<RequestEvent | null>;
    close(): void;
  }

  /**
   * Services HTTP requests given a TCP or TLS socket.
   *
   * ```ts
   * const conn = Deno.listen({ port: 80 });
   * const httpConn = Deno.serveHttp(await conn.accept());
   * const e = await httpConn.nextRequest();
   * if (e) {
   *   e.respondWith(new Response("Hello World"));
   * }
   * ```
   *
   * If `httpConn.nextRequest()` encounters an error or returns `null`
   * then the underlying HttpConn resource is closed automatically.
   *
   * Alternatively, you can also use the Async Iterator approach:
   *
   * ```ts
   * async function handleHttp(conn: Deno.Conn) {
   *   for await (const e of Deno.serveHttp(conn)) {
   *     e.respondWith(new Response("Hello World"));
   *   }
   * }
   *
   * for await (const conn of Deno.listen({ port: 80 })) {
   *   handleHttp(conn);
   * }
   * ```
   *
   * @category HTTP Server
   */
  export function serveHttp(conn: Conn): HttpConn;

  /** @category Web Sockets */
  export interface WebSocketUpgrade {
    response: Response;
    socket: WebSocket;
  }

  /** @category Web Sockets */
  export interface UpgradeWebSocketOptions {
    protocol?: string;
    /**
     * If the client does not respond to this frame with a
     * `pong` within the timeout specified, the connection is deemed
     * unhealthy and is closed. The `close` and `error` event will be emitted.
     *
     * The default is 120 seconds. Set to 0 to disable timeouts.
     */
    idleTimeout?: number;
  }

  /**
   * Used to upgrade an incoming HTTP request to a WebSocket.
   *
   * Given a request, returns a pair of WebSocket and Response. The original
   * request must be responded to with the returned response for the websocket
   * upgrade to be successful.
   *
   * ```ts
   * const conn = Deno.listen({ port: 80 });
   * const httpConn = Deno.serveHttp(await conn.accept());
   * const e = await httpConn.nextRequest();
   * if (e) {
   *   const { socket, response } = Deno.upgradeWebSocket(e.request);
   *   socket.onopen = () => {
   *     socket.send("Hello World!");
   *   };
   *   socket.onmessage = (e) => {
   *     console.log(e.data);
   *     socket.close();
   *   };
   *   socket.onclose = () => console.log("WebSocket has been closed.");
   *   socket.onerror = (e) => console.error("WebSocket error:", e);
   *   e.respondWith(response);
   * }
   * ```
   *
   * If the request body is disturbed (read from) before the upgrade is
   * completed, upgrading fails.
   *
   * This operation does not yet consume the request or open the websocket. This
   * only happens once the returned response has been passed to `respondWith`.
   *
   * @category Web Sockets
   */
  export function upgradeWebSocket(
    request: Request,
    options?: UpgradeWebSocketOptions,
  ): WebSocketUpgrade;

  /** Send a signal to process under given `pid`.
   *
   * If `pid` is negative, the signal will be sent to the process group
   * identified by `pid`. An error will be thrown if a negative
   * `pid` is used on Windows.
   *
   * ```ts
   * const p = Deno.run({
   *   cmd: ["sleep", "10000"]
   * });
   *
   * Deno.kill(p.pid, "SIGINT");
   * ```
   *
   * Requires `allow-run` permission.
   *
   * @tags allow-run
   * @category Sub Process
   */
  export function kill(pid: number, signo: Signal): void;

  /** The type of the resource record.
   * Only the listed types are supported currently.
   *
   * @category Network
   */
  export type RecordType =
    | "A"
    | "AAAA"
    | "ANAME"
    | "CAA"
    | "CNAME"
    | "MX"
    | "NAPTR"
    | "NS"
    | "PTR"
    | "SOA"
    | "SRV"
    | "TXT";

  /** @category Network */
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

  /** If `resolveDns` is called with "CAA" record type specified, it will return
   * an array of this interface.
   *
   * @category Network
   */
  export interface CAARecord {
    critical: boolean;
    tag: string;
    value: string;
  }

  /** If `resolveDns` is called with "MX" record type specified, it will return
   * an array of this interface.
   *
   * @category Network
   */
  export interface MXRecord {
    preference: number;
    exchange: string;
  }

  /** If `resolveDns` is called with "NAPTR" record type specified, it will
   * return an array of this interface.
   *
   * @category Network
   */
  export interface NAPTRRecord {
    order: number;
    preference: number;
    flags: string;
    services: string;
    regexp: string;
    replacement: string;
  }

  /** If `resolveDns` is called with "SOA" record type specified, it will return
   * an array of this interface.
   *
   * @category Network
   */
  export interface SOARecord {
    mname: string;
    rname: string;
    serial: number;
    refresh: number;
    retry: number;
    expire: number;
    minimum: number;
  }

  /** If `resolveDns` is called with "SRV" record type specified, it will return
   * an array of this interface.
   *
   * @category Network
   */
  export interface SRVRecord {
    priority: number;
    weight: number;
    port: number;
    target: string;
  }

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "A" | "AAAA" | "ANAME" | "CNAME" | "NS" | "PTR",
    options?: ResolveDnsOptions,
  ): Promise<string[]>;

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "CAA",
    options?: ResolveDnsOptions,
  ): Promise<CAARecord[]>;

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "MX",
    options?: ResolveDnsOptions,
  ): Promise<MXRecord[]>;

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "NAPTR",
    options?: ResolveDnsOptions,
  ): Promise<NAPTRRecord[]>;

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "SOA",
    options?: ResolveDnsOptions,
  ): Promise<SOARecord[]>;

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "SRV",
    options?: ResolveDnsOptions,
  ): Promise<SRVRecord[]>;

  /** @category Network */
  export function resolveDns(
    query: string,
    recordType: "TXT",
    options?: ResolveDnsOptions,
  ): Promise<string[][]>;

  /**
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
   *   nameServer: { ipAddr: "8.8.8.8", port: 53 },
   * });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function resolveDns(
    query: string,
    recordType: RecordType,
    options?: ResolveDnsOptions,
  ): Promise<
    | string[]
    | CAARecord[]
    | MXRecord[]
    | NAPTRRecord[]
    | SOARecord[]
    | SRVRecord[]
    | string[][]
  >;
}
