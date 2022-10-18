// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />
/// <reference lib="deno.net" />

/** Deno provides extra properties on `import.meta`. These are included here
 * to ensure that these are still available when using the Deno namespace in
 * conjunction with other type libs, like `dom`.
 *
 * @category ES Modules
 */
declare interface ImportMeta {
  /** A string representation of the fully qualified module URL. When the
   * module is loaded locally, the value will be a file URL (e.g.
   * `file:///path/module.ts`).
   *
   * You can also parse the string as a URL to determine more information about
   * how the current module was loaded. For example to determine if a module was
   * local or not:
   *
   * ```ts
   * const url = new URL(import.meta.url);
   * if (url.protocol === "file:") {
   *   console.log("this module was loaded locally");
   * }
   * ```
   */
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

/** Deno supports [User Timing Level 3](https://w3c.github.io/user-timing)
 * which is not widely supported yet in other runtimes.
 *
 * Check out the
 * [Performance API](https://developer.mozilla.org/en-US/docs/Web/API/Performance)
 * documentation on MDN for further information about how to use the API.
 *
 * @category Performance
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
 * Options which are used in conjunction with `performance.mark`. Check out the
 * MDN
 * [`performance.mark()`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/mark#markoptions)
 * documentation for more details.
 *
 * @category Performance
 */
declare interface PerformanceMarkOptions {
  /** Metadata to be included in the mark. */
  // deno-lint-ignore no-explicit-any
  detail?: any;

  /** Timestamp to be used as the mark time. */
  startTime?: number;
}

/**
 * Options which are used in conjunction with `performance.measure`. Check out the
 * MDN
 * [`performance.mark()`](https://developer.mozilla.org/en-US/docs/Web/API/Performance/measure#measureoptions)
 * documentation for more details.
 *
 * @category Performance
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

/** The global namespace where Deno specific, non-standard APIs are located. */
declare namespace Deno {
  /** A set of error constructors that are raised by Deno APIs.
   *
   * Can be used to provide more specific handling of failures within code
   * which is using Deno APIs. For example, handling attempting to open a file
   * which does not exist:
   *
   * ```ts
   * try {
   *   const file = await Deno.open("./some/file.txt");
   * } catch (error) {
   *   if (error instanceof Deno.errors.NotFound) {
   *     console.error("the file was not found");
   *   } else {
   *     // otherwise re-throw
   *     throw error;
   *   }
   * }
   * ```
   *
   * @category Errors
   */
  export namespace errors {
    /**
     * Raised when the underlying operating system indicates that the file
     * was not found.
     *
     * @category Errors */
    export class NotFound extends Error {}
    /**
     * Raised when the underlying operating system indicates the current user
     * which the Deno process is running under does not have the appropriate
     * permissions to a file or resource, or the user _did not_ provide required
     * `--allow-*` flag.
     *
     * @category Errors */
    export class PermissionDenied extends Error {}
    /**
     * Raised when the underlying operating system reports that a connection to
     * a resource is refused.
     *
     * @category Errors */
    export class ConnectionRefused extends Error {}
    /**
     * Raised when the underlying operating system reports that a connection has
     * been reset. With network servers, it can be a _normal_ occurrence where a
     * client will abort a connection instead of properly shutting it down.
     *
     * @category Errors */
    export class ConnectionReset extends Error {}
    /**
     * Raised when the underlying operating system reports an `ECONNABORTED`
     * error.
     *
     * @category Errors */
    export class ConnectionAborted extends Error {}
    /**
     * Raised when the underlying operating system reports an `ENOTCONN` error.
     *
     * @category Errors */
    export class NotConnected extends Error {}
    /**
     * Raised when attempting to open a server listener on an address and port
     * that already has a listener.
     *
     * @category Errors */
    export class AddrInUse extends Error {}
    /**
     * Raised when the underlying operating system reports an `EADDRNOTAVAIL`
     * error.
     *
     * @category Errors */
    export class AddrNotAvailable extends Error {}
    /**
     * Raised when trying to write to a resource and a broken pipe error occurs.
     * This can happen when trying to write directly to `stdout` or `stderr`
     * and the operating system is unable to pipe the output for a reason
     * external to the Deno runtime.
     *
     * @category Errors */
    export class BrokenPipe extends Error {}
    /**
     * Raised when trying to create a resource, like a file, that already
     * exits.
     *
     * @category Errors */
    export class AlreadyExists extends Error {}
    /**
     * Raised when an operation to returns data that is invalid for the
     * operation being performed.
     *
     * @category Errors */
    export class InvalidData extends Error {}
    /**
     * Raised when the underlying operating system reports that an I/O operation
     * has timed out (`ETIMEDOUT`).
     *
     * @category Errors */
    export class TimedOut extends Error {}
    /**
     * Raised when the underlying operating system reports an `EINTR` error. In
     * many cases, this underlying IO error will be handled internally within
     * Deno, or result in an @{link BadResource} error instead.
     *
     * @category Errors */
    export class Interrupted extends Error {}
    /**
     * Raised when expecting to write to a IO buffer resulted in zero bytes
     * being written.
     *
     * @category Errors */
    export class WriteZero extends Error {}
    /**
     * Raised when attempting to read bytes from a resource, but the EOF was
     * unexpectedly encountered.
     *
     * @category Errors */
    export class UnexpectedEof extends Error {}
    /**
     * The underlying IO resource is invalid or closed, and so the operation
     * could not be performed.
     *
     * @category Errors */
    export class BadResource extends Error {}
    /**
     * Raised in situations where when attempting to load a dynamic import,
     * too many redirects were encountered.
     *
     * @category Errors */
    export class Http extends Error {}
    /**
     * Raised when the underlying IO resource is not available because it is
     * being awaited on in another block of code.
     *
     * @category Errors */
    export class Busy extends Error {}
    /**
     * Raised when the underlying Deno API is asked to perform a function that
     * is not currently supported.
     *
     * @category Errors */
    export class NotSupported extends Error {}
  }

  /** The current process ID of this instance of the Deno CLI.
   *
   * ```ts
   * console.log(Deno.pid);
   * ```
   *
   * @category Runtime Environment
   */
  export const pid: number;

  /**
   * The process ID of parent process of this instance of the Deno CLI.
   *
   * ```ts
   * console.log(Deno.ppid);
   * ```
   *
   * @category Runtime Environment
   */
  export const ppid: number;

  /** @category Runtime Environment */
  export interface MemoryUsage {
    /** The number of bytes of the current Deno's process resident set size,
     * which is the amount of memory occupied in main memory (RAM). */
    rss: number;
    /** The total size of the heap for V8, in bytes. */
    heapTotal: number;
    /** The amount of the heap used for V8, in bytes. */
    heapUsed: number;
    /** Memory, in bytes, associated with JavaScript objects outside of the
     * JavaScript isolate. */
    external: number;
  }

  /**
   * Returns an object describing the memory usage of the Deno process and the
   * V8 subsystem measured in bytes.
   *
   * @category Runtime Environment
   */
  export function memoryUsage(): MemoryUsage;

  /**
   * Get the `hostname` of the machine the Deno process is running on.
   *
   * ```ts
   * console.log(Deno.hostname());
   * ```
   *
   * Requires `allow-sys` permission.
   *
   * @tags allow-sys
   * @category Runtime Environment
   */
  export function hostname(): string;

  /** Reflects the `NO_COLOR` environment variable at program start.
   *
   * When the value is `true`, the Deno CLI will attempt to not send color codes
   * to `stderr` or `stdout` and other command line programs should also attempt
   * to respect this value.
   *
   * See: https://no-color.org/
   *
   * @category Runtime Environment
   */
  export const noColor: boolean;

  /**
   * Options which define the permissions within a test or worker context.
   *
   * `"inherit"` ensures that all permissions of the parent process will be
   * applied to the test context. `"none"` ensures the test context has no
   * permissions. A `PermissionOptionsObject` provides a more specific
   * set of permissions to the test context.
   *
   * @category Permissions */
  export type PermissionOptions =
    | "inherit"
    | "none"
    | PermissionOptionsObject;

  /**
   * A set of options which can define the permissions within a test or worker
   * context at a highly specific level.
   *
   * @category Permissions */
  export interface PermissionOptionsObject {
    /** Specifies if the `env` permission should be requested or revoked.
     * If set to `"inherit"`, the current `env` permission will be inherited.
     * If set to `true`, the global `env` permission will be requested.
     * If set to `false`, the global `env` permission will be revoked.
     *
     * Defaults to `false`.
     */
    env?: "inherit" | boolean | string[];

    /** Specifies if the `sys` permission should be requested or revoked.
     * If set to `"inherit"`, the current `sys` permission will be inherited.
     * If set to `true`, the global `sys` permission will be requested.
     * If set to `false`, the global `sys` permission will be revoked.
     *
     * Defaults to `false`.
     */
    sys?: "inherit" | boolean | string[];

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

  /**
   * Context that is passed to a testing function, which can be used to either
   * gain information about the current test, or register additional test
   * steps within the current test.
   *
   * @category Testing */
  export interface TestContext {
    /** The current test name. */
    name: string;
    /** The string URL of the current test. */
    origin: string;
    /** If the current test is a step of another test, the parent test context
     * will be set here. */
    parent?: TestContext;

    /** Run a sub step of the parent test or step. Returns a promise
     * that resolves to a boolean signifying if the step completed successfully.
     *
     * The returned promise never rejects unless the arguments are invalid.
     *
     * If the test was ignored the promise returns `false`.
     *
     * ```ts
     * Deno.test({
     *   name: "a parent test",
     *   async fn(t) {
     *     console.log("before the step");
     *     await t.step({
     *       name: "step 1",
     *       fn(t) {
     *         console.log("current step:", t.name);
     *       }
     *     });
     *     console.log("after the step");
     *   }
     * });
     * ```
     */
    step(definition: TestStepDefinition): Promise<boolean>;

    /** Run a sub step of the parent test or step. Returns a promise
     * that resolves to a boolean signifying if the step completed successfully.
     *
     * The returned promise never rejects unless the arguments are invalid.
     *
     * If the test was ignored the promise returns `false`.
     *
     * ```ts
     * Deno.test(
     *   "a parent test",
     *   async (t) => {
     *     console.log("before the step");
     *     await t.step(
     *       "step 1",
     *       (t) => {
     *         console.log("current step:", t.name);
     *       }
     *     );
     *     console.log("after the step");
     *   }
     * );
     * ```
     */
    step(
      name: string,
      fn: (t: TestContext) => void | Promise<void>,
    ): Promise<boolean>;
  }

  /** @category Testing */
  export interface TestStepDefinition {
    /** The test function that will be tested when this step is executed. The
     * function can take an argument which will provide information about the
     * current step's context. */
    fn: (t: TestContext) => void | Promise<void>;
    /** The name of the step. */
    name: string;
    /** If truthy the current test step will be ignored.
     *
     * This is a quick way to skip over a step, but also can be used for
     * conditional logic, like determining if an environment feature is present.
     */
    ignore?: boolean;
    /** Check that the number of async completed operations after the test step
     * is the same as number of dispatched operations. This ensures that the
     * code tested does not start async operations which it then does
     * not await. This helps in preventing logic errors and memory leaks
     * in the application code.
     *
     * Defaults to the parent test or step's value. */
    sanitizeOps?: boolean;
    /** Ensure the test step does not "leak" resources - like open files or
     * network connections - by ensuring the open resources at the start of the
     * step match the open resources at the end of the step.
     *
     * Defaults to the parent test or step's value. */
    sanitizeResources?: boolean;
    /** Ensure the test step does not prematurely cause the process to exit,
     * for example via a call to {@linkcode Deno.exit}.
     *
     * Defaults to the parent test or step's value. */
    sanitizeExit?: boolean;
  }

  /** @category Testing */
  export interface TestDefinition {
    fn: (t: TestContext) => void | Promise<void>;
    /** The name of the test. */
    name: string;
    /** If truthy the current test step will be ignored.
     *
     * It is a quick way to skip over a step, but also can be used for
     * conditional logic, like determining if an environment feature is present.
     */
    ignore?: boolean;
    /** If at least one test has `only` set to `true`, only run tests that have
     * `only` set to `true` and fail the test suite. */
    only?: boolean;
    /** Check that the number of async completed operations after the test step
     * is the same as number of dispatched operations. This ensures that the
     * code tested does not start async operations which it then does
     * not await. This helps in preventing logic errors and memory leaks
     * in the application code.
     *
     * Defaults to `true`. */
    sanitizeOps?: boolean;
    /** Ensure the test step does not "leak" resources - like open files or
     * network connections - by ensuring the open resources at the start of the
     * test match the open resources at the end of the test.
     *
     * Defaults to `true`. */
    sanitizeResources?: boolean;
    /** Ensure the test case does not prematurely cause the process to exit,
     * for example via a call to {@linkcode Deno.exit}.
     *
     * Defaults to `true`. */
    sanitizeExit?: boolean;
    /** Specifies the permissions that should be used to run the test.
     *
     * Set this to "inherit" to keep the calling runtime permissions, set this
     * to "none" to revoke all permissions, or set a more specific set of
     * permissions using a {@linkcode PermissionOptionsObject}.
     *
     * Defaults to `"inherit"`. */
    permissions?: PermissionOptions;
  }

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module.
   *
   * `fn` can be async if required.
   *
   * ```ts
   * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test({
   *   name: "example test",
   *   fn() {
   *     assertEquals("world", "world");
   *   },
   * });
   *
   * Deno.test({
   *   name: "example ignored test",
   *   ignore: Deno.build.os === "windows",
   *   fn() {
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
   *
   * `fn` can be async if required.
   *
   * ```ts
   * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test("My test description", () => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test("My async test description", async () => {
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
   *
   * `fn` can be async if required. Declared function must have a name.
   *
   * ```ts
   * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test(function myTestName() {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.test(async function myOtherTestName() {
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
   *
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
   *
   * `fn` can be async if required.
   *
   * ```ts
   * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test(
   *   {
   *     name: "My test description",
   *     permissions: { read: true },
   *   },
   *   () => {
   *     assertEquals("hello", "hello");
   *   },
   * );
   *
   * Deno.test(
   *   {
   *     name: "My async test description",
   *     permissions: { read: false },
   *   },
   *   async () => {
   *     const decoder = new TextDecoder("utf-8");
   *     const data = await Deno.readFile("hello_world.txt");
   *     assertEquals(decoder.decode(data), "Hello world");
   *   },
   * );
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
   *
   * `fn` can be async if required. Declared function must have a name.
   *
   * ```ts
   * import { assertEquals } from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.test(
   *   { permissions: { read: true } },
   *   function myTestName() {
   *     assertEquals("hello", "hello");
   *   },
   * );
   *
   * Deno.test(
   *   { permissions: { read: false } },
   *   async function myOtherTestName() {
   *     const decoder = new TextDecoder("utf-8");
   *     const data = await Deno.readFile("hello_world.txt");
   *     assertEquals(decoder.decode(data), "Hello world");
   *   },
   * );
   * ```
   *
   * @category Testing
   */
  export function test(
    options: Omit<TestDefinition, "fn" | "name">,
    fn: (t: TestContext) => void | Promise<void>,
  ): void;

  /** Exit the Deno process with optional exit code.
   *
   * If no exit code is supplied then Deno will exit with return code of `0`.
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

  /** An interface containing methods to interact with the process environment
   * variables.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export interface Env {
    /** Retrieve the value of an environment variable.
     *
     * Returns `undefined` if the supplied environment variable is not defined.
     *
     * ```ts
     * console.log(Deno.env.get("HOME"));  // e.g. outputs "/home/alice"
     * console.log(Deno.env.get("MADE_UP_VAR"));  // outputs "undefined"
     * ```
     *
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

    /** Returns a snapshot of the environment variables at invocation as a
     * simple object of keys and values.
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
  }

  /** An interface containing methods to interact with the process environment
   * variables.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export const env: Env;

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
   * Throws {@linkcode Deno.errors.NotFound} if directory not found.
   *
   * Throws {@linkcode Deno.errors.PermissionDenied} if the user does not have
   * operating system file access rights.
   *
   * Requires `allow-read` permission.
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
   * Throws {@linkcode Deno.errors.NotFound} if directory not available.
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category Runtime Environment
   */
  export function cwd(): string;

  /**
   * Creates `newpath` as a hard link to `oldpath`.
   *
   * ```ts
   * await Deno.link("old/name", "new/name");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function link(oldpath: string, newpath: string): Promise<void>;

  /**
   * Synchronously creates `newpath` as a hard link to `oldpath`.
   *
   * ```ts
   * Deno.linkSync("old/name", "new/name");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function linkSync(oldpath: string, newpath: string): void;

  /**
   * A enum which defines the seek mode for IO related APIs that support
   * seeking.
   *
   * @category I/O */
  export enum SeekMode {
    /* Seek from the start of the file/resource. */
    Start = 0,
    /* Seek from the current position within the file/resource. */
    Current = 1,
    /* Seek from the end of the current file/resource. */
    End = 2,
  }

  /**
   * An abstract interface which when implemented provides an interface to read
   * bytes into an array buffer asynchronously.
   *
   * @category I/O */
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
     * Use
     * [`itereateReader`](https://deno.land/std/streams/conversion.ts?s=iterateReader)
     * from
     * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
     * to turn a `Reader` into an {@linkcode AsyncIterator}.
     */
    read(p: Uint8Array): Promise<number | null>;
  }

  /**
   * An abstract interface which when implemented provides an interface to read
   * bytes into an array buffer synchronously.
   *
   * @category I/O */
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
     * considering the EOF (`null`). Doing so correctly handles I/O errors that
     * happen after reading some bytes and also both of the allowed EOF
     * behaviors.
     *
     * Implementations should not retain a reference to `p`.
     *
     * Use
     * [`itereateReaderSync`](https://deno.land/std/streams/conversion.ts?s=iterateReaderSync)
     * from from
     * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
     * to turn a `ReaderSync` into an {@linkcode Iterator}.
     */
    readSync(p: Uint8Array): number | null;
  }

  /**
   * An abstract interface which when implemented provides an interface to write
   * bytes from an array buffer to a file/resource asynchronously.
   *
   * @category I/O */
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

  /**
   * An abstract interface which when implemented provides an interface to write
   * bytes from an array buffer to a file/resource synchronously.
   *
   * @category I/O */
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

  /**
   * An abstract interface which when implemented provides an interface to close
   * files/resources that were previously opened.
   *
   * @category I/O */
  export interface Closer {
    /** Closes the resource, "freeing" the backing file/resource. */
    close(): void;
  }

  /**
   * An abstract interface which when implemented provides an interface to seek
   * within an open file/resource asynchronously.
   *
   * @category I/O */
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
     *
     * It resolves with the updated offset.
     */
    seek(offset: number, whence: SeekMode): Promise<number>;
  }

  /**
   * An abstract interface which when implemented provides an interface to seek
   * within an open file/resource synchronously.
   *
   * @category I/O */
  export interface SeekerSync {
    /** Seek sets the offset for the next `readSync()` or `writeSync()` to
     * offset, interpreted according to `whence`: `Start` means relative
     * to the start of the file, `Current` means relative to the current
     * offset, and `End` means relative to the end.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O
     * operations on the underlying object is implementation-dependent.
     *
     * It returns the updated offset.
     */
    seekSync(offset: number, whence: SeekMode): number;
  }

  /**
   * Copies from `src` to `dst` until either EOF (`null`) is read from `src` or
   * an error occurs. It resolves to the number of bytes copied or rejects with
   * the first error encountered while copying.
   *
   * @deprecated Use
   * [`copy`](https://deno.land/std/streams/conversion.ts?s=copy) from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead. `Deno.copy` will be removed in the future.
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
    options?: { bufSize?: number },
  ): Promise<number>;

  /**
   * Turns a Reader, `r`, into an async iterator.
   *
   * @deprecated Use
   * [`iterateReader`](https://deno.land/std/streams/conversion.ts?s=iterateReader)
   * from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead. `Deno.iter` will be removed in the future.
   *
   * @category I/O
   */
  export function iter(
    r: Reader,
    options?: { bufSize?: number },
  ): AsyncIterableIterator<Uint8Array>;

  /**
   * Turns a ReaderSync, `r`, into an iterator.
   *
   * @deprecated Use
   * [`iterateReaderSync`](https://deno.land/std/streams/conversion.ts?s=iterateReaderSync)
   * from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead. `Deno.iterSync` will be removed in the future.
   *
   * @category I/O
   */
  export function iterSync(
    r: ReaderSync,
    options?: {
      bufSize?: number;
    },
  ): IterableIterator<Uint8Array>;

  /** Open a file and resolve to an instance of {@linkcode Deno.FsFile}. The
   * file does not need to previously exist if using the `create` or `createNew`
   * open options. It is the caller's responsibility to close the file when
   * finished with it.
   *
   * ```ts
   * const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
   * // Do work with file
   * Deno.close(file.rid);
   * ```
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on
   * options.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function open(
    path: string | URL,
    options?: OpenOptions,
  ): Promise<FsFile>;

  /** Synchronously open a file and return an instance of
   * {@linkcode Deno.FsFile}. The file does not need to previously exist if
   * using the `create` or `createNew` open options. It is the caller's
   * responsibility to close the file when finished with it.
   *
   * ```ts
   * const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
   * // Do work with file
   * Deno.close(file.rid);
   * ```
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on
   * options.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function openSync(path: string | URL, options?: OpenOptions): FsFile;

  /** Creates a file if none exists or truncates an existing file and resolves to
   *  an instance of {@linkcode Deno.FsFile}.
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

  /** Creates a file if none exists or truncates an existing file and returns
   *  an instance of {@linkcode Deno.FsFile}.
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
   * [`readAll()`](https://deno.land/std/streams/conversion.ts?s=readAll) from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead.
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

  /** Synchronously read from a resource ID (`rid`) into an array buffer
   * (`buffer`).
   *
   * Returns either the number of bytes read during the operation or EOF
   * (`null`) if there was nothing more to read.
   *
   * It is possible for a read to successfully return with `0` bytes. This does
   * not indicate EOF.
   *
   * This function is one of the lowest level APIs and most users should not
   * work with this directly, but rather use
   * [`readAllSync()`](https://deno.land/std/streams/conversion.ts?s=readAllSync)
   * from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead.
   *
   * **It is not guaranteed that the full buffer will be read in a single
   * call.**
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

  /** Write to the resource ID (`rid`) the contents of the array buffer (`data`).
   *
   * Resolves to the number of bytes written. This function is one of the lowest
   * level APIs and most users should not work with this directly, but rather use
   * [`writeAll()`](https://deno.land/std/streams/conversion.ts?s=writeAll) from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead.
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

  /** Synchronously write to the resource ID (`rid`) the contents of the array
   * buffer (`data`).
   *
   * Returns the number of bytes written. This function is one of the lowest
   * level APIs and most users should not work with this directly, but rather
   * use
   * [`writeAllSync()`](https://deno.land/std/streams/conversion.ts?s=writeAllSync)
   * from
   * [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   * instead.
   *
   * **It is not guaranteed that the full buffer will be written in a single
   * call.**
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world");
   * const file = Deno.openSync("/foo/bar.txt", { write: true });
   * const bytesWritten = Deno.writeSync(file.rid, data); // 11
   * Deno.close(file.rid);
   * ```
   *
   * @category I/O
   */
  export function writeSync(rid: number, data: Uint8Array): number;

  /** Seek a resource ID (`rid`) to the given `offset` under mode given by `whence`.
   * The call resolves to the new position within the resource (bytes from the start).
   *
   * ```ts
   * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   * const file = await Deno.open(
   *   "hello.txt",
   *   { read: true, write: true, truncate: true, create: true },
   * );
   * await Deno.write(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // advance cursor 6 bytes
   * const cursorPosition = await Deno.seek(file.rid, 6, Deno.SeekMode.Start);
   * console.log(cursorPosition);  // 6
   * const buf = new Uint8Array(100);
   * await file.read(buf);
   * console.log(new TextDecoder().decode(buf)); // "world"
   * file.close();
   * ```
   *
   * The seek modes work as follows:
   *
   * ```ts
   * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   * const file = await Deno.open(
   *   "hello.txt",
   *   { read: true, write: true, truncate: true, create: true },
   * );
   * await Deno.write(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // Seek 6 bytes from the start of the file
   * console.log(await Deno.seek(file.rid, 6, Deno.SeekMode.Start)); // "6"
   * // Seek 2 more bytes from the current position
   * console.log(await Deno.seek(file.rid, 2, Deno.SeekMode.Current)); // "8"
   * // Seek backwards 2 bytes from the end of the file
   * console.log(await Deno.seek(file.rid, -2, Deno.SeekMode.End)); // "9" (e.g. 11-2)
   * file.close();
   * ```
   *
   * @category I/O
   */
  export function seek(
    rid: number,
    offset: number,
    whence: SeekMode,
  ): Promise<number>;

  /** Synchronously seek a resource ID (`rid`) to the given `offset` under mode
   * given by `whence`. The new position within the resource (bytes from the
   * start) is returned.
   *
   * ```ts
   * const file = Deno.openSync(
   *   "hello.txt",
   *   { read: true, write: true, truncate: true, create: true },
   * );
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // advance cursor 6 bytes
   * const cursorPosition = Deno.seekSync(file.rid, 6, Deno.SeekMode.Start);
   * console.log(cursorPosition);  // 6
   * const buf = new Uint8Array(100);
   * file.readSync(buf);
   * console.log(new TextDecoder().decode(buf)); // "world"
   * file.close();
   * ```
   *
   * The seek modes work as follows:
   *
   * ```ts
   * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   * const file = Deno.openSync(
   *   "hello.txt",
   *   { read: true, write: true, truncate: true, create: true },
   * );
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello world"));
   *
   * // Seek 6 bytes from the start of the file
   * console.log(Deno.seekSync(file.rid, 6, Deno.SeekMode.Start)); // "6"
   * // Seek 2 more bytes from the current position
   * console.log(Deno.seekSync(file.rid, 2, Deno.SeekMode.Current)); // "8"
   * // Seek backwards 2 bytes from the end of the file
   * console.log(Deno.seekSync(file.rid, -2, Deno.SeekMode.End)); // "9" (e.g. 11-2)
   * file.close();
   * ```
   *
   * @category I/O
   */
  export function seekSync(
    rid: number,
    offset: number,
    whence: SeekMode,
  ): number;

  /**
   * Flushes any pending data and metadata operations of the given file stream
   * to disk.
   *
   * ```ts
   * const file = await Deno.open(
   *   "my_file.txt",
   *   { read: true, write: true, create: true },
   * );
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
   * Synchronously flushes any pending data and metadata operations of the given
   * file stream to disk.
   *
   * ```ts
   * const file = Deno.openSync(
   *   "my_file.txt",
   *   { read: true, write: true, create: true },
   * );
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
   * Flushes any pending data operations of the given file stream to disk.
   *  ```ts
   * const file = await Deno.open(
   *   "my_file.txt",
   *   { read: true, write: true, create: true },
   * );
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.fdatasync(file.rid);
   * console.log(new TextDecoder().decode(await Deno.readFile("my_file.txt"))); // Hello World
   * ```
   *
   * @category I/O
   */
  export function fdatasync(rid: number): Promise<void>;

  /**
   * Synchronously flushes any pending data operations of the given file stream
   * to disk.
   *
   *  ```ts
   * const file = Deno.openSync(
   *   "my_file.txt",
   *   { read: true, write: true, create: true },
   * );
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.fdatasyncSync(file.rid);
   * console.log(new TextDecoder().decode(Deno.readFileSync("my_file.txt"))); // Hello World
   * ```
   *
   * @category I/O
   */
  export function fdatasyncSync(rid: number): void;

  /** Close the given resource ID (`rid`) which has been previously opened, such
   * as via opening or creating a file. Closing a file when you are finished
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
   * This is the most straight forward way of handling files within Deno and is
   * recommended over using the discreet functions within the `Deno` namespace.
   *
   * ```ts
   * const file = await Deno.open("/foo/bar.txt", { read: true });
   * const fileInfo = await file.stat();
   * if (fileInfo.isFile) {
   *   const buf = new Uint8Array(100);
   *   const numberOfBytesRead = await file.read(buf); // 11 bytes
   *   const text = new TextDecoder().decode(buf);  // "hello world"
   * }
   * file.close();
   * ```
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
    /** The resource ID associated with the file instance. The resource ID
     * should be considered an opaque reference to resource. */
    readonly rid: number;
    /** A {@linkcode ReadableStream} instance representing to the byte contents
     * of the file. This makes it easy to interoperate with other web streams
     * based APIs.
     *
     * ```ts
     * const file = await Deno.open("my_file.txt", { read: true });
     * const decoder = new TextDecoder();
     * for await (const chunk of file.readable) {
     *   console.log(decoder.decode(chunk));
     * }
     * file.close();
     * ```
     */
    readonly readable: ReadableStream<Uint8Array>;
    /** A {@linkcode WritableStream} instance to write the contents of the
     * file. This makes it easy to interoperate with other web streams based
     * APIs.
     *
     * ```ts
     * const items = ["hello", "world"];
     * const file = await Deno.open("my_file.txt", { write: true });
     * const encoder = new TextEncoder();
     * const writer = file.writable.getWriter();
     * for (const item of items) {
     *   await writer.write(encoder.encode(item));
     * }
     * file.close();
     * ```
     */
    readonly writable: WritableStream<Uint8Array>;
    /** The constructor which takes a resource ID. Generally `FsFile` should
     * not be constructed directly. Instead use {@linkcode Deno.open} or
     * {@linkcode Deno.openSync} to create a new instance of `FsFile`. */
    constructor(rid: number);
    /** Write the contents of the array buffer (`p`) to the file.
     *
     * Resolves to the number of bytes written.
     *
     * **It is not guaranteed that the full buffer will be written in a single
     * call.**
     *
     * ```ts
     * const encoder = new TextEncoder();
     * const data = encoder.encode("Hello world");
     * const file = await Deno.open("/foo/bar.txt", { write: true });
     * const bytesWritten = await file.write(data); // 11
     * file.close();
     * ```
     *
     * @category I/O
     */
    write(p: Uint8Array): Promise<number>;
    /** Synchronously write the contents of the array buffer (`p`) to the file.
     *
     * Returns the number of bytes written.
     *
     * **It is not guaranteed that the full buffer will be written in a single
     * call.**
     *
     * ```ts
     * const encoder = new TextEncoder();
     * const data = encoder.encode("Hello world");
     * const file = Deno.openSync("/foo/bar.txt", { write: true });
     * const bytesWritten = file.writeSync(data); // 11
     * file.close();
     * ```
     */
    writeSync(p: Uint8Array): number;
    /** Truncates (or extends) the file to reach the specified `len`. If `len`
     * is not specified, then the entire file contents are truncated.
     *
     * ### Truncate the entire file
     *
     * ```ts
     * const file = await Deno.open("my_file.txt", { write: true });
     * await file.truncate();
     * file.close();
     * ```
     *
     * ### Truncate part of the file
     *
     * ```ts
     * // if "my_file.txt" contains the text "hello world":
     * const file = await Deno.open("my_file.txt", { write: true });
     * await file.truncate(7);
     * const buf = new Uint8Array(100);
     * await file.read(buf);
     * const text = new TextDecoder().decode(buf); // "hello w"
     * file.close();
     * ```
     */
    truncate(len?: number): Promise<void>;
    /** Synchronously truncates (or extends) the file to reach the specified
     * `len`. If `len` is not specified, then the entire file contents are
     * truncated.
     *
     * ### Truncate the entire file
     *
     * ```ts
     * const file = Deno.openSync("my_file.txt", { write: true });
     * file.truncateSync();
     * file.close();
     * ```
     *
     * ### Truncate part of the file
     *
     * ```ts
     * // if "my_file.txt" contains the text "hello world":
     * const file = Deno.openSync("my_file.txt", { write: true });
     * file.truncateSync(7);
     * const buf = new Uint8Array(100);
     * file.readSync(buf);
     * const text = new TextDecoder().decode(buf); // "hello w"
     * file.close();
     * ```
     */
    truncateSync(len?: number): void;
    /** Read the file into an array buffer (`p`).
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
     * // if "/foo/bar.txt" contains the text "hello world":
     * const file = await Deno.open("/foo/bar.txt");
     * const buf = new Uint8Array(100);
     * const numberOfBytesRead = await file.read(buf); // 11 bytes
     * const text = new TextDecoder().decode(buf);  // "hello world"
     * file.close();
     * ```
     */
    read(p: Uint8Array): Promise<number | null>;
    /** Synchronously read from the file into an array buffer (`p`).
     *
     * Returns either the number of bytes read during the operation or EOF
     * (`null`) if there was nothing more to read.
     *
     * It is possible for a read to successfully return with `0` bytes. This
     * does not indicate EOF.
     *
     * **It is not guaranteed that the full buffer will be read in a single
     * call.**
     *
     * ```ts
     * // if "/foo/bar.txt" contains the text "hello world":
     * const file = Deno.openSync("/foo/bar.txt");
     * const buf = new Uint8Array(100);
     * const numberOfBytesRead = file.readSync(buf); // 11 bytes
     * const text = new TextDecoder().decode(buf);  // "hello world"
     * file.close();
     * ```
     */
    readSync(p: Uint8Array): number | null;
    /** Seek to the given `offset` under mode given by `whence`. The call
     * resolves to the new position within the resource (bytes from the start).
     *
     * ```ts
     * // Given file pointing to file with "Hello world", which is 11 bytes long:
     * const file = await Deno.open(
     *   "hello.txt",
     *   { read: true, write: true, truncate: true, create: true },
     * );
     * await file.write(new TextEncoder().encode("Hello world"));
     *
     * // advance cursor 6 bytes
     * const cursorPosition = await file.seek(6, Deno.SeekMode.Start);
     * console.log(cursorPosition);  // 6
     * const buf = new Uint8Array(100);
     * await file.read(buf);
     * console.log(new TextDecoder().decode(buf)); // "world"
     * file.close();
     * ```
     *
     * The seek modes work as follows:
     *
     * ```ts
     * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
     * const file = await Deno.open(
     *   "hello.txt",
     *   { read: true, write: true, truncate: true, create: true },
     * );
     * await file.write(new TextEncoder().encode("Hello world"));
     *
     * // Seek 6 bytes from the start of the file
     * console.log(await file.seek(6, Deno.SeekMode.Start)); // "6"
     * // Seek 2 more bytes from the current position
     * console.log(await file.seek(2, Deno.SeekMode.Current)); // "8"
     * // Seek backwards 2 bytes from the end of the file
     * console.log(await file.seek(-2, Deno.SeekMode.End)); // "9" (e.g. 11-2)
     * ```
     */
    seek(offset: number, whence: SeekMode): Promise<number>;
    /** Synchronously seek to the given `offset` under mode given by `whence`.
     * The new position within the resource (bytes from the start) is returned.
     *
     * ```ts
     * const file = Deno.openSync(
     *   "hello.txt",
     *   { read: true, write: true, truncate: true, create: true },
     * );
     * file.writeSync(new TextEncoder().encode("Hello world"));
     *
     * // advance cursor 6 bytes
     * const cursorPosition = file.seekSync(6, Deno.SeekMode.Start);
     * console.log(cursorPosition);  // 6
     * const buf = new Uint8Array(100);
     * file.readSync(buf);
     * console.log(new TextDecoder().decode(buf)); // "world"
     * file.close();
     * ```
     *
     * The seek modes work as follows:
     *
     * ```ts
     * // Given file.rid pointing to file with "Hello world", which is 11 bytes long:
     * const file = Deno.openSync(
     *   "hello.txt",
     *   { read: true, write: true, truncate: true, create: true },
     * );
     * file.writeSync(new TextEncoder().encode("Hello world"));
     *
     * // Seek 6 bytes from the start of the file
     * console.log(file.seekSync(6, Deno.SeekMode.Start)); // "6"
     * // Seek 2 more bytes from the current position
     * console.log(file.seekSync(2, Deno.SeekMode.Current)); // "8"
     * // Seek backwards 2 bytes from the end of the file
     * console.log(file.seekSync(-2, Deno.SeekMode.End)); // "9" (e.g. 11-2)
     * file.close();
     * ```
     */
    seekSync(offset: number, whence: SeekMode): number;
    /** Resolves to a {@linkcode Deno.FileInfo} for the file.
     *
     * ```ts
     * import { assert } from "https://deno.land/std/testing/asserts.ts";
     *
     * const file = await Deno.open("hello.txt");
     * const fileInfo = await file.stat();
     * assert(fileInfo.isFile);
     * file.close();
     * ```
     */
    stat(): Promise<FileInfo>;
    /** Synchronously returns a {@linkcode Deno.FileInfo} for the file.
     *
     * ```ts
     * import { assert } from "https://deno.land/std/testing/asserts.ts";
     *
     * const file = Deno.openSync("hello.txt")
     * const fileInfo = file.statSync();
     * assert(fileInfo.isFile);
     * file.close();
     * ```
     */
    statSync(): FileInfo;
    /** Close the file. Closing a file when you are finished with it is
     * important to avoid leaking resources.
     *
     * ```ts
     * const file = await Deno.open("my_file.txt");
     * // do work with "file" object
     * file.close();
     * ```
     */
    close(): void;
  }

  /**
   * The Deno abstraction for reading and writing files.
   *
   * @deprecated Use {@linkcode Deno.FsFile} instead. `Deno.File` will be
   *   removed in the future.
   * @category File System
   */
  export const File: typeof FsFile;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   *  @category I/O */
  export interface SetRawOptions {
    cbreak: boolean;
  }

  /** A reference to `stdin` which can be used to read directly from `stdin`.
   * It implements the Deno specific {@linkcode Reader}, {@linkcode ReaderSync},
   * and {@linkcode Closer} interfaces as well as provides a
   * {@linkcode ReadableStream} interface.
   *
   * ### Reading chunks from the readable stream
   *
   * ```ts
   * const decoder = new TextDecoder();
   * for await (const chunk of Deno.stdin.readable) {
   *   const text = decoder.decode(chunk);
   *   // do something with the text
   * }
   * ```
   *
   * @category I/O
   */
  export const stdin: Reader & ReaderSync & Closer & {
    /** The resource ID assigned to `stdin`. This can be used with the discreet
     * I/O functions in the `Deno` namespace. */
    readonly rid: number;
    /** A readable stream interface to `stdin`. */
    readonly readable: ReadableStream<Uint8Array>;
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Set TTY to be under raw mode or not. In raw mode, characters are read and
     * returned as is, without being processed. All special processing of
     * characters by the terminal is disabled, including echoing input
     * characters. Reading from a TTY device in raw mode is faster than reading
     * from a TTY device in canonical mode.
     *
     * The `cbreak` option can be used to indicate that characters that
     * correspond to a signal should still be generated. When disabling raw
     * mode, this option is ignored. This functionality currently only works on
     * Linux and Mac OS.
     *
     * ```ts
     * Deno.stdin.setRaw(true, { cbreak: true });
     * ```
     *
     * @category I/O
     */
    setRaw(mode: boolean, options?: SetRawOptions): void;
  };
  /** A reference to `stdout` which can be used to write directly to `stdout`.
   * It implements the Deno specific {@linkcode Writer}, {@linkcode WriterSync},
   * and {@linkcode Closer} interfaces as well as provides a
   * {@linkcode WritableStream} interface.
   *
   * These are low level constructs, and the {@linkcode console} interface is a
   * more straight forward way to interact with `stdout` and `stderr`.
   *
   * @category I/O
   */
  export const stdout: Writer & WriterSync & Closer & {
    /** The resource ID assigned to `stdout`. This can be used with the discreet
     * I/O functions in the `Deno` namespace. */
    readonly rid: number;
    /** A writable stream interface to `stdout`. */
    readonly writable: WritableStream<Uint8Array>;
  };
  /** A reference to `stderr` which can be used to write directly to `stderr`.
   * It implements the Deno specific {@linkcode Writer}, {@linkcode WriterSync},
   * and {@linkcode Closer} interfaces as well as provides a
   * {@linkcode WritableStream} interface.
   *
   * These are low level constructs, and the {@linkcode console} interface is a
   * more straight forward way to interact with `stdout` and `stderr`.
   *
   * @category I/O
   */
  export const stderr: Writer & WriterSync & Closer & {
    /** The resource ID assigned to `stderr`. This can be used with the discreet
     * I/O functions in the `Deno` namespace. */
    readonly rid: number;
    /** A writable stream interface to `stderr`. */
    readonly writable: WritableStream<Uint8Array>;
  };

  /**
   * Options which can be set when doing {@linkcode Deno.open} and
   * {@linkcode Deno.openSync}.
   *
   * @category File System */
  export interface OpenOptions {
    /** Defaults to `true`. Sets the option for read access. This option, when `true`, means that
     * the file should be read-able if opened. */
    read?: boolean;
    /** Defaults to `false`. Sets the option for write access. This option, when `true`, means that
     * the file should be write-able if opened. If the file already exists,
     * any write calls on it will overwrite its contents, by default without
     * truncating it. */
    write?: boolean;
    /** Defaults to `false`. Sets the option for the append mode. This option, when `true`, means
     * that writes will append to a file instead of overwriting previous
     * contents.
     *
     * Note that setting `{ write: true, append: true }` has the same effect as
     * setting only `{ append: true }`. */
    append?: boolean;
    /** Defaults to `false`. Sets the option for truncating a previous file. If a file is
     * successfully opened with this option set it will truncate the file to `0`
     * size if it already exists. The file must be opened with write access
     * for truncate to work. */
    truncate?: boolean;
    /** Defaults to `false`. Sets the option to allow creating a new file, if one doesn't already
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
     *
     * Ignored on Windows. */
    mode?: number;
  }

  /**
   * Options which can be set when using {@linkcode Deno.readFile} or
   * {@linkcode Deno.readFileSync}.
   *
   * @category File System */
  export interface ReadFileOptions {
    /**
     * An abort signal to allow cancellation of the file read operation.
     * If the signal becomes aborted the readFile operation will be stopped
     * and the promise returned will be rejected with an AbortError.
     */
    signal?: AbortSignal;
  }

  /**
   *  Check if a given resource id (`rid`) is a TTY (a terminal).
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
   * @deprecated Use [`Buffer`](https://deno.land/std/io/buffer.ts?s=Buffer)
   *   from [`std/io/buffer.ts`](https://deno.land/std/io/buffer.ts) instead.
   *   `Deno.Buffer` will be removed in the future.
   *
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
   * @deprecated Use
   *   [`readAll`](https://deno.land/std/streams/conversion.ts?s=readAll) from
   *   [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   *   instead. `Deno.readAll` will be removed in the future.
   *
   * @category I/O
   */
  export function readAll(r: Reader): Promise<Uint8Array>;

  /**
   * Synchronously reads Reader `r` until EOF (`null`) and returns the content
   * as `Uint8Array`.
   *
   * @deprecated Use
   *   [`readAllSync`](https://deno.land/std/streams/conversion.ts?s=readAllSync)
   *   from
   *   [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   *   instead. `Deno.readAllSync` will be removed in the future.
   *
   * @category I/O
   */
  export function readAllSync(r: ReaderSync): Uint8Array;

  /**
   * Write all the content of the array buffer (`arr`) to the writer (`w`).
   *
   * @deprecated Use
   *   [`writeAll`](https://deno.land/std/streams/conversion.ts?s=writeAll) from
   *   [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   *   instead. `Deno.writeAll` will be removed in the future.
   *
   * @category I/O
   */
  export function writeAll(w: Writer, arr: Uint8Array): Promise<void>;

  /**
   * Synchronously write all the content of the array buffer (`arr`) to the
   * writer (`w`).
   *
   * @deprecated Use
   *   [`writeAllSync`](https://deno.land/std/streams/conversion.ts?s=writeAllSync)
   *   from
   *   [`std/streams/conversion.ts`](https://deno.land/std/streams/conversion.ts)
   *   instead. `Deno.writeAllSync` will be removed in the future.
   *
   * @category I/O
   */
  export function writeAllSync(w: WriterSync, arr: Uint8Array): void;

  /**
   * Options which can be set when using {@linkcode Deno.mkdir} and
   * {@linkcode Deno.mkdirSync}.
   *
   * @category File System */
  export interface MkdirOptions {
    /** Defaults to `false`. If set to `true`, means that any intermediate
     * directories will also be created (as with the shell command `mkdir -p`).
     *
     * Intermediate directories are created with the same permissions.
     *
     * When recursive is set to `true`, succeeds silently (without changing any
     * permissions) if a directory already exists at the path, or if the path
     * is a symlink to an existing directory. */
    recursive?: boolean;
    /** Permissions to use when creating the directory (defaults to `0o777`,
     * before the process's umask).
     *
     * Ignored on Windows. */
    mode?: number;
  }

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

  /**
   * Options which can be set when using {@linkcode Deno.makeTempDir},
   * {@linkcode Deno.makeTempDirSync}, {@linkcode Deno.makeTempFile}, and
   * {@linkcode Deno.makeTempFileSync}.
   *
   * @category File System */
  export interface MakeTempOptions {
    /** Directory where the temporary directory should be created (defaults to
     * the env variable `TMPDIR`, or the system's default, usually `/tmp`).
     *
     * Note that if the passed `dir` is relative, the path returned by
     * `makeTempFile()` and `makeTempDir()` will also be relative. Be mindful of
     * this when changing working directory. */
    dir?: string;
    /** String that should precede the random portion of the temporary
     * directory's name. */
    prefix?: string;
    /** String that should follow the random portion of the temporary
     * directory's name. */
    suffix?: string;
  }

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

  /** Creates a new temporary file in the default directory for temporary
   * files, unless `dir` is specified.
   *
   * Other options include prefixing and suffixing the directory name with
   * `prefix` and `suffix` respectively.
   *
   * This call resolves to the full path to the newly created file.
   *
   * Multiple programs calling this function simultaneously will create
   * different files. It is the caller's responsibility to remove the file when
   * no longer needed.
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

  /** Synchronously creates a new temporary file in the default directory for
   * temporary files, unless `dir` is specified.
   *
   * Other options include prefixing and suffixing the directory name with
   * `prefix` and `suffix` respectively.
   *
   * The full path to the newly created file is returned.
   *
   * Multiple programs calling this function simultaneously will create
   * different files. It is the caller's responsibility to remove the file when
   * no longer needed.
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

  /** Changes the permission of a specific file/directory of specified path.
   * Ignores the process's umask.
   *
   * ```ts
   * await Deno.chmod("/path/to/file", 0o666);
   * ```
   *
   * The mode is a sequence of 3 octal numbers. The first/left-most number
   * specifies the permissions for the owner. The second number specifies the
   * permissions for the group. The last/right-most number specifies the
   * permissions for others. For example, with a mode of 0o764, the owner (7)
   * can read/write/execute, the group (6) can read/write and everyone else (4)
   * can read only.
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

  /** Synchronously changes the permission of a specific file/directory of
   * specified path. Ignores the process's umask.
   *
   * ```ts
   * Deno.chmodSync("/path/to/file", 0o666);
   * ```
   *
   * For a full description, see {@linkcode Deno.chmod}.
   *
   * NOTE: This API currently throws on Windows
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function chmodSync(path: string | URL, mode: number): void;

  /** Change owner of a regular file or directory.
   *
   * This functionality is not available on Windows.
   *
   * ```ts
   * await Deno.chown("myFile.txt", 1000, 1002);
   * ```
   *
   * Requires `allow-write` permission.
   *
   * Throws Error (not implemented) if executed on Windows.
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

  /** Synchronously change owner of a regular file or directory.
   *
   * This functionality is not available on Windows.
   *
   * ```ts
   * Deno.chownSync("myFile.txt", 1000, 1002);
   * ```
   *
   * Requires `allow-write` permission.
   *
   * Throws Error (not implemented) if executed on Windows.
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

  /**
   * Options which can be set when using {@linkcode Deno.remove} and
   * {@linkcode Deno.removeSync}.
   *
   * @category File System */
  export interface RemoveOptions {
    /** Defaults to `false`. If set to `true`, path will be removed even if
     * it's a non-empty directory. */
    recursive?: boolean;
  }

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

  /** Synchronously renames (moves) `oldpath` to `newpath`. Paths may be files or
   * directories. If `newpath` already exists and is not a directory,
   * `renameSync()` replaces it. OS-specific restrictions may apply when
   * `oldpath` and `newpath` are in different directories.
   *
   * ```ts
   * Deno.renameSync("old/path", "new/path");
   * ```
   *
   * On Unix-like OSes, this operation does not follow symlinks at either path.
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

  /** Renames (moves) `oldpath` to `newpath`. Paths may be files or directories.
   * If `newpath` already exists and is not a directory, `rename()` replaces it.
   * OS-specific restrictions may apply when `oldpath` and `newpath` are in
   * different directories.
   *
   * ```ts
   * await Deno.rename("old/path", "new/path");
   * ```
   *
   * On Unix-like OSes, this operation does not follow symlinks at either path.
   *
   * It varies between platforms when the operation throws errors, and if so
   * what they are. It's always an error to rename anything to a non-empty
   * directory.
   *
   * Requires `allow-read` and `allow-write` permissions.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function rename(
    oldpath: string | URL,
    newpath: string | URL,
  ): Promise<void>;

  /** Asynchronously reads and returns the entire contents of a file as an UTF-8
   *  decoded string. Reading a directory throws an error.
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

  /** Synchronously reads and returns the entire contents of a file as an UTF-8
   *  decoded string. Reading a directory throws an error.
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

  /** Synchronously reads and returns the entire contents of a file as an array
   * of bytes. `TextDecoder` can be used to transform the bytes to string if
   * required. Reading a directory returns an empty data array.
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

  /** Provides information about a file and is returned by
   * {@linkcode Deno.stat}, {@linkcode Deno.lstat}, {@linkcode Deno.statSync},
   * and {@linkcode Deno.lstatSync} or from calling `stat()` and `statSync()`
   * on an {@linkcode Deno.FsFile} instance.
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
   *
   * Also requires `allow-read` permission for the `CWD` if the target path is
   * relative.
   *
   * @tags allow-read
   * @category File System
   */
  export function realPath(path: string | URL): Promise<string>;

  /** Synchronously returns absolute normalized path, with symbolic links
   * resolved.
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
   *
   * Also requires `allow-read` permission for the `CWD` if the target path is
   * relative.
   *
   * @tags allow-read
   * @category File System
   */
  export function realPathSync(path: string | URL): string;

  /**
   * Information about a directory entry returned from {@linkcode Deno.readDir}
   * and {@linkcode Deno.readDirSync}.
   *
   * @category File System */
  export interface DirEntry {
    /** The file name of the entry. It is just the entity name and does not
     * include the full path. */
    name: string;
    /** True if this is info for a regular file. Mutually exclusive to
     * `DirEntry.isDirectory` and `DirEntry.isSymlink`. */
    isFile: boolean;
    /** True if this is info for a regular directory. Mutually exclusive to
     * `DirEntry.isFile` and `DirEntry.isSymlink`. */
    isDirectory: boolean;
    /** True if this is info for a symlink. Mutually exclusive to
     * `DirEntry.isFile` and `DirEntry.isDirectory`. */
    isSymlink: boolean;
  }

  /** Reads the directory given by `path` and returns an async iterable of
   * {@linkcode Deno.DirEntry}.
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

  /** Copies the contents and permissions of one file to another specified path,
   * by default creating a new file if needed, else overwriting. Fails if target
   * path is a directory or is unwritable.
   *
   * ```ts
   * await Deno.copyFile("from.txt", "to.txt");
   * ```
   *
   * Requires `allow-read` permission on `fromPath`.
   *
   * Requires `allow-write` permission on `toPath`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function copyFile(
    fromPath: string | URL,
    toPath: string | URL,
  ): Promise<void>;

  /** Synchronously copies the contents and permissions of one file to another
   * specified path, by default creating a new file if needed, else overwriting.
   * Fails if target path is a directory or is unwritable.
   *
   * ```ts
   * Deno.copyFileSync("from.txt", "to.txt");
   * ```
   *
   * Requires `allow-read` permission on `fromPath`.
   *
   * Requires `allow-write` permission on `toPath`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function copyFileSync(
    fromPath: string | URL,
    toPath: string | URL,
  ): void;

  /** Resolves to the full path destination of the named symbolic link.
   *
   * ```ts
   * await Deno.symlink("./test.txt", "./test_link.txt");
   * const target = await Deno.readLink("./test_link.txt"); // full path of ./test.txt
   * ```
   *
   * Throws TypeError if called with a hard link.
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readLink(path: string | URL): Promise<string>;

  /** Synchronously returns the full path destination of the named symbolic
   * link.
   *
   * ```ts
   * Deno.symlinkSync("./test.txt", "./test_link.txt");
   * const target = Deno.readLinkSync("./test_link.txt"); // full path of ./test.txt
   * ```
   *
   * Throws TypeError if called with a hard link.
   *
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function readLinkSync(path: string | URL): string;

  /** Resolves to a {@linkcode Deno.FileInfo} for the specified `path`. If
   * `path` is a symlink, information for the symlink will be returned instead
   * of what it points to.
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

  /** Synchronously returns a {@linkcode Deno.FileInfo} for the specified
   * `path`. If `path` is a symlink, information for the symlink will be
   * returned instead of what it points to.
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

  /** Resolves to a {@linkcode Deno.FileInfo} for the specified `path`. Will
   * always follow symlinks.
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

  /** Synchronously returns a {@linkcode Deno.FileInfo} for the specified
   * `path`. Will always follow symlinks.
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
    /** An abort signal to allow cancellation of the file write operation.
     *
     * If the signal becomes aborted the write file operation will be stopped
     * and the promise returned will be rejected with an {@linkcode AbortError}.
     */
    signal?: AbortSignal;
  }

  /** Write `data` to the given `path`, by default creating a new file if
   * needed, else overwriting.
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world\n");
   * await Deno.writeFile("hello1.txt", data);  // overwrite "hello1.txt" or create it
   * await Deno.writeFile("hello2.txt", data, { create: false });  // only works if "hello2.txt" exists
   * await Deno.writeFile("hello3.txt", data, { mode: 0o777 });  // set permissions on new file
   * await Deno.writeFile("hello4.txt", data, { append: true });  // add data to the end of the file
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is
   * `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeFile(
    path: string | URL,
    data: Uint8Array,
    options?: WriteFileOptions,
  ): Promise<void>;

  /** Synchronously write `data` to the given `path`, by default creating a new
   * file if needed, else overwriting.
   *
   * ```ts
   * const encoder = new TextEncoder();
   * const data = encoder.encode("Hello world\n");
   * Deno.writeFileSync("hello1.txt", data);  // overwrite "hello1.txt" or create it
   * Deno.writeFileSync("hello2.txt", data, { create: false });  // only works if "hello2.txt" exists
   * Deno.writeFileSync("hello3.txt", data, { mode: 0o777 });  // set permissions on new file
   * Deno.writeFileSync("hello4.txt", data, { append: true });  // add data to the end of the file
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

  /** Write string `data` to the given `path`, by default creating a new file if
   * needed, else overwriting.
   *
   * ```ts
   * await Deno.writeTextFile("hello1.txt", "Hello world\n");  // overwrite "hello1.txt" or create it
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is
   * `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeTextFile(
    path: string | URL,
    data: string,
    options?: WriteFileOptions,
  ): Promise<void>;

  /** Synchronously write string `data` to the given `path`, by default creating
   * a new file if needed, else overwriting.
   *
   * ```ts
   * Deno.writeTextFileSync("hello1.txt", "Hello world\n");  // overwrite "hello1.txt" or create it
   * ```
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is
   * `false`.
   *
   * @tags allow-read, allow-write
   * @category File System
   */
  export function writeTextFileSync(
    path: string | URL,
    data: string,
    options?: WriteFileOptions,
  ): void;

  /** Truncates (or extends) the specified file, to reach the specified `len`.
   * If `len` is not specified then the entire file contents are truncated.
   *
   * ### Truncate the entire file
   * ```ts
   * await Deno.truncate("my_file.txt");
   * ```
   *
   * ### Truncate part of the file
   *
   * ```
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

  /** Synchronously truncates (or extends) the specified file, to reach the
   * specified `len`. If `len` is not specified then the entire file contents
   * are truncated.
   *
   * ### Truncate the entire file
   *
   * ```ts
   * Deno.truncateSync("my_file.txt");
   * ```
   *
   * ### Truncate part of the file
   *
   * ```ts
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
   * in the development of Deno. _Ops_, also called _bindings_, are the
   * go-between between Deno JavaScript sandbox and the rest of Deno.
   *
   * ```shell
   * > console.table(Deno.metrics())
   * ┌─────────────────────────┬────────┐
   * │         (index)         │ Values │
   * ├─────────────────────────┼────────┤
   * │      opsDispatched      │   3    │
   * │    opsDispatchedSync    │   2    │
   * │   opsDispatchedAsync    │   1    │
   * │ opsDispatchedAsyncUnref │   0    │
   * │      opsCompleted       │   3    │
   * │    opsCompletedSync     │   2    │
   * │    opsCompletedAsync    │   1    │
   * │ opsCompletedAsyncUnref  │   0    │
   * │    bytesSentControl     │   73   │
   * │      bytesSentData      │   0    │
   * │      bytesReceived      │  375   │
   * └─────────────────────────┴────────┘
   * ```
   *
   * @category Observability
   */
  export function metrics(): Metrics;

  /**
   * A map of open resources that Deno is tracking. The key is the resource ID
   * (_rid_) and the value is its representation.
   *
   * @category Observability */
  interface ResourceMap {
    [rid: number]: unknown;
  }

  /** Returns a map of open resource IDs (_rid_) along with their string
   * representations. This is an internal API and as such resource
   * representation has `unknown` type; that means it can change any time and
   * should not be depended upon.
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
   * - `"rescan"`: rescan notices indicate either a lapse in the events or a
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

  /**
   * Represents a unique file system event yielded by a
   * {@linkcode Deno.FsWatcher}.
   *
   * @category File System */
  export interface FsEvent {
    /** The kind/type of the file system event. */
    kind: "any" | "access" | "create" | "modify" | "remove" | "other";
    /** An array of paths that are associated with the file system event. */
    paths: string[];
    /** Any additional flags associated with the event. */
    flag?: FsEventFlag;
  }

  /**
   * Returned by {@linkcode Deno.watchFs}. It is an async iterator yielding up
   * system events. To stop watching the file system by calling `.close()`
   * method.
   *
   * @category File System
   */
  export interface FsWatcher extends AsyncIterable<FsEvent> {
    /** The resource id. */
    readonly rid: number;
    /** Stops watching the file system and closes the watcher resource. */
    close(): void;
    /**
     * Stops watching the file system and closes the watcher resource.
     *
     * @deprecated Will be removed in the future.
     */
    return?(value?: any): Promise<IteratorResult<FsEvent>>;
    [Symbol.asyncIterator](): AsyncIterableIterator<FsEvent>;
  }

  /** Watch for file system events against one or more `paths`, which can be
   * files or directories. These paths must exist already. One user action (e.g.
   * `touch test.file`) can generate multiple file system events. Likewise,
   * one user action can result in multiple file paths in one event (e.g. `mv
   * old_name.txt new_name.txt`).
   *
   * The recursive option is `true` by default and, for directories, will watch
   * the specified directory and all sub directories.
   *
   * Note that the exact ordering of the events can vary between operating
   * systems.
   *
   * ```ts
   * const watcher = Deno.watchFs("/");
   * for await (const event of watcher) {
   *    console.log(">>>> event", event);
   *    // { kind: "create", paths: [ "/foo.txt" ] }
   * }
   * ```
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
   * Requires `allow-read` permission.
   *
   * @tags allow-read
   * @category File System
   */
  export function watchFs(
    paths: string | string[],
    options?: { recursive: boolean },
  ): FsWatcher;

  /** Options which can be used with {@linkcode Deno.run}.
   *
   * @category Sub Process */
  export interface RunOptions {
    /** Arguments to pass.
     *
     * _Note_: the first element needs to be a path to the executable that is
     * being run. */
    cmd: readonly string[] | [string | URL, ...string[]];
    /** The current working directory that should be used when running the
     * sub-process. */
    cwd?: string;
    /** Any environment variables to be set when running the sub-process. */
    env?: Record<string, string>;
    /** By default subprocess inherits `stdout` of parent process. To change
     * this this option can be set to a resource ID (_rid_) of an open file,
     * `"inherit"`, `"piped"`, or `"null"`:
     *
     * - _number_: the resource ID of an open file/resource. This allows you to
     *   write to a file.
     * - `"inherit"`: The default if unspecified. The subprocess inherits from the
     *   parent.
     * - `"piped"`: A new pipe should be arranged to connect the parent and child
     *   sub-process.
     * - `"null"`: This stream will be ignored. This is the equivalent of attaching
     *   the stream to `/dev/null`.
     */
    stdout?: "inherit" | "piped" | "null" | number;
    /** By default subprocess inherits `stderr` of parent process. To change
     * this this option can be set to a resource ID (_rid_) of an open file,
     * `"inherit"`, `"piped"`, or `"null"`:
     *
     * - _number_: the resource ID of an open file/resource. This allows you to
     *   write to a file.
     * - `"inherit"`: The default if unspecified. The subprocess inherits from the
     *   parent.
     * - `"piped"`: A new pipe should be arranged to connect the parent and child
     *   sub-process.
     * - `"null"`: This stream will be ignored. This is the equivalent of attaching
     *   the stream to `/dev/null`.
     */
    stderr?: "inherit" | "piped" | "null" | number;
    /** By default subprocess inherits `stdin` of parent process. To change
     * this this option can be set to a resource ID (_rid_) of an open file,
     * `"inherit"`, `"piped"`, or `"null"`:
     *
     * - _number_: the resource ID of an open file/resource. This allows you to
     *   read from a file.
     * - `"inherit"`: The default if unspecified. The subprocess inherits from the
     *   parent.
     * - `"piped"`: A new pipe should be arranged to connect the parent and child
     *   sub-process.
     * - `"null"`: This stream will be ignored. This is the equivalent of attaching
     *   the stream to `/dev/null`.
     */
    stdin?: "inherit" | "piped" | "null" | number;
  }

  /** The status resolved from the `.status()` method of a
   * {@linkcode Deno.Process} instance.
   *
   * If `success` is `true`, then `code` will be `0`, but if `success` is
   * `false`, the sub-process exit code will be set in `code`.
   *
   * @category Sub Process */
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

  /**
   * Represents an instance of a sub process that is returned from
   * {@linkcode Deno.run} which can be used to manage the sub-process.
   *
   * @category Sub Process */
  export class Process<T extends RunOptions = RunOptions> {
    /** The resource ID of the sub-process. */
    readonly rid: number;
    /** The operating system's process ID for the sub-process. */
    readonly pid: number;
    /** A reference to the sub-processes `stdin`, which allows interacting with
     * the sub-process at a low level. */
    readonly stdin: T["stdin"] extends "piped" ? Writer & Closer & {
        writable: WritableStream<Uint8Array>;
      }
      : (Writer & Closer & { writable: WritableStream<Uint8Array> }) | null;
    /** A reference to the sub-processes `stdout`, which allows interacting with
     * the sub-process at a low level. */
    readonly stdout: T["stdout"] extends "piped" ? Reader & Closer & {
        readable: ReadableStream<Uint8Array>;
      }
      : (Reader & Closer & { readable: ReadableStream<Uint8Array> }) | null;
    /** A reference to the sub-processes `stderr`, which allows interacting with
     * the sub-process at a low level. */
    readonly stderr: T["stderr"] extends "piped" ? Reader & Closer & {
        readable: ReadableStream<Uint8Array>;
      }
      : (Reader & Closer & { readable: ReadableStream<Uint8Array> }) | null;
    /** Wait for the process to exit and return its exit status.
     *
     * Calling this function multiple times will return the same status.
     *
     * The `stdin` reference to the process will be closed before waiting to
     * avoid a deadlock.
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
     * You must set `stdout` to `"piped"` when creating the process.
     *
     * This calls `close()` on stdout after its done. */
    output(): Promise<Uint8Array>;
    /** Buffer the stderr until EOF and return it as `Uint8Array`.
     *
     * You must set `stderr` to `"piped"` when creating the process.
     *
     * This calls `close()` on stderr after its done. */
    stderrOutput(): Promise<Uint8Array>;
    /** Clean up resources associated with the sub-process instance. */
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

  /** Operating signals which can be listened for or sent to sub-processes. What
   * signals and what their standard behaviors are are OS dependent.
   *
   * @category Runtime Environment */
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
   * Deno.addSignalListener(
   *   "SIGTERM",
   *   () => {
   *     console.log("SIGTERM!")
   *   }
   * );
   * ```
   *
   * _Note_: On Windows only `"SIGINT"` (CTRL+C) and `"SIGBREAK"` (CTRL+Break)
   * are supported.
   *
   * @category Runtime Environment
   */
  export function addSignalListener(signal: Signal, handler: () => void): void;

  /** Removes the given signal listener that has been registered with
   * {@linkcode Deno.addSignalListener}.
   *
   * ```ts
   * const listener = () => {
   *   console.log("SIGTERM!")
   * };
   * Deno.addSignalListener("SIGTERM", listener);
   * Deno.removeSignalListener("SIGTERM", listener);
   * ```
   *
   * _Note_: On Windows only `"SIGINT"` (CTRL+C) and `"SIGBREAK"` (CTRL+Break)
   * are supported.
   *
   * @category Runtime Environment
   */
  export function removeSignalListener(
    signal: Signal,
    handler: () => void,
  ): void;

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
   * By default subprocess inherits stdio of parent process. To change
   * this this, `opt.stdin`, `opt.stdout`, and `opt.stderr` can be set
   * independently to a resource ID (_rid_) of an open file, `"inherit"`,
   * `"piped"`, or `"null"`:
   *
   * - _number_: the resource ID of an open file/resource. This allows you to
   *   read or write to a file.
   * - `"inherit"`: The default if unspecified. The subprocess inherits from the
   *   parent.
   * - `"piped"`: A new pipe should be arranged to connect the parent and child
   *   sub-process.
   * - `"null"`: This stream will be ignored. This is the equivalent of attaching
   *   the stream to `/dev/null`.
   *
   * Details of the spawned process are returned as an instance of
   * {@linkcode Deno.Process}.
   *
   * Requires `allow-run` permission.
   *
   * @tags allow-run
   * @category Sub Process
   */
  export function run<T extends RunOptions = RunOptions>(opt: T): Process<T>;

  /** Option which can be specified when performing {@linkcode Deno.inspect}.
   *
   * @category Console and Debugging */
  export interface InspectOptions {
    /** Stylize output with ANSI colors. Defaults to `false`. */
    colors?: boolean;
    /** Try to fit more than one entry of a collection on the same line.
     * Defaults to `true`. */
    compact?: boolean;
    /** Traversal depth for nested objects. Defaults to `4`. */
    depth?: number;
    /** The maximum number of iterable entries to print. Defaults to `100`. */
    iterableLimit?: number;
    /** Show a Proxy's target and handler. Defaults to `false`. */
    showProxy?: boolean;
    /** Sort Object, Set and Map entries by key. Defaults to `false`. */
    sorted?: boolean;
    /** Add a trailing comma for multiline collections. Defaults to `false`. */
    trailingComma?: boolean;
    /*** Evaluate the result of calling getters. Defaults to `false`. */
    getters?: boolean;
    /** Show an object's non-enumerable properties. Defaults to `false`. */
    showHidden?: boolean;
    /** The maximum length of a string before it is truncated with an
     * ellipsis. */
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
   * A custom inspect functions can be registered on objects, via the symbol
   * `Symbol.for("Deno.customInspect")`, to control and customize the output
   * of `inspect()` or when using `console` logging:
   *
   * ```ts
   * class A {
   *   x = 10;
   *   y = "hello";
   *   [Symbol.for("Deno.customInspect")]() {
   *     return `x=${this.x}, y=${this.y}`;
   *   }
   * }
   *
   * const inStringFormat = Deno.inspect(new A()); // "x=10, y=hello"
   * console.log(inStringFormat);  // prints "x=10, y=hello"
   * ```
   *
   * A depth can be specified by using the `depth` option:
   *
   * ```ts
   * Deno.inspect({a: {b: {c: {d: 'hello'}}}}, {depth: 2}); // { a: { b: [Object] } }
   * ```
   *
   * @category Console and Debugging
   */
  export function inspect(value: unknown, options?: InspectOptions): string;

  /** The name of a privileged feature which needs permission.
   *
   * @category Permissions
   */
  export type PermissionName =
    | "run"
    | "read"
    | "write"
    | "net"
    | "env"
    | "sys"
    | "ffi"
    | "hrtime";

  /** The current status of the permission:
   *
   * - `"granted"` - the permission has been granted.
   * - `"denied"` - the permission has been explicitly denied.
   * - `"prompt"` - the permission has not explicitly granted nor denied.
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
  export interface SysPermissionDescriptor {
    name: "sys";
    kind?:
      | "loadavg"
      | "hostname"
      | "systemMemoryInfo"
      | "networkInterfaces"
      | "osRelease"
      | "getUid"
      | "getGid";
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
    | SysPermissionDescriptor
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
   * Note that this function *consumes* the given connection passed to it, thus the
   * original connection will be unusable after calling this. Additionally, you
   * need to ensure that the connection is not being used elsewhere when calling
   * this function in order for the connection to be consumed properly.
   * For instance, if there is a `Promise` that is waiting for read operation on
   * the connection to complete, it is considered that the connection is being
   * used elsewhere. In such a case, this function will fail.
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

  /**
   * Make the timer of the given `id` block the event loop from finishing.
   *
   * @category Timers
   */
  export function refTimer(id: number): void;

  /**
   * Make the timer of the given `id` not block the event loop from finishing.
   *
   * @category Timers
   */
  export function unrefTimer(id: number): void;
}
