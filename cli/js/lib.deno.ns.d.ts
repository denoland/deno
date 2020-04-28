// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  /** The current process id of the runtime. */
  export let pid: number;

  /** Reflects the `NO_COLOR` environment variable.
   *
   * See: https://no-color.org/ */
  export let noColor: boolean;

  export interface TestDefinition {
    fn: () => void | Promise<void>;
    name: string;
    ignore?: boolean;
    disableOpSanitizer?: boolean;
    disableResourceSanitizer?: boolean;
  }

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module, or explicitly
   * when `Deno.runTests` is used.  `fn` can be async if required.
   *
   *          import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   *          Deno.test({
   *            name: "example test",
   *            fn(): void {
   *              assertEquals("world", "world");
   *            },
   *          });
   *
   *          Deno.test({
   *            name: "example ignored test",
   *            ignore: Deno.build.os === "win"
   *            fn(): void {
   *              //This test is ignored only on Windows machines
   *            },
   *          });
   *
   *          Deno.test({
   *            name: "example async test",
   *            async fn() {
   *              const decoder = new TextDecoder("utf-8");
   *              const data = await Deno.readFile("hello_world.txt");
   *              assertEquals(decoder.decode(data), "Hello world")
   *            }
   *          });
   */
  export function test(t: TestDefinition): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module, or explicitly
   * when `Deno.runTests` is used
   *
   *        import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   *        Deno.test(function myTestFunction():void {
   *          assertEquals("hello", "hello");
   *        });
   *
   *        Deno.test(async function myAsyncTestFunction():Promise<void> {
   *          const decoder = new TextDecoder("utf-8");
   *          const data = await Deno.readFile("hello_world.txt");
   *          assertEquals(decoder.decode(data), "Hello world")
   *        });
   **/
  export function test(fn: () => void | Promise<void>): void;

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module, or explicitly
   * when `Deno.runTests` is used
   *
   *        import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   *        Deno.test("My test description", ():void => {
   *          assertEquals("hello", "hello");
   *        });
   *
   *        Deno.test("My async test description", async ():Promise<void> => {
   *          const decoder = new TextDecoder("utf-8");
   *          const data = await Deno.readFile("hello_world.txt");
   *          assertEquals(decoder.decode(data), "Hello world")
   *        });
   * */
  export function test(name: string, fn: () => void | Promise<void>): void;

  export interface TestMessage {
    start?: {
      tests: TestDefinition[];
    };
    testStart?: {
      [P in keyof TestDefinition]: TestDefinition[P];
    };
    testEnd?: {
      name: string;
      status: "passed" | "failed" | "ignored";
      duration: number;
      error?: Error;
    };
    end?: {
      filtered: number;
      ignored: number;
      measured: number;
      passed: number;
      failed: number;
      duration: number;
      results: Array<TestMessage["testEnd"] & {}>;
    };
  }

  export interface RunTestsOptions {
    /** If `true`, Deno will exit with status code 1 if there was
     * test failure. Defaults to `true`. */
    exitOnFail?: boolean;
    /** If `true`, Deno will exit upon first test failure. Defaults to `false`. */
    failFast?: boolean;
    /** String or RegExp used to filter test to run. Only test with names
     * matching provided `String` or `RegExp` will be run. */
    filter?: string | RegExp;
    /** String or RegExp used to skip tests to run. Tests with names
     * matching provided `String` or `RegExp` will not be run. */
    skip?: string | RegExp;
    /** Disable logging of the results. Defaults to `false`. */
    disableLog?: boolean;
    /** If true, report results to the console as is done for `deno test`. Defaults to `true`. */
    reportToConsole?: boolean;
    /** Called for each message received from the test run. */
    onMessage?: (message: TestMessage) => void | Promise<void>;
  }

  /** Run any tests which have been registered via `Deno.test()`. Always resolves
   * asynchronously.
   *
   *        //Register test
   *        Deno.test({
   *          name: "example test",
   *          fn(): void {
   *            assertEquals("world", "world");
   *            assertEquals({ hello: "world" }, { hello: "world" });
   *          },
   *        });
   *
   *        //Run tests
   *        const runInfo = await Deno.runTests();
   *        console.log(runInfo.duration);  // all tests duration, e.g. "5" (in ms)
   *        console.log(runInfo.stats.passed);  //e.g. 1
   *        console.log(runInfo.results[0].name);  //e.g. "example test"
   */
  export function runTests(
    opts?: RunTestsOptions
  ): Promise<TestMessage["end"]> & {};

  /** Returns an array containing the 1, 5, and 15 minute load averages. The
   * load average is a measure of CPU and IO utilization of the last one, five,
   * and 15 minute periods expressed as a fractional number.  Zero means there
   * is no load. On Windows, the three values are always the same and represent
   * the current load, not the 1, 5 and 15 minute load averages.
   *
   *       console.log(Deno.loadavg());  //e.g. [ 0.71, 0.44, 0.44 ]
   *
   * Requires `allow-env` permission.
   */
  export function loadavg(): number[];

  /** Get the `hostname` of the machine the Deno process is running on.
   *
   *       console.log(Deno.hostname());
   *
   *  Requires `allow-env` permission.
   */
  export function hostname(): string;

  /** Returns the release version of the Operating System.
   *
   *       console.log(Deno.osRelease());
   *
   * Requires `allow-env` permission.
   */
  export function osRelease(): string;

  /** Exit the Deno process with optional exit code. If no exit code is supplied
   * then Deno will exit with return code of 0.
   *
   *       Deno.exit(5);
   */
  export function exit(code?: number): never;

  /** Returns a snapshot of the environment variables at invocation. Changing a
   * property in the object will set that variable in the environment for the
   * process. The environment object will only accept `string`s as values.
   *
   *       const myEnv = Deno.env();
   *       console.log(myEnv.SHELL);
   *       myEnv.TEST_VAR = "HELLO";
   *       const newEnv = Deno.env();
   *       console.log(myEnv.TEST_VAR === newEnv.TEST_VAR);  //outputs "true"
   *
   * Requires `allow-env` permission. */
  export function env(): {
    [index: string]: string;
  };

  /** Retrieve the value of an environment variable. Returns undefined if that
   * key doesn't exist.
   *
   *       console.log(Deno.env("HOME"));  //e.g. outputs "/home/alice"
   *       console.log(Deno.env("MADE_UP_VAR"));  //outputs "Undefined"
   *
   * Requires `allow-env` permission. */
  export function env(key: string): string | undefined;

  /** **UNSTABLE** */
  export type DirKind =
    | "home"
    | "cache"
    | "config"
    | "executable"
    | "data"
    | "data_local"
    | "audio"
    | "desktop"
    | "document"
    | "download"
    | "font"
    | "picture"
    | "public"
    | "template"
    | "tmp"
    | "video";

  /**
   * **UNSTABLE**: Currently under evaluation to decide if method name `dir` and
   * parameter type alias name `DirKind` should be renamed.
   *
   * Returns the user and platform specific directories.
   *
   *       const homeDirectory = Deno.dir("home");
   *
   * Requires `allow-env` permission.
   *
   * Returns `null` if there is no applicable directory or if any other error
   * occurs.
   *
   * Argument values: `"home"`, `"cache"`, `"config"`, `"executable"`, `"data"`,
   * `"data_local"`, `"audio"`, `"desktop"`, `"document"`, `"download"`,
   * `"font"`, `"picture"`, `"public"`, `"template"`, `"tmp"`, `"video"`
   *
   * `"home"`
   *
   * |Platform | Value                                    | Example                |
   * | ------- | -----------------------------------------| -----------------------|
   * | Linux   | `$HOME`                                  | /home/alice            |
   * | macOS   | `$HOME`                                  | /Users/alice           |
   * | Windows | `{FOLDERID_Profile}`                     | C:\Users\Alice         |
   *
   * `"cache"`
   *
   * |Platform | Value                               | Example                      |
   * | ------- | ----------------------------------- | ---------------------------- |
   * | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
   * | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
   * | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
   *
   * `"config"`
   *
   * |Platform | Value                                 | Example                          |
   * | ------- | ------------------------------------- | -------------------------------- |
   * | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config              |
   * | macOS   | `$HOME`/Library/Preferences           | /Users/Alice/Library/Preferences |
   * | Windows | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming   |
   *
   * `"executable"`
   *
   * |Platform | Value                                                           | Example                |
   * | ------- | --------------------------------------------------------------- | -----------------------|
   * | Linux   | `XDG_BIN_HOME` or `$XDG_DATA_HOME`/../bin or `$HOME`/.local/bin | /home/alice/.local/bin |
   * | macOS   | -                                                               | -                      |
   * | Windows | -                                                               | -                      |
   *
   * `"data"`
   *
   * |Platform | Value                                    | Example                                  |
   * | ------- | ---------------------------------------- | ---------------------------------------- |
   * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
   * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
   * | Windows | `{FOLDERID_RoamingAppData}`              | C:\Users\Alice\AppData\Roaming           |
   *
   * `"data_local"`
   *
   * |Platform | Value                                    | Example                                  |
   * | ------- | ---------------------------------------- | ---------------------------------------- |
   * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
   * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
   * | Windows | `{FOLDERID_LocalAppData}`                | C:\Users\Alice\AppData\Local             |
   *
   * `"audio"`
   *
   * |Platform | Value              | Example              |
   * | ------- | ------------------ | -------------------- |
   * | Linux   | `XDG_MUSIC_DIR`    | /home/alice/Music    |
   * | macOS   | `$HOME`/Music      | /Users/Alice/Music   |
   * | Windows | `{FOLDERID_Music}` | C:\Users\Alice\Music |
   *
   * `"desktop"`
   *
   * |Platform | Value                | Example                |
   * | ------- | -------------------- | ---------------------- |
   * | Linux   | `XDG_DESKTOP_DIR`    | /home/alice/Desktop    |
   * | macOS   | `$HOME`/Desktop      | /Users/Alice/Desktop   |
   * | Windows | `{FOLDERID_Desktop}` | C:\Users\Alice\Desktop |
   *
   * `"document"`
   *
   * |Platform | Value                  | Example                  |
   * | ------- | ---------------------- | ------------------------ |
   * | Linux   | `XDG_DOCUMENTS_DIR`    | /home/alice/Documents    |
   * | macOS   | `$HOME`/Documents      | /Users/Alice/Documents   |
   * | Windows | `{FOLDERID_Documents}` | C:\Users\Alice\Documents |
   *
   * `"download"`
   *
   * |Platform | Value                  | Example                  |
   * | ------- | ---------------------- | ------------------------ |
   * | Linux   | `XDG_DOWNLOAD_DIR`     | /home/alice/Downloads    |
   * | macOS   | `$HOME`/Downloads      | /Users/Alice/Downloads   |
   * | Windows | `{FOLDERID_Downloads}` | C:\Users\Alice\Downloads |
   *
   * `"font"`
   *
   * |Platform | Value                                                | Example                        |
   * | ------- | ---------------------------------------------------- | ------------------------------ |
   * | Linux   | `$XDG_DATA_HOME`/fonts or `$HOME`/.local/share/fonts | /home/alice/.local/share/fonts |
   * | macOS   | `$HOME/Library/Fonts`                                | /Users/Alice/Library/Fonts     |
   * | Windows | –                                                    | –                              |
   *
   * `"picture"`
   *
   * |Platform | Value                 | Example                 |
   * | ------- | --------------------- | ----------------------- |
   * | Linux   | `XDG_PICTURES_DIR`    | /home/alice/Pictures    |
   * | macOS   | `$HOME`/Pictures      | /Users/Alice/Pictures   |
   * | Windows | `{FOLDERID_Pictures}` | C:\Users\Alice\Pictures |
   *
   * `"public"`
   *
   * |Platform | Value                 | Example             |
   * | ------- | --------------------- | ------------------- |
   * | Linux   | `XDG_PUBLICSHARE_DIR` | /home/alice/Public  |
   * | macOS   | `$HOME`/Public        | /Users/Alice/Public |
   * | Windows | `{FOLDERID_Public}`   | C:\Users\Public     |
   *
   * `"template"`
   *
   * |Platform | Value                  | Example                                                    |
   * | ------- | ---------------------- | ---------------------------------------------------------- |
   * | Linux   | `XDG_TEMPLATES_DIR`    | /home/alice/Templates                                      |
   * | macOS   | –                      | –                                                          |
   * | Windows | `{FOLDERID_Templates}` | C:\Users\Alice\AppData\Roaming\Microsoft\Windows\Templates |
   *
   * `"tmp"`
   *
   * |Platform | Value                  | Example                                                    |
   * | ------- | ---------------------- | ---------------------------------------------------------- |
   * | Linux   | `TMPDIR`               | /tmp                                                       |
   * | macOS   | `TMPDIR`               | /tmp                                                       |
   * | Windows | `{TMP}`                | C:\Users\Alice\AppData\Local\Temp                          |
   *
   * `"video"`
   *
   * |Platform | Value               | Example               |
   * | ------- | ------------------- | --------------------- |
   * | Linux   | `XDG_VIDEOS_DIR`    | /home/alice/Videos    |
   * | macOS   | `$HOME`/Movies      | /Users/Alice/Movies   |
   * | Windows | `{FOLDERID_Videos}` | C:\Users\Alice\Videos |
   *
   */
  export function dir(kind: DirKind): string | null;

  /**
   * Returns the path to the current deno executable.
   *
   *       console.log(Deno.execPath());  //e.g. "/home/alice/.local/bin/deno"
   *
   * Requires `allow-env` permission.
   */
  export function execPath(): string;

  /**
   * **UNSTABLE**: Currently under evaluation to decide if explicit permission is
   * required to get the value of the current working directory.
   *
   * Return a string representing the current working directory.
   *
   * If the current directory can be reached via multiple paths (due to symbolic
   * links), `cwd()` may return any one of them.
   *
   *       const currentWorkingDirectory = Deno.cwd();
   *
   * Throws `Deno.errors.NotFound` if directory not available.
   */
  export function cwd(): string;

  /**
   * **UNSTABLE**: Currently under evaluation to decide if explicit permission is
   * required to change the current working directory.
   *
   * Change the current working directory to the specified path.
   *
   *       Deno.chdir("/home/userA");
   *       Deno.chdir("../userB");
   *       Deno.chdir("C:\\Program Files (x86)\\Java");
   *
   * Throws `Deno.errors.NotFound` if directory not found.
   * Throws `Deno.errors.PermissionDenied` if the user does not have access
   * rights
   */
  export function chdir(directory: string): void;

  /**
   * **UNSTABLE**: New API, yet to be vetted.  This API is under consideration to
   * determine if permissions are required to call it.
   *
   * Retrieve the process umask.  If `mask` is provided, sets the process umask.
   * This call always returns what the umask was before the call.
   *
   *        console.log(Deno.umask());  //e.g. 18 (0o022)
   *        const prevUmaskValue = Deno.umask(0o077);  //e.g. 18 (0o022)
   *        console.log(Deno.umask());  //e.g. 63 (0o077)
   *
   * NOTE:  This API is not implemented on Windows
   */
  export function umask(mask?: number): number;

  /** **UNSTABLE**: might move to `Deno.symbols`. */
  export const EOF: unique symbol;
  export type EOF = typeof EOF;

  /** **UNSTABLE**: might remove `"SEEK_"` prefix. Might not use all-caps. */
  export enum SeekMode {
    SEEK_START = 0,
    SEEK_CURRENT = 1,
    SEEK_END = 2,
  }

  /** **UNSTABLE**: might make `Reader` into iterator of some sort. */
  export interface Reader {
    /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number of
     * bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
     * encountered. Even if `read()` resolves to `n` < `p.byteLength`, it may
     * use all of `p` as scratch space during the call. If some data is
     * available but not `p.byteLength` bytes, `read()` conventionally resolves
     * to what is available instead of waiting for more.
     *
     * When `read()` encounters end-of-file condition, it resolves to
     * `Deno.EOF` symbol.
     *
     * When `read()` encounters an error, it rejects with an error.
     *
     * Callers should always process the `n` > `0` bytes returned before
     * considering the `EOF`. Doing so correctly handles I/O errors that happen
     * after reading some bytes and also both of the allowed EOF behaviors.
     *
     * Implementations should not retain a reference to `p`.
     */
    read(p: Uint8Array): Promise<number | EOF>;
  }

  export interface SyncReader {
    /** Reads up to `p.byteLength` bytes into `p`. It resolves to the number
     * of bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error
     * encountered. Even if `read()` returns `n` < `p.byteLength`, it may use
     * all of `p` as scratch space during the call. If some data is available
     * but not `p.byteLength` bytes, `read()` conventionally returns what is
     * available instead of waiting for more.
     *
     * When `readSync()` encounters end-of-file condition, it returns `Deno.EOF`
     * symbol.
     *
     * When `readSync()` encounters an error, it throws with an error.
     *
     * Callers should always process the `n` > `0` bytes returned before
     * considering the `EOF`. Doing so correctly handles I/O errors that happen
     * after reading some bytes and also both of the allowed EOF behaviors.
     *
     * Implementations should not retain a reference to `p`.
     */
    readSync(p: Uint8Array): number | EOF;
  }

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

  export interface SyncWriter {
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

  export interface Closer {
    close(): void;
  }

  export interface Seeker {
    /** Seek sets the offset for the next `read()` or `write()` to offset,
     * interpreted according to `whence`: `SEEK_START` means relative to the
     * start of the file, `SEEK_CURRENT` means relative to the current offset,
     * and `SEEK_END` means relative to the end. Seek resolves to the new offset
     * relative to the start of the file.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O
     * operations on the underlying object is implementation-dependent.
     * It returns the number of cursor position.
     */
    seek(offset: number, whence: SeekMode): Promise<number>;
  }

  export interface SyncSeeker {
    /** Seek sets the offset for the next `readSync()` or `writeSync()` to
     * offset, interpreted according to `whence`: `SEEK_START` means relative
     * to the start of the file, `SEEK_CURRENT` means relative to the current
     * offset, and `SEEK_END` means relative to the end.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O
     * operations on the underlying object is implementation-dependent.
     */
    seekSync(offset: number, whence: SeekMode): number;
  }

  export interface ReadCloser extends Reader, Closer {}
  export interface WriteCloser extends Writer, Closer {}
  export interface ReadSeeker extends Reader, Seeker {}
  export interface WriteSeeker extends Writer, Seeker {}
  export interface ReadWriteCloser extends Reader, Writer, Closer {}
  export interface ReadWriteSeeker extends Reader, Writer, Seeker {}

  /** Copies from `src` to `dst` until either `EOF` is reached on `src` or an
   * error occurs. It resolves to the number of bytes copied or rejects with
   * the first error encountered while copying.
   *
   *       const source = await Deno.open("my_file.txt");
   *       const buffer = new Deno.Buffer()
   *       const bytesCopied1 = await Deno.copy(Deno.stdout, source);
   *       const bytesCopied2 = await Deno.copy(buffer, source);
   *
   * Because `copy()` is defined to read from `src` until `EOF`, it does not
   * treat an `EOF` from `read()` as an error to be reported.
   *
   * @param dst The destination to copy to
   * @param src The source to copy from
   */
  export function copy(dst: Writer, src: Reader): Promise<number>;

  /** Turns a Reader, `r`, into an async iterator.
   *
   *      for await (const chunk of toAsyncIterator(reader)) {
   *        console.log(chunk);
   *      }
   */
  export function toAsyncIterator(r: Reader): AsyncIterableIterator<Uint8Array>;

  /** Synchronously open a file and return an instance of `Deno.File`.  The
   * file does not need to previously exist if using the `create` or `createNew`
   * open options.  It is the callers responsibility to close the file when finished
   * with it.
   *
   *       const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
   *       // Do work with file
   *       Deno.close(file.rid);
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on options.
   */
  export function openSync(path: string, options?: OpenOptions): File;

  /** Synchronously open a file and return an instance of `Deno.File`.  The file
   * may be created depending on the mode passed in.  It is the callers responsibility
   * to close the file when finished with it.
   *
   *       const file = Deno.openSync("/foo/bar.txt", "r");
   *       // Do work with file
   *       Deno.close(file.rid);
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on openMode.
   */
  export function openSync(path: string, openMode?: OpenMode): File;

  /** Open a file and resolve to an instance of `Deno.File`.  The
   * file does not need to previously exist if using the `create` or `createNew`
   * open options.  It is the callers responsibility to close the file when finished
   * with it.
   *
   *       const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
   *       // Do work with file
   *       Deno.close(file.rid);
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on options.
   */
  export function open(path: string, options?: OpenOptions): Promise<File>;

  /** Open a file and resolve to an instance of `Deno.File`.  The file may be
   * created depending on the mode passed in.  It is the callers responsibility
   * to close the file when finished with it.
   *
   *       const file = await Deno.open("/foo/bar.txt", "w+");
   *       // Do work with file
   *       Deno.close(file.rid);
   *
   * Requires `allow-read` and/or `allow-write` permissions depending on openMode.
   */
  export function open(path: string, openMode?: OpenMode): Promise<File>;

  /** Creates a file if none exists or truncates an existing file and returns
   *  an instance of `Deno.File`.
   *
   *       const file = Deno.createSync("/foo/bar.txt");
   *
   * Requires `allow-read` and `allow-write` permissions.
   */
  export function createSync(path: string): File;

  /** Creates a file if none exists or truncates an existing file and resolves to
   *  an instance of `Deno.File`.
   *
   *       const file = await Deno.create("/foo/bar.txt");
   *
   * Requires `allow-read` and `allow-write` permissions.
   */
  export function create(path: string): Promise<File>;

  /** Synchronously read from a resource ID (`rid`) into an array buffer (`buffer`).
   *
   * Returns either the number of bytes read during the operation or End Of File
   * (`Symbol(EOF)`) if there was nothing to read.
   *
   *      // if "/foo/bar.txt" contains the text "hello world":
   *      const file = Deno.openSync("/foo/bar.txt");
   *      const buf = new Uint8Array(100);
   *      const numberOfBytesRead = Deno.readSync(file.rid, buf); // 11 bytes
   *      const text = new TextDecoder().decode(buf);  // "hello world"
   *      Deno.close(file.rid);
   */
  export function readSync(rid: number, buffer: Uint8Array): number | EOF;

  /** Read from a resource ID (`rid`) into an array buffer (`buffer`).
   *
   * Resolves to either the number of bytes read during the operation or End Of
   * File (`Symbol(EOF)`) if there was nothing to read.
   *
   *      // if "/foo/bar.txt" contains the text "hello world":
   *      const file = await Deno.open("/foo/bar.txt");
   *      const buf = new Uint8Array(100);
   *      const numberOfBytesRead = await Deno.read(file.rid, buf); // 11 bytes
   *      const text = new TextDecoder().decode(buf);  // "hello world"
   *      Deno.close(file.rid);
   */
  export function read(rid: number, buffer: Uint8Array): Promise<number | EOF>;

  /** Synchronously write to the resource ID (`rid`) the contents of the array
   * buffer (`data`).
   *
   * Returns the number of bytes written.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world");
   *       const file = Deno.openSync("/foo/bar.txt");
   *       const bytesWritten = Deno.writeSync(file.rid, data); // 11
   *       Deno.close(file.rid);
   */
  export function writeSync(rid: number, data: Uint8Array): number;

  /** Write to the resource ID (`rid`) the contents of the array buffer (`data`).
   *
   * Resolves to the number of bytes written.
   *
   *      const encoder = new TextEncoder();
   *      const data = encoder.encode("Hello world");
   *      const file = await Deno.open("/foo/bar.txt");
   *      const bytesWritten = await Deno.write(file.rid, data); // 11
   *      Deno.close(file.rid);
   */
  export function write(rid: number, data: Uint8Array): Promise<number>;

  /** Synchronously seek a resource ID (`rid`) to the given `offset` under mode
   * given by `whence`.  The new position within the resource (bytes from the
   * start) is returned.
   *
   *        const file = Deno.openSync('hello.txt', {read: true, write: true, truncate: true, create: true});
   *        Deno.writeSync(file.rid, new TextEncoder().encode("Hello world"));
   *        //advance cursor 6 bytes
   *        const cursorPosition = Deno.seekSync(file.rid, 6, Deno.SeekMode.SEEK_START);
   *        console.log(cursorPosition);  // 6
   *        const buf = new Uint8Array(100);
   *        file.readSync(buf);
   *        console.log(new TextDecoder().decode(buf)); // "world"
   *
   * The seek modes work as follows:
   *
   *        //Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   *        //Seek 6 bytes from the start of the file
   *        console.log(Deno.seekSync(file.rid, 6, Deno.SeekMode.SEEK_START)); //"6"
   *        //Seek 2 more bytes from the current position
   *        console.log(Deno.seekSync(file.rid, 2, Deno.SeekMode.SEEK_CURRENT)); //"8"
   *        //Seek backwards 2 bytes from the end of the file
   *        console.log(Deno.seekSync(file.rid, -2, Deno.SeekMode.SEEK_END)); //"9" (e.g. 11-2)
   */
  export function seekSync(
    rid: number,
    offset: number,
    whence: SeekMode
  ): number;

  /** Seek a resource ID (`rid`) to the given `offset` under mode given by `whence`.
   * The call resolves to the new position within the resource (bytes from the start).
   *
   *        const file = await Deno.open('hello.txt', {read: true, write: true, truncate: true, create: true});
   *        await Deno.write(file.rid, new TextEncoder().encode("Hello world"));
   *        //advance cursor 6 bytes
   *        const cursorPosition = await Deno.seek(file.rid, 6, Deno.SeekMode.SEEK_START);
   *        console.log(cursorPosition);  // 6
   *        const buf = new Uint8Array(100);
   *        await file.read(buf);
   *        console.log(new TextDecoder().decode(buf)); // "world"
   *
   * The seek modes work as follows:
   *
   *        //Given file.rid pointing to file with "Hello world", which is 11 bytes long:
   *        //Seek 6 bytes from the start of the file
   *        console.log(await Deno.seek(file.rid, 6, Deno.SeekMode.SEEK_START)); //"6"
   *        //Seek 2 more bytes from the current position
   *        console.log(await Deno.seek(file.rid, 2, Deno.SeekMode.SEEK_CURRENT)); //"8"
   *        //Seek backwards 2 bytes from the end of the file
   *        console.log(await Deno.seek(file.rid, -2, Deno.SeekMode.SEEK_END)); //"9" (e.g. 11-2)
   */
  export function seek(
    rid: number,
    offset: number,
    whence: SeekMode
  ): Promise<number>;

  /** Close the given resource ID (rid) which has been previously opened, such
   * as via opening or creating a file.  Closing a file when you are finished
   * with it is important to avoid leaking resources.
   *
   *      const file = await Deno.open("my_file.txt");
   *      // do work with "file" object
   *      Deno.close(file.rid);
   */
  export function close(rid: number): void;

  /** The Deno abstraction for reading and writing files. */
  export class File
    implements
      Reader,
      SyncReader,
      Writer,
      SyncWriter,
      Seeker,
      SyncSeeker,
      Closer {
    readonly rid: number;
    constructor(rid: number);
    write(p: Uint8Array): Promise<number>;
    writeSync(p: Uint8Array): number;
    read(p: Uint8Array): Promise<number | EOF>;
    readSync(p: Uint8Array): number | EOF;
    seek(offset: number, whence: SeekMode): Promise<number>;
    seekSync(offset: number, whence: SeekMode): number;
    close(): void;
  }

  /** An instance of `Deno.File` for `stdin`. */
  export const stdin: File;
  /** An instance of `Deno.File` for `stdout`. */
  export const stdout: File;
  /** An instance of `Deno.File` for `stderr`. */
  export const stderr: File;

  export interface OpenOptions {
    /** Sets the option for read access. This option, when `true`, means that the
     * file should be read-able if opened. */
    read?: boolean;
    /** Sets the option for write access. This option, when `true`, means that
     * the file should be write-able if opened. If the file already exists,
     * any write calls on it will overwrite its contents, by default without
     * truncating it. */
    write?: boolean;
    /**Sets the option for the append mode. This option, when `true`, means that
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

  /** A set of string literals which specify how to open a file.
   *
   * |Value |Description                                                                                       |
   * |------|--------------------------------------------------------------------------------------------------|
   * |`"r"` |Read-only. Default. Starts at beginning of file.                                                  |
   * |`"r+"`|Read-write. Start at beginning of file.                                                           |
   * |`"w"` |Write-only. Opens and truncates existing file or creates new one for writing only.                |
   * |`"w+"`|Read-write. Opens and truncates existing file or creates new one for writing and reading.         |
   * |`"a"` |Write-only. Opens existing file or creates new one. Each write appends content to the end of file.|
   * |`"a+"`|Read-write. Behaves like `"a"` and allows to read from file.                                      |
   * |`"x"` |Write-only. Exclusive create - creates new file only if one doesn't exist already.                |
   * |`"x+"`|Read-write. Behaves like `x` and allows reading from file.                                        |
   */
  export type OpenMode = "r" | "r+" | "w" | "w+" | "a" | "a+" | "x" | "x+";

  /** **UNSTABLE**: new API, yet to be vetted
   *
   *  Check if a given resource id (`rid`) is a TTY.
   *
   *       //This example is system and context specific
   *       const nonTTYRid = Deno.openSync("my_file.txt").rid;
   *       const ttyRid = Deno.openSync("/dev/tty6").rid;
   *       console.log(Deno.isatty(nonTTYRid)); // false
   *       console.log(Deno.isatty(ttyRid)); // true
   *       Deno.close(nonTTYRid);
   *       Deno.close(ttyRid);
   */
  export function isatty(rid: number): boolean;

  /** **UNSTABLE**: new API, yet to be vetted
   *
   * Set TTY to be under raw mode or not. In raw mode, characters are read and
   * returned as is, without being processed. All special processing of
   * characters by the terminal is disabled, including echoing input characters.
   * Reading from a TTY device in raw mode is faster than reading from a TTY
   * device in canonical mode.
   *
   *       Deno.setRaw(myTTY.rid, true);
   */
  export function setRaw(rid: number, mode: boolean): void;

  /** A variable-sized buffer of bytes with `read()` and `write()` methods.
   *
   * Based on [Go Buffer](https://golang.org/pkg/bytes/#Buffer). */
  export class Buffer implements Reader, SyncReader, Writer, SyncWriter {
    constructor(ab?: ArrayBuffer);
    /** Returns a slice holding the unread portion of the buffer.
     *
     * The slice is valid for use only until the next buffer modification (that
     * is, only until the next call to a method like `read()`, `write()`,
     * `reset()`, or `truncate()`). The slice aliases the buffer content at
     * least until the next buffer modification, so immediate changes to the
     * slice will affect the result of future reads. */
    bytes(): Uint8Array;
    /** Returns the contents of the unread portion of the buffer as a `string`.
     *
     * **Warning**: if multibyte characters are present when data is flowing
     * through the buffer, this method may result in incorrect strings due to a
     * character being split. */
    toString(): string;
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
     * return, the return is `Deno.EOF`. */
    readSync(p: Uint8Array): number | EOF;
    /** Reads the next `p.length` bytes from the buffer or until the buffer is
     * drained. Resolves to the number of bytes read. If the buffer has no
     * data to return, resolves to `Deno.EOF`. */
    read(p: Uint8Array): Promise<number | EOF>;
    writeSync(p: Uint8Array): number;
    write(p: Uint8Array): Promise<number>;
    /** Grows the buffer's capacity, if necessary, to guarantee space for
     * another `n` bytes. After `.grow(n)`, at least `n` bytes can be written to
     * the buffer without another allocation. If `n` is negative, `.grow()` will
     * throw. If the buffer can't grow it will throw an error.
     *
     * Based on Go Lang's
     * [Buffer.Grow](https://golang.org/pkg/bytes/#Buffer.Grow). */
    grow(n: number): void;
    /** Reads data from `r` until `Deno.EOF` and appends it to the buffer,
     * growing the buffer as needed. It resolves to the number of bytes read.
     * If the buffer becomes too large, `.readFrom()` will reject with an error.
     *
     * Based on Go Lang's
     * [Buffer.ReadFrom](https://golang.org/pkg/bytes/#Buffer.ReadFrom). */
    readFrom(r: Reader): Promise<number>;
    /** Reads data from `r` until `Deno.EOF` and appends it to the buffer,
     * growing the buffer as needed. It returns the number of bytes read. If the
     * buffer becomes too large, `.readFromSync()` will throw an error.
     *
     * Based on Go Lang's
     * [Buffer.ReadFrom](https://golang.org/pkg/bytes/#Buffer.ReadFrom). */
    readFromSync(r: SyncReader): number;
  }

  /** Read Reader `r` until end of file (`Deno.EOF`) and resolve to the content
   * as `Uint8Array`.
   *
   *       //Example from stdin
   *       const stdinContent = await Deno.readAll(Deno.stdin);
   *
   *       //Example from file
   *       const file = await Deno.open("my_file.txt", {read: true});
   *       const myFileContent = await Deno.readAll(file);
   *       Deno.close(file.rid);
   *
   *       //Example from buffer
   *       const myData = new Uint8Array(100);
   *       // ... fill myData array with data
   *       const reader = new Deno.Buffer(myData.buffer as ArrayBuffer);
   *       const bufferContent = await Deno.readAll(reader);
   */
  export function readAll(r: Reader): Promise<Uint8Array>;

  /** Synchronously reads Reader `r` until end of file (`Deno.EOF`) and returns
   * the content as `Uint8Array`.
   *
   *       //Example from stdin
   *       const stdinContent = Deno.readAllSync(Deno.stdin);
   *
   *       //Example from file
   *       const file = Deno.openSync("my_file.txt", {read: true});
   *       const myFileContent = Deno.readAllSync(file);
   *       Deno.close(file.rid);
   *
   *       //Example from buffer
   *       const myData = new Uint8Array(100);
   *       // ... fill myData array with data
   *       const reader = new Deno.Buffer(myData.buffer as ArrayBuffer);
   *       const bufferContent = Deno.readAllSync(reader);
   */
  export function readAllSync(r: SyncReader): Uint8Array;

  /** Write all the content of the array buffer (`arr`) to the writer (`w`).
   *
   *       //Example writing to stdout
   *       const contentBytes = new TextEncoder().encode("Hello World");
   *       await Deno.writeAll(Deno.stdout, contentBytes);
   *
   *       //Example writing to file
   *       const contentBytes = new TextEncoder().encode("Hello World");
   *       const file = await Deno.open('test.file', {write: true});
   *       await Deno.writeAll(file, contentBytes);
   *       Deno.close(file.rid);
   *
   *       //Example writing to buffer
   *       const contentBytes = new TextEncoder().encode("Hello World");
   *       const writer = new Deno.Buffer();
   *       await Deno.writeAll(writer, contentBytes);
   *       console.log(writer.bytes().length);  // 11
   */
  export function writeAll(w: Writer, arr: Uint8Array): Promise<void>;

  /** Synchronously write all the content of the array buffer (`arr`) to the
   * writer (`w`).
   *
   *       //Example writing to stdout
   *       const contentBytes = new TextEncoder().encode("Hello World");
   *       Deno.writeAllSync(Deno.stdout, contentBytes);
   *
   *       //Example writing to file
   *       const contentBytes = new TextEncoder().encode("Hello World");
   *       const file = Deno.openSync('test.file', {write: true});
   *       Deno.writeAllSync(file, contentBytes);
   *       Deno.close(file.rid);
   *
   *       //Example writing to buffer
   *       const contentBytes = new TextEncoder().encode("Hello World");
   *       const writer = new Deno.Buffer();
   *       Deno.writeAllSync(writer, contentBytes);
   *       console.log(writer.bytes().length);  // 11
   */
  export function writeAllSync(w: SyncWriter, arr: Uint8Array): void;

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
   *       Deno.mkdirSync("new_dir");
   *       Deno.mkdirSync("nested/directories", { recursive: true });
   *       Deno.mkdirSync("restricted_access_dir", { mode: 0o700 });
   *
   * Defaults to throwing error if the directory already exists.
   *
   * Requires `allow-write` permission. */
  export function mkdirSync(path: string, options?: MkdirOptions): void;

  /** Creates a new directory with the specified path.
   *
   *       await Deno.mkdir("new_dir");
   *       await Deno.mkdir("nested/directories", { recursive: true });
   *       await Deno.mkdir("restricted_access_dir", { mode: 0o700 });
   *
   * Defaults to throwing error if the directory already exists.
   *
   * Requires `allow-write` permission. */
  export function mkdir(path: string, options?: MkdirOptions): Promise<void>;

  export interface MakeTempOptions {
    /** Directory where the temporary directory should be created (defaults to
     * the env variable TMPDIR, or the system's default, usually /tmp). */
    dir?: string;
    /** String that should precede the random portion of the temporary
     * directory's name. */
    prefix?: string;
    /** String that should follow the random portion of the temporary
     * directory's name. */
    suffix?: string;
  }

  /** Synchronously creates a new temporary directory in the default directory
   * for temporary files (see also `Deno.dir("temp")`), unless `dir` is specified.
   * Other optional options include prefixing and suffixing the directory name
   * with `prefix` and `suffix` respectively.
   *
   * The full path to the newly created directory is returned.
   *
   * Multiple programs calling this function simultaneously will create different
   * directories. It is the caller's responsibility to remove the directory when
   * no longer needed.
   *
   *       const tempDirName0 = Deno.makeTempDirSync();  // e.g. /tmp/2894ea76
   *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });  // e.g. /tmp/my_temp339c944d
   *
   * Requires `allow-write` permission. */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDirSync(options?: MakeTempOptions): string;

  /** Creates a new temporary directory in the default directory for temporary
   * files (see also `Deno.dir("temp")`), unless `dir` is specified.  Other
   * optional options include prefixing and suffixing the directory name with
   * `prefix` and `suffix` respectively.
   *
   * This call resolves to the full path to the newly created directory.
   *
   * Multiple programs calling this function simultaneously will create different
   * directories. It is the caller's responsibility to remove the directory when
   * no longer needed.
   *
   *       const tempDirName0 = await Deno.makeTempDir();  // e.g. /tmp/2894ea76
   *       const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' }); // e.g. /tmp/my_temp339c944d
   *
   * Requires `allow-write` permission. */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDir(options?: MakeTempOptions): Promise<string>;

  /** Synchronously creates a new temporary file in the default directory for
   * temporary files (see also `Deno.dir("temp")`), unless `dir` is specified.
   * Other optional options include prefixing and suffixing the directory name
   * with `prefix` and `suffix` respectively.
   *
   * The full path to the newly created file is returned.
   *
   * Multiple programs calling this function simultaneously will create different
   * files. It is the caller's responsibility to remove the file when no longer
   * needed.
   *
   *       const tempFileName0 = Deno.makeTempFileSync(); // e.g. /tmp/419e0bf2
   *       const tempFileName1 = Deno.makeTempFileSync({ prefix: 'my_temp' });  //e.g. /tmp/my_temp754d3098
   *
   * Requires `allow-write` permission. */
  export function makeTempFileSync(options?: MakeTempOptions): string;

  /** Creates a new temporary file in the default directory for temporary
   * files (see also `Deno.dir("temp")`), unless `dir` is specified.  Other
   * optional options include prefixing and suffixing the directory name with
   * `prefix` and `suffix` respectively.
   *
   * This call resolves to the full path to the newly created file.
   *
   * Multiple programs calling this function simultaneously will create different
   * files. It is the caller's responsibility to remove the file when no longer
   * needed.
   *
   *       const tmpFileName0 = await Deno.makeTempFile();  // e.g. /tmp/419e0bf2
   *       const tmpFileName1 = await Deno.makeTempFile({ prefix: 'my_temp' });  //e.g. /tmp/my_temp754d3098
   *
   * Requires `allow-write` permission. */
  export function makeTempFile(options?: MakeTempOptions): Promise<string>;

  /** Synchronously changes the permission of a specific file/directory of
   * specified path.  Ignores the process's umask.
   *
   *       Deno.chmodSync("/path/to/file", 0o666);
   *
   * For a full description, see [chmod](#chmod)
   *
   * NOTE: This API currently throws on Windows
   *
   * Requires `allow-write` permission. */
  export function chmodSync(path: string, mode: number): void;

  /** Changes the permission of a specific file/directory of specified path.
   * Ignores the process's umask.
   *
   *       await Deno.chmod("/path/to/file", 0o666);
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
   * Requires `allow-write` permission. */
  export function chmod(path: string, mode: number): Promise<void>;

  /** Synchronously change owner of a regular file or directory. This functionality
   * is not available on Windows.
   *
   *      Deno.chownSync("myFile.txt", 1000, 1002);
   *
   * Requires `allow-write` permission.
   *
   * Throws Error (not implemented) if executed on Windows
   *
   * @param path path to the file
   * @param uid user id (UID) of the new owner
   * @param gid group id (GID) of the new owner
   */
  export function chownSync(path: string, uid: number, gid: number): void;

  /** Change owner of a regular file or directory. This functionality
   * is not available on Windows.
   *
   *      await Deno.chown("myFile.txt", 1000, 1002);
   *
   * Requires `allow-write` permission.
   *
   * Throws Error (not implemented) if executed on Windows
   *
   * @param path path to the file
   * @param uid user id (UID) of the new owner
   * @param gid group id (GID) of the new owner
   */
  export function chown(path: string, uid: number, gid: number): Promise<void>;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Synchronously changes the access (`atime`) and modification (`mtime`) times
   * of a file system object referenced by `path`. Given times are either in
   * seconds (UNIX epoch time) or as `Date` objects.
   *
   *       Deno.utimeSync("myfile.txt", 1556495550, new Date());
   *
   * Requires `allow-write` permission. */
  export function utimeSync(
    path: string,
    atime: number | Date,
    mtime: number | Date
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Changes the access (`atime`) and modification (`mtime`) times of a file
   * system object referenced by `path`. Given times are either in seconds
   * (UNIX epoch time) or as `Date` objects.
   *
   *       await Deno.utime("myfile.txt", 1556495550, new Date());
   *
   * Requires `allow-write` permission. */
  export function utime(
    path: string,
    atime: number | Date,
    mtime: number | Date
  ): Promise<void>;

  export interface RemoveOptions {
    /** Defaults to `false`. If set to `true`, path will be removed even if
     * it's a non-empty directory. */
    recursive?: boolean;
  }

  /** Synchronously removes the named file or directory.
   *
   *       Deno.removeSync("/path/to/empty_dir/or/file");
   *       Deno.removeSync("/path/to/populated_dir/or/file", { recursive: true });
   *
   * Throws error if permission denied, path not found, or path is a non-empty
   * directory and the `recursive` option isn't set to `true`.
   *
   * Requires `allow-write` permission. */
  export function removeSync(path: string, options?: RemoveOptions): void;

  /** Removes the named file or directory.
   *
   *       await Deno.remove("/path/to/empty_dir/or/file");
   *       await Deno.remove("/path/to/populated_dir/or/file", { recursive: true });
   *
   * Throws error if permission denied, path not found, or path is a non-empty
   * directory and the `recursive` option isn't set to `true`.
   *
   * Requires `allow-write` permission. */
  export function remove(path: string, options?: RemoveOptions): Promise<void>;

  /** Synchronously renames (moves) `oldpath` to `newpath`. Paths may be files or
   * directories.  If `newpath` already exists and is not a directory,
   * `renameSync()` replaces it. OS-specific restrictions may apply when
   * `oldpath` and `newpath` are in different directories.
   *
   *       Deno.renameSync("old/path", "new/path");
   *
   * On Unix, this operation does not follow symlinks at either path.
   *
   * It varies between platforms when the operation throws errors, and if so what
   * they are. It's always an error to rename anything to a non-empty directory.
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function renameSync(oldpath: string, newpath: string): void;

  /** Renames (moves) `oldpath` to `newpath`.  Paths may be files or directories.
   * If `newpath` already exists and is not a directory, `rename()` replaces it.
   * OS-specific restrictions may apply when `oldpath` and `newpath` are in
   * different directories.
   *
   *       await Deno.rename("old/path", "new/path");
   *
   * On Unix, this operation does not follow symlinks at either path.
   *
   * It varies between platforms when the operation throws errors, and if so what
   * they are. It's always an error to rename anything to a non-empty directory.
   *
   * Requires `allow-read` and `allow-write` permission. */
  export function rename(oldpath: string, newpath: string): Promise<void>;

  /** Synchronously reads and returns the entire contents of a file as an array
   * of bytes. `TextDecoder` can be used to transform the bytes to string if
   * required.  Reading a directory returns an empty data array.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = Deno.readFileSync("hello.txt");
   *       console.log(decoder.decode(data));
   *
   * Requires `allow-read` permission. */
  export function readFileSync(path: string): Uint8Array;

  /** Reads and resolves to the entire contents of a file as an array of bytes.
   * `TextDecoder` can be used to transform the bytes to string if required.
   * Reading a directory returns an empty data array.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = await Deno.readFile("hello.txt");
   *       console.log(decoder.decode(data));
   *
   * Requires `allow-read` permission. */
  export function readFile(path: string): Promise<Uint8Array>;

  /** A FileInfo describes a file and is returned by `stat`, `lstat`,
   * `statSync`, `lstatSync`. */
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
    modified: number | null;
    /** The last access time of the file. This corresponds to the `atime`
     * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
     * be available on all platforms. */
    accessed: number | null;
    /** The last access time of the file. This corresponds to the `birthtime`
     * field from `stat` on Mac/BSD and `ftCreationTime` on Windows. This may not
     * be available on all platforms. */
    created: number | null;
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
    /** User ID of the owner of this file.
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
   *       // e.g. given /home/alice/file.txt and current directory /home/alice
   *       Deno.symlinkSync("file.txt", "symlink_file.txt");
   *       const realPath = Deno.realpathSync("./file.txt");
   *       const realSymLinkPath = Deno.realpathSync("./symlink_file.txt");
   *       console.log(realPath);  // outputs "/home/alice/file.txt"
   *       console.log(realSymLinkPath);  //outputs "/home/alice/file.txt"
   *
   * Requires `allow-read` permission. */
  export function realpathSync(path: string): string;

  /** Resolves to the absolute normalized path, with symbolic links resolved.
   *
   *       // e.g. given /home/alice/file.txt and current directory /home/alice
   *       await Deno.symlink("file.txt", "symlink_file.txt");
   *       const realPath = await Deno.realpath("./file.txt");
   *       const realSymLinkPath = await Deno.realpath("./symlink_file.txt");
   *       console.log(realPath);  // outputs "/home/alice/file.txt"
   *       console.log(realSymLinkPath);  //outputs "/home/alice/file.txt"
   *
   * Requires `allow-read` permission. */
  export function realpath(path: string): Promise<string>;

  export interface DirEntry extends FileInfo {
    name: string;
  }

  /** Synchronously reads the directory given by `path` and returns an iterable
   * of `Deno.DirEntry`.
   *
   *       for (const dirEntry of Deno.readdirSync("/")) {
   *         console.log(dirEntry.name);
   *       }
   *
   * Throws error if `path` is not a directory.
   *
   * Requires `allow-read` permission. */
  export function readdirSync(path: string): Iterable<DirEntry>;

  /** Reads the directory given by `path` and returns an async iterable of
   * `Deno.DirEntry`.
   *
   *       for await (const dirEntry of Deno.readdir("/")) {
   *         console.log(dirEntry.name);
   *       }
   *
   * Throws error if `path` is not a directory.
   *
   * Requires `allow-read` permission. */
  export function readdir(path: string): AsyncIterable<DirEntry>;

  /** Synchronously copies the contents and permissions of one file to another
   * specified path, by default creating a new file if needed, else overwriting.
   * Fails if target path is a directory or is unwritable.
   *
   *       Deno.copyFileSync("from.txt", "to.txt");
   *
   * Requires `allow-read` permission on fromPath.
   * Requires `allow-write` permission on toPath. */
  export function copyFileSync(fromPath: string, toPath: string): void;

  /** Copies the contents and permissions of one file to another specified path,
   * by default creating a new file if needed, else overwriting. Fails if target
   * path is a directory or is unwritable.
   *
   *       await Deno.copyFile("from.txt", "to.txt");
   *
   * Requires `allow-read` permission on fromPath.
   * Requires `allow-write` permission on toPath. */
  export function copyFile(fromPath: string, toPath: string): Promise<void>;

  /** Returns the full path destination of the named symbolic link.
   *
   *       Deno.symlinkSync("./test.txt", "./test_link.txt");
   *       const target = Deno.readlinkSync("./test_link.txt"); // full path of ./test.txt
   *
   * Throws TypeError if called with a hard link
   *
   * Requires `allow-read` permission. */
  export function readlinkSync(path: string): string;

  /** Resolves to the full path destination of the named symbolic link.
   *
   *       await Deno.symlink("./test.txt", "./test_link.txt");
   *       const target = await Deno.readlink("./test_link.txt"); // full path of ./test.txt
   *
   * Throws TypeError if called with a hard link
   *
   * Requires `allow-read` permission. */
  export function readlink(path: string): Promise<string>;

  /** Resolves to a `Deno.FileInfo` for the specified `path`. If `path` is a
   * symlink, information for the symlink will be returned instead of what it
   * points to.
   *
   *       const fileInfo = await Deno.lstat("hello.txt");
   *       assert(fileInfo.isFile);
   *
   * Requires `allow-read` permission. */
  export function lstat(path: string): Promise<FileInfo>;

  /** Synchronously returns a `Deno.FileInfo` for the specified `path`. If
   * `path` is a symlink, information for the symlink will be returned instead of
   * what it points to..
   *
   *       const fileInfo = Deno.lstatSync("hello.txt");
   *       assert(fileInfo.isFile);
   *
   * Requires `allow-read` permission. */
  export function lstatSync(path: string): FileInfo;

  /** Resolves to a `Deno.FileInfo` for the specified `path`. Will always
   * follow symlinks.
   *
   *       const fileInfo = await Deno.stat("hello.txt");
   *       assert(fileInfo.isFile);
   *
   * Requires `allow-read` permission. */
  export function stat(path: string): Promise<FileInfo>;

  /** Synchronously returns a `Deno.FileInfo` for the specified `path`. Will
   * always follow symlinks.
   *
   *       const fileInfo = Deno.statSync("hello.txt");
   *       assert(fileInfo.isFile);
   *
   * Requires `allow-read` permission. */
  export function statSync(path: string): FileInfo;

  /** Synchronously creates `newpath` as a hard link to `oldpath`.
   *
   *       Deno.linkSync("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function linkSync(oldpath: string, newpath: string): void;

  /** Creates `newpath` as a hard link to `oldpath`.
   *
   *       await Deno.link("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function link(oldpath: string, newpath: string): Promise<void>;

  /** **UNSTABLE**: `type` argument type may be changed to `"dir" | "file"`.
   *
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The type argument can be set to `dir` or `file`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * NOTE: This function is not yet implemented on Windows.
   *
   *       Deno.symlinkSync("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function symlinkSync(
    oldpath: string,
    newpath: string,
    type?: string
  ): void;

  /** **UNSTABLE**: `type` argument may be changed to `"dir" | "file"`
   *
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The type argument can be set to `dir` or `file`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * NOTE: This function is not yet implemented on Windows.
   *
   *       await Deno.symlink("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function symlink(
    oldpath: string,
    newpath: string,
    type?: string
  ): Promise<void>;

  /** Options for writing to a file. */
  export interface WriteFileOptions {
    /** Defaults to `false`. If set to `true`, will append to a file instead of
     * overwriting previous contents. */
    append?: boolean;
    /** Sets the option to allow creating a new file, if one doesn't already
     * exist at the specified path (defaults to `true`). */
    create?: boolean;
    /** Permissions always applied to file. */
    mode?: number;
  }

  /** Synchronously write `data` to the given `path`, by default creating a new
   * file if needed, else overwriting.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       Deno.writeFileSync("hello1.txt", data);  //overwrite "hello.txt" or create it
   *       Deno.writeFileSync("hello2.txt", data, {create: false});  //only works if "hello2.txt" exists
   *       Deno.writeFileSync("hello3.txt", data, {mode: 0o777});  //set permissions on new file
   *       Deno.writeFileSync("hello4.txt", data, {append: true});  //add data to the end of the file
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is
   * `false`.
   */
  export function writeFileSync(
    path: string,
    data: Uint8Array,
    options?: WriteFileOptions
  ): void;

  /** Write `data` to the given `path`, by default creating a new file if needed,
   * else overwriting.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       await Deno.writeFile("hello1.txt", data);  //overwrite "hello.txt" or create it
   *       await Deno.writeFile("hello2.txt", data, {create: false});  //only works if "hello2.txt" exists
   *       await Deno.writeFile("hello3.txt", data, {mode: 0o777});  //set permissions on new file
   *       await Deno.writeFile("hello4.txt", data, {append: true});  //add data to the end of the file
   *
   * Requires `allow-write` permission, and `allow-read` if `options.create` is `false`.
   */
  export function writeFile(
    path: string,
    data: Uint8Array,
    options?: WriteFileOptions
  ): Promise<void>;

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

  /** UNSTABLE: new API, yet to be vetted.
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
   * **Note:** `line` and `column` are 1 indexed, which matches display
   * expectations, but is not typical of most index numbers in Deno.
   *
   * An example:
   *
   *       const orig = Deno.applySourceMap({
   *         fileName: "file://my/module.ts",
   *         lineNumber: 5,
   *         columnNumber: 15
   *       });
   *       console.log(`${orig.filename}:${orig.line}:${orig.column}`);
   */
  export function applySourceMap(location: Location): Location;

  /** A set of error constructors that are raised by Deno APIs. */
  export const errors: {
    NotFound: ErrorConstructor;
    PermissionDenied: ErrorConstructor;
    ConnectionRefused: ErrorConstructor;
    ConnectionReset: ErrorConstructor;
    ConnectionAborted: ErrorConstructor;
    NotConnected: ErrorConstructor;
    AddrInUse: ErrorConstructor;
    AddrNotAvailable: ErrorConstructor;
    BrokenPipe: ErrorConstructor;
    AlreadyExists: ErrorConstructor;
    InvalidData: ErrorConstructor;
    TimedOut: ErrorConstructor;
    Interrupted: ErrorConstructor;
    WriteZero: ErrorConstructor;
    UnexpectedEof: ErrorConstructor;
    BadResource: ErrorConstructor;
    Http: ErrorConstructor;
    Busy: ErrorConstructor;
  };

  /** **UNSTABLE**: potentially want names to overlap more with browser.
   *
   * The permissions as granted by the caller.
   *
   * See: https://w3c.github.io/permissions/#permission-registry */
  export type PermissionName =
    | "run"
    | "read"
    | "write"
    | "net"
    | "env"
    | "plugin"
    | "hrtime";

  /** The current status of the permission.
   *
   * See: https://w3c.github.io/permissions/#status-of-a-permission */
  export type PermissionState = "granted" | "denied" | "prompt";

  interface RunPermissionDescriptor {
    name: "run";
  }

  interface ReadWritePermissionDescriptor {
    name: "read" | "write";
    path?: string;
  }

  interface NetPermissionDescriptor {
    name: "net";
    url?: string;
  }

  interface EnvPermissionDescriptor {
    name: "env";
  }

  interface PluginPermissionDescriptor {
    name: "plugin";
  }

  interface HrtimePermissionDescriptor {
    name: "hrtime";
  }

  /** Permission descriptors which define a permission which can be queried,
   * requested, or revoked.
   *
   * See: https://w3c.github.io/permissions/#permission-descriptor */
  type PermissionDescriptor =
    | RunPermissionDescriptor
    | ReadWritePermissionDescriptor
    | NetPermissionDescriptor
    | EnvPermissionDescriptor
    | PluginPermissionDescriptor
    | HrtimePermissionDescriptor;

  export class Permissions {
    /** Resolves to the current status of a permission.
     *
     *       const status = await Deno.permissions.query({ name: "read", path: "/etc" });
     *       if (status.state === "granted") {
     *         data = await Deno.readFile("/etc/passwd");
     *       }
     */
    query(desc: PermissionDescriptor): Promise<PermissionStatus>;

    /** Revokes a permission, and resolves to the state of the permission.
     *
     *       const status = await Deno.permissions.revoke({ name: "run" });
     *       assert(status.state !== "granted")
     */
    revoke(desc: PermissionDescriptor): Promise<PermissionStatus>;

    /** Requests the permission, and resolves to the state of the permission.
     *
     *       const status = await Deno.permissions.request({ name: "env" });
     *       if (status.state === "granted") {
     *         console.log(Deno.homeDir());
     *       } else {
     *         console.log("'env' permission is denied.");
     *       }
     */
    request(desc: PermissionDescriptor): Promise<PermissionStatus>;
  }

  /** **UNSTABLE**: maybe move to `navigator.permissions` to match web API. */
  export const permissions: Permissions;

  /** see: https://w3c.github.io/permissions/#permissionstatus */
  export class PermissionStatus {
    state: PermissionState;
    constructor(state: PermissionState);
  }

  /** Synchronously truncates or extends the specified file, to reach the
   * specified `len`.  If `len` is not specified then the entire file contents
   * are truncated.
   *
   *       //truncate the entire file
   *       Deno.truncateSync("my_file.txt");
   *
   *       //truncate part of the file
   *       const file = Deno.makeTempFileSync();
   *       Deno.writeFileSync(file, new TextEncoder().encode("Hello World"));
   *       Deno.truncateSync(file, 7);
   *       const data = Deno.readFileSync(file);
   *       console.log(new TextDecoder().decode(data));
   *
   * Requires `allow-write` permission. */
  export function truncateSync(name: string, len?: number): void;

  /** Truncates or extends the specified file, to reach the specified `len`. If
   * `len` is not specified then the entire file contents are truncated.
   *
   *       //truncate the entire file
   *       await Deno.truncate("my_file.txt");
   *
   *       //truncate part of the file
   *       const file = await Deno.makeTempFile();
   *       await Deno.writeFile(file, new TextEncoder().encode("Hello World"));
   *       await Deno.truncate(file, 7);
   *       const data = await Deno.readFile(file);
   *       console.log(new TextDecoder().decode(data));  //"Hello W"
   *
   * Requires `allow-write` permission. */
  export function truncate(name: string, len?: number): Promise<void>;

  export interface AsyncHandler {
    (msg: Uint8Array): void;
  }

  export interface PluginOp {
    dispatch(
      control: Uint8Array,
      zeroCopy?: ArrayBufferView | null
    ): Uint8Array | null;
    setAsyncHandler(handler: AsyncHandler): void;
  }

  export interface Plugin {
    ops: {
      [name: string]: PluginOp;
    };
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Open and initalize a plugin.
   *
   *        const plugin = Deno.openPlugin("./path/to/some/plugin.so");
   *        const some_op = plugin.ops.some_op;
   *        const response = some_op.dispatch(new Uint8Array([1,2,3,4]));
   *        console.log(`Response from plugin ${response}`);
   *
   * Requires `allow-plugin` permission. */
  export function openPlugin(filename: string): Plugin;
  export interface NetAddr {
    transport: "tcp" | "udp";
    hostname: string;
    port: number;
  }

  export interface UnixAddr {
    transport: "unix" | "unixpacket";
    address: string;
  }

  export type Addr = NetAddr | UnixAddr;
  /** **UNSTABLE**: Maybe remove `ShutdownMode` entirely.
   *
   * Corresponds to `SHUT_RD`, `SHUT_WR`, `SHUT_RDWR` on POSIX-like systems.
   *
   * See: http://man7.org/linux/man-pages/man2/shutdown.2.html */
  export enum ShutdownMode {
    Read = 0,
    Write,
    ReadWrite, // TODO(ry) panics on ReadWrite.
  }

  /** **UNSTABLE**: Both the `how` parameter and `ShutdownMode` enum are under
   * consideration for removal.
   *
   * Shutdown socket send and receive operations.
   *
   * Matches behavior of POSIX shutdown(3).
   *
   *       const listener = Deno.listen({ port: 80 });
   *       const conn = await listener.accept();
   *       Deno.shutdown(conn.rid, Deno.ShutdownMode.Write);
   */
  export function shutdown(rid: number, how: ShutdownMode): void;

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
    send(p: Uint8Array, addr: Addr): Promise<void>;
    /** UNSTABLE: new API, yet to be vetted.
     *
     * Close closes the socket. Any pending message promises will be rejected
     * with errors. */
    close(): void;
    /** Return the address of the `UDPConn`. */
    readonly addr: Addr;
    [Symbol.asyncIterator](): AsyncIterableIterator<[Uint8Array, Addr]>;
  }

  /** A generic network listener for stream-oriented protocols. */
  export interface Listener extends AsyncIterable<Conn> {
    /** Waits for and resolves to the next connection to the `Listener`. */
    accept(): Promise<Conn>;
    /** Close closes the listener. Any pending accept promises will be rejected
     * with errors. */
    close(): void;
    /** Return the address of the `Listener`. */
    readonly addr: Addr;

    [Symbol.asyncIterator](): AsyncIterableIterator<Conn>;
  }

  export interface Conn extends Reader, Writer, Closer {
    /** The local address of the connection. */
    readonly localAddr: Addr;
    /** The remote address of the connection. */
    readonly remoteAddr: Addr;
    /** The resource ID of the connection. */
    readonly rid: number;
    /** Shuts down (`shutdown(2)`) the reading side of the TCP connection. Most
     * callers should just use `close()`. */
    closeRead(): void;
    /** Shuts down (`shutdown(2)`) the writing side of the TCP connection. Most
     * callers should just use `close()`. */
    closeWrite(): void;
  }

  export interface ListenOptions {
    /** The port to listen on. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
     * If not specified, defaults to `0.0.0.0`. */
    hostname?: string;
  }

  export interface UnixListenOptions {
    /** A Path to the Unix Socket. */
    address: string;
  }
  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   *      const listener1 = Deno.listen({ port: 80 })
   *      const listener2 = Deno.listen({ hostname: "192.0.2.1", port: 80 })
   *      const listener3 = Deno.listen({ hostname: "[2001:db8::1]", port: 80 });
   *      const listener4 = Deno.listen({ hostname: "golang.org", port: 80, transport: "tcp" });
   *
   * Requires `allow-net` permission. */
  export function listen(
    options: ListenOptions & { transport?: "tcp" }
  ): Listener;
  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   *     const listener = Deno.listen({ address: "/foo/bar.sock", transport: "unix" })
   *
   * Requires `allow-read` permission. */
  export function listen(
    options: UnixListenOptions & { transport: "unix" }
  ): Listener;
  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   *      const listener1 = Deno.listen({ port: 80, transport: "udp" })
   *      const listener2 = Deno.listen({ hostname: "golang.org", port: 80, transport: "udp" });
   *
   * Requires `allow-net` permission. */
  export function listen(
    options: ListenOptions & { transport: "udp" }
  ): DatagramConn;
  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   *     const listener = Deno.listen({ address: "/foo/bar.sock", transport: "unixpacket" })
   *
   * Requires `allow-read` permission. */
  export function listen(
    options: UnixListenOptions & { transport: "unixpacket" }
  ): DatagramConn;

  export interface ListenTLSOptions extends ListenOptions {
    /** Server certificate file. */
    certFile: string;
    /** Server public key file. */
    keyFile: string;

    transport?: "tcp";
  }

  /** Listen announces on the local transport address over TLS (transport layer
   * security).
   *
   *      const lstnr = Deno.listenTLS({ port: 443, certFile: "./server.crt", keyFile: "./server.key" });
   *
   * Requires `allow-net` permission. */
  export function listenTLS(options: ListenTLSOptions): Listener;

  export interface ConnectOptions {
    /** The port to connect to. */
    port: number;
    /** A literal IP address or host name that can be resolved to an IP address.
     * If not specified, defaults to `127.0.0.1`. */
    hostname?: string;
    transport?: "tcp";
  }

  export interface UnixConnectOptions {
    transport: "unix";
    address: string;
  }

  /**
   * Connects to the hostname (default is "127.0.0.1") and port on the named
   * transport (default is "tcp"), and resolves to the connection (`Conn`).
   *
   *     const conn1 = await Deno.connect({ port: 80 });
   *     const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
   *     const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
   *     const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
   *     const conn5 = await Deno.connect({ address: "/foo/bar.sock", transport: "unix" });
   *
   * Requires `allow-net` permission for "tcp" and `allow-read` for unix. */
  export function connect(
    options: ConnectOptions | UnixConnectOptions
  ): Promise<Conn>;

  export interface ConnectTLSOptions {
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
   *     const conn1 = await Deno.connectTLS({ port: 80 });
   *     const conn2 = await Deno.connectTLS({ certFile: "./certs/my_custom_root_CA.pem", hostname: "192.0.2.1", port: 80 });
   *     const conn3 = await Deno.connectTLS({ hostname: "[2001:db8::1]", port: 80 });
   *     const conn4 = await Deno.connectTLS({ certFile: "./certs/my_custom_root_CA.pem", hostname: "golang.org", port: 80});
   *
   * Requires `allow-net` permission.
   */
  export function connectTLS(options: ConnectTLSOptions): Promise<Conn>;

  /** **UNSTABLE**: not sure if broken or not */
  export interface Metrics {
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

  /** Receive metrics from the privileged side of Deno.  This is primarily used
   * in the development of Deno. 'Ops', also called 'bindings', are the go-between
   * between Deno Javascript and Deno Rust.
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
   */
  export function metrics(): Metrics;

  /** **UNSTABLE**: reconsider representation. */
  interface ResourceMap {
    [rid: number]: string;
  }

  /** **UNSTABLE**: The return type is under consideration and may change.
   *
   * Returns a map of open _file like_ resource ids (rid) along with their string
   * representations.
   *
   *       console.log(Deno.resources()); //e.g. { 0: "stdin", 1: "stdout", 2: "stderr" }
   *       Deno.openSync('../test.file');
   *       console.log(Deno.resources()); //e.g. { 0: "stdin", 1: "stdout", 2: "stderr", 3: "fsFile" }
   */
  export function resources(): ResourceMap;

  /** **UNSTABLE**: new API. Needs docs. */
  export interface FsEvent {
    kind: "any" | "access" | "create" | "modify" | "remove";
    paths: string[];
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Watch for file system events against one or more `paths`, which can be files
   * or directories.  These paths must exist already.  One user action (e.g.
   * `touch test.file`) can  generate multiple file system events.  Likewise,
   * one user action can result in multiple file paths in one event (e.g. `mv
   * old_name.txt new_name.txt`).  Recursive option is `true` by default and,
   * for directories, will watch the specified directory and all sub directories.
   * Note that the exact ordering of the events can vary between operating systems.
   *
   *       const iter = Deno.fsEvents("/");
   *       for await (const event of iter) {
   *          console.log(">>>> event", event);  //e.g. { kind: "create", paths: [ "/foo.txt" ] }
   *       }
   *
   * Requires `allow-read` permission.
   */
  export function fsEvents(
    paths: string | string[],
    options?: { recursive: boolean }
  ): AsyncIterableIterator<FsEvent>;

  /** How to handle subprocess stdio.
   *
   * `"inherit"` The default if unspecified. The child inherits from the
   * corresponding parent descriptor.
   *
   * `"piped"` A new pipe should be arranged to connect the parent and child
   * sub-processes.
   *
   * `"null"` This stream will be ignored. This is the equivalent of attaching
   * the stream to `/dev/null`. */
  type ProcessStdio = "inherit" | "piped" | "null";

  /** **UNSTABLE**: The `signo` argument may change to require the Deno.Signal
   * enum.
   *
   * Send a signal to process under given `pid`. This functionality currently
   * only works on Linux and Mac OS.
   *
   * If `pid` is negative, the signal will be sent to the process group
   * identified by `pid`.
   *
   *      const p = Deno.run({
   *        cmd: ["python", "-c", "from time import sleep; sleep(10000)"]
   *      });
   *
   *      Deno.kill(p.pid, Deno.Signal.SIGINT);
   *
   * Throws Error (not yet implemented) on Windows
   *
   * Requires `allow-run` permission. */
  export function kill(pid: number, signo: number): void;

  /** **UNSTABLE**: There are some issues to work out with respect to when and
   * how the process should be closed. */
  export class Process {
    readonly rid: number;
    readonly pid: number;
    readonly stdin?: WriteCloser;
    readonly stdout?: ReadCloser;
    readonly stderr?: ReadCloser;
    /** Resolves to the current status of the process. */
    status(): Promise<ProcessStatus>;
    /** Buffer the stdout and return it as `Uint8Array` after `Deno.EOF`.
     *
     * You must set stdout to `"piped"` when creating the process.
     *
     * This calls `close()` on stdout after its done. */
    output(): Promise<Uint8Array>;
    /** Buffer the stderr and return it as `Uint8Array` after `Deno.EOF`.
     *
     * You must set stderr to `"piped"` when creating the process.
     *
     * This calls `close()` on stderr after its done. */
    stderrOutput(): Promise<Uint8Array>;
    close(): void;
    kill(signo: number): void;
  }

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

  /** **UNSTABLE**: `args` has been recently renamed to `cmd` to differentiate from
   * `Deno.args`. */
  export interface RunOptions {
    /** Arguments to pass. Note, the first element needs to be a path to the
     * binary */
    cmd: string[];
    cwd?: string;
    env?: {
      [key: string]: string;
    };
    stdout?: ProcessStdio | number;
    stderr?: ProcessStdio | number;
    stdin?: ProcessStdio | number;
  }

  /** Spawns new subprocess.  RunOptions must contain at a minimum the `opt.cmd`,
   * an array of program arguments, the first of which is the binary.
   *
   * Subprocess uses same working directory as parent process unless `opt.cwd`
   * is specified.
   *
   * Environmental variables for subprocess can be specified using `opt.env`
   * mapping.
   *
   * By default subprocess inherits stdio of parent process. To change that
   * `opt.stdout`, `opt.stderr` and `opt.stdin` can be specified independently -
   * they can be set to either `ProcessStdio` or `rid` of open file.
   *
   * Details of the spawned process are returned.
   *
   *       const p = Deno.run({
   *         cmd: ["echo", "hello"],
   *       });
   *
   * Requires `allow-run` permission. */
  export function run(opt: RunOptions): Process;

  enum LinuxSignal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGIO = 29,
    SIGPWR = 30,
    SIGSYS = 31,
  }
  enum MacOSSignal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGEMT = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGBUS = 10,
    SIGSEGV = 11,
    SIGSYS = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGURG = 16,
    SIGSTOP = 17,
    SIGTSTP = 18,
    SIGCONT = 19,
    SIGCHLD = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGIO = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGINFO = 29,
    SIGUSR1 = 30,
    SIGUSR2 = 31,
  }

  /** **UNSTABLE**: make platform independent.
   *
   * Signals numbers. This is platform dependent. */
  export const Signal: typeof MacOSSignal | typeof LinuxSignal;

  interface InspectOptions {
    showHidden?: boolean;
    depth?: number;
    colors?: boolean;
    indentLevel?: number;
  }

  /** **UNSTABLE**: The exact form of the string output is under consideration
   * and may change.
   *
   * Converts the input into a string that has the same format as printed by
   * `console.log()`.
   *
   *      const obj = {};
   *      obj.propA = 10;
   *      obj.propB = "hello"
   *      const objAsString = Deno.inspect(obj); //{ propA: 10, propB: "hello" }
   *      console.log(obj);  //prints same value as objAsString, e.g. { propA: 10, propB: "hello" }
   *
   * You can also register custom inspect functions, via the `customInspect` Deno
   * symbol on objects, to control and customize the output.
   *
   *      class A {
   *        x = 10;
   *        y = "hello";
   *        [Deno.symbols.customInspect](): string {
   *          return "x=" + this.x + ", y=" + this.y;
   *        }
   *      }
   *
   *      const inStringFormat = Deno.inspect(new A()); //"x=10, y=hello"
   *      console.log(inStringFormat);  //prints "x=10, y=hello"
   *
   * Finally, a number of output options are also available.
   *
   *      const out = Deno.inspect(obj, {showHidden: true, depth: 4, colors: true, indentLevel: 2});
   *
   */
  export function inspect(value: unknown, options?: InspectOptions): string;

  export type OperatingSystem = "mac" | "win" | "linux";

  export type Arch = "x64" | "arm64";

  interface BuildInfo {
    /** The CPU architecture. */
    arch: Arch;
    /** The operating system. */
    os: OperatingSystem;
  }

  /** Build related information. */
  export const build: BuildInfo;

  interface Version {
    deno: string;
    v8: string;
    typescript: string;
  }
  /** Version related information. */
  export const version: Version;

  /** The log category for a diagnostic message. */
  export enum DiagnosticCategory {
    Log = 0,
    Debug = 1,
    Info = 2,
    Error = 3,
    Warning = 4,
    Suggestion = 5,
  }

  export interface DiagnosticMessageChain {
    message: string;
    category: DiagnosticCategory;
    code: number;
    next?: DiagnosticMessageChain[];
  }

  export interface DiagnosticItem {
    /** A string message summarizing the diagnostic. */
    message: string;
    /** An ordered array of further diagnostics. */
    messageChain?: DiagnosticMessageChain;
    /** Information related to the diagnostic. This is present when there is a
     * suggestion or other additional diagnostic information */
    relatedInformation?: DiagnosticItem[];
    /** The text of the source line related to the diagnostic. */
    sourceLine?: string;
    /** The line number that is related to the diagnostic. */
    lineNumber?: number;
    /** The name of the script resource related to the diagnostic. */
    scriptResourceName?: string;
    /** The start position related to the diagnostic. */
    startPosition?: number;
    /** The end position related to the diagnostic. */
    endPosition?: number;
    /** The category of the diagnostic. */
    category: DiagnosticCategory;
    /** A number identifier. */
    code: number;
    /** The the start column of the sourceLine related to the diagnostic. */
    startColumn?: number;
    /** The end column of the sourceLine related to the diagnostic. */
    endColumn?: number;
  }

  export interface Diagnostic {
    /** An array of diagnostic items. */
    items: DiagnosticItem[];
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Format an array of diagnostic items and return them as a single string in a
   * user friendly format.
   *
   *       const [diagnostics, result] = Deno.compile("file_with_compile_issues.ts");
   *       console.table(diagnostics);  //Prints raw diagnostic data
   *       console.log(Deno.formatDiagnostics(diagnostics));  //User friendly output of diagnostics
   *
   * @param items An array of diagnostic items to format
   */
  export function formatDiagnostics(items: DiagnosticItem[]): string;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * A specific subset TypeScript compiler options that can be supported by the
   * Deno TypeScript compiler. */
  export interface CompilerOptions {
    /** Allow JavaScript files to be compiled. Defaults to `true`. */
    allowJs?: boolean;
    /** Allow default imports from modules with no default export. This does not
     * affect code emit, just typechecking. Defaults to `false`. */
    allowSyntheticDefaultImports?: boolean;
    /** Allow accessing UMD globals from modules. Defaults to `false`. */
    allowUmdGlobalAccess?: boolean;
    /** Do not report errors on unreachable code. Defaults to `false`. */
    allowUnreachableCode?: boolean;
    /** Do not report errors on unused labels. Defaults to `false` */
    allowUnusedLabels?: boolean;
    /** Parse in strict mode and emit `"use strict"` for each source file.
     * Defaults to `true`. */
    alwaysStrict?: boolean;
    /** Base directory to resolve non-relative module names. Defaults to
     * `undefined`. */
    baseUrl?: string;
    /** Report errors in `.js` files. Use in conjunction with `allowJs`. Defaults
     * to `false`. */
    checkJs?: boolean;
    /** Generates corresponding `.d.ts` file. Defaults to `false`. */
    declaration?: boolean;
    /** Output directory for generated declaration files. */
    declarationDir?: string;
    /** Generates a source map for each corresponding `.d.ts` file. Defaults to
     * `false`. */
    declarationMap?: boolean;
    /** Provide full support for iterables in `for..of`, spread and
     * destructuring when targeting ES5 or ES3. Defaults to `false`. */
    downlevelIteration?: boolean;
    /** Emit a UTF-8 Byte Order Mark (BOM) in the beginning of output files.
     * Defaults to `false`. */
    emitBOM?: boolean;
    /** Only emit `.d.ts` declaration files. Defaults to `false`. */
    emitDeclarationOnly?: boolean;
    /** Emit design-type metadata for decorated declarations in source. See issue
     * [microsoft/TypeScript#2577](https://github.com/Microsoft/TypeScript/issues/2577)
     * for details. Defaults to `false`. */
    emitDecoratorMetadata?: boolean;
    /** Emit `__importStar` and `__importDefault` helpers for runtime babel
     * ecosystem compatibility and enable `allowSyntheticDefaultImports` for type
     * system compatibility. Defaults to `true`. */
    esModuleInterop?: boolean;
    /** Enables experimental support for ES decorators. Defaults to `false`. */
    experimentalDecorators?: boolean;
    /** Emit a single file with source maps instead of having a separate file.
     * Defaults to `false`. */
    inlineSourceMap?: boolean;
    /** Emit the source alongside the source maps within a single file; requires
     * `inlineSourceMap` or `sourceMap` to be set. Defaults to `false`. */
    inlineSources?: boolean;
    /** Perform additional checks to ensure that transpile only would be safe.
     * Defaults to `false`. */
    isolatedModules?: boolean;
    /** Support JSX in `.tsx` files: `"react"`, `"preserve"`, `"react-native"`.
     * Defaults to `"react"`. */
    jsx?: "react" | "preserve" | "react-native";
    /** Specify the JSX factory function to use when targeting react JSX emit,
     * e.g. `React.createElement` or `h`. Defaults to `React.createElement`. */
    jsxFactory?: string;
    /** Resolve keyof to string valued property names only (no numbers or
     * symbols). Defaults to `false`. */
    keyofStringsOnly?: string;
    /** Emit class fields with ECMAScript-standard semantics. Defaults to `false`.
     * Does not apply to `"esnext"` target. */
    useDefineForClassFields?: boolean;
    /** List of library files to be included in the compilation. If omitted,
     * then the Deno main runtime libs are used. */
    lib?: string[];
    /** The locale to use to show error messages. */
    locale?: string;
    /** Specifies the location where debugger should locate map files instead of
     * generated locations. Use this flag if the `.map` files will be located at
     * run-time in a different location than the `.js` files. The location
     * specified will be embedded in the source map to direct the debugger where
     * the map files will be located. Defaults to `undefined`. */
    mapRoot?: string;
    /** Specify the module format for the emitted code. Defaults to
     * `"esnext"`. */
    module?:
      | "none"
      | "commonjs"
      | "amd"
      | "system"
      | "umd"
      | "es6"
      | "es2015"
      | "esnext";
    /** Do not generate custom helper functions like `__extends` in compiled
     * output. Defaults to `false`. */
    noEmitHelpers?: boolean;
    /** Report errors for fallthrough cases in switch statement. Defaults to
     * `false`. */
    noFallthroughCasesInSwitch?: boolean;
    /** Raise error on expressions and declarations with an implied any type.
     * Defaults to `true`. */
    noImplicitAny?: boolean;
    /** Report an error when not all code paths in function return a value.
     * Defaults to `false`. */
    noImplicitReturns?: boolean;
    /** Raise error on `this` expressions with an implied `any` type. Defaults to
     * `true`. */
    noImplicitThis?: boolean;
    /** Do not emit `"use strict"` directives in module output. Defaults to
     * `false`. */
    noImplicitUseStrict?: boolean;
    /** Do not add triple-slash references or module import targets to the list of
     * compiled files. Defaults to `false`. */
    noResolve?: boolean;
    /** Disable strict checking of generic signatures in function types. Defaults
     * to `false`. */
    noStrictGenericChecks?: boolean;
    /** Report errors on unused locals. Defaults to `false`. */
    noUnusedLocals?: boolean;
    /** Report errors on unused parameters. Defaults to `false`. */
    noUnusedParameters?: boolean;
    /** Redirect output structure to the directory. This only impacts
     * `Deno.compile` and only changes the emitted file names. Defaults to
     * `undefined`. */
    outDir?: string;
    /** List of path mapping entries for module names to locations relative to the
     * `baseUrl`. Defaults to `undefined`. */
    paths?: Record<string, string[]>;
    /** Do not erase const enum declarations in generated code. Defaults to
     * `false`. */
    preserveConstEnums?: boolean;
    /** Remove all comments except copy-right header comments beginning with
     * `/*!`. Defaults to `true`. */
    removeComments?: boolean;
    /** Include modules imported with `.json` extension. Defaults to `true`. */
    resolveJsonModule?: boolean;
    /** Specifies the root directory of input files. Only use to control the
     * output directory structure with `outDir`. Defaults to `undefined`. */
    rootDir?: string;
    /** List of _root_ folders whose combined content represent the structure of
     * the project at runtime. Defaults to `undefined`. */
    rootDirs?: string[];
    /** Generates corresponding `.map` file. Defaults to `false`. */
    sourceMap?: boolean;
    /** Specifies the location where debugger should locate TypeScript files
     * instead of source locations. Use this flag if the sources will be located
     * at run-time in a different location than that at design-time. The location
     * specified will be embedded in the sourceMap to direct the debugger where
     * the source files will be located. Defaults to `undefined`. */
    sourceRoot?: string;
    /** Enable all strict type checking options. Enabling `strict` enables
     * `noImplicitAny`, `noImplicitThis`, `alwaysStrict`, `strictBindCallApply`,
     * `strictNullChecks`, `strictFunctionTypes` and
     * `strictPropertyInitialization`. Defaults to `true`. */
    strict?: boolean;
    /** Enable stricter checking of the `bind`, `call`, and `apply` methods on
     * functions. Defaults to `true`. */
    strictBindCallApply?: boolean;
    /** Disable bivariant parameter checking for function types. Defaults to
     * `true`. */
    strictFunctionTypes?: boolean;
    /** Ensure non-undefined class properties are initialized in the constructor.
     * This option requires `strictNullChecks` be enabled in order to take effect.
     * Defaults to `true`. */
    strictPropertyInitialization?: boolean;
    /** In strict null checking mode, the `null` and `undefined` values are not in
     * the domain of every type and are only assignable to themselves and `any`
     * (the one exception being that `undefined` is also assignable to `void`). */
    strictNullChecks?: boolean;
    /** Suppress excess property checks for object literals. Defaults to
     * `false`. */
    suppressExcessPropertyErrors?: boolean;
    /** Suppress `noImplicitAny` errors for indexing objects lacking index
     * signatures. */
    suppressImplicitAnyIndexErrors?: boolean;
    /** Specify ECMAScript target version. Defaults to `esnext`. */
    target?:
      | "es3"
      | "es5"
      | "es6"
      | "es2015"
      | "es2016"
      | "es2017"
      | "es2018"
      | "es2019"
      | "es2020"
      | "esnext";
    /** List of names of type definitions to include. Defaults to `undefined`.
     *
     * The type definitions are resolved according to the normal Deno resolution
     * irrespective of if sources are provided on the call. Like other Deno
     * modules, there is no "magical" resolution. For example:
     *
     *      Deno.compile(
     *        "./foo.js",
     *        undefined,
     *        {
     *          types: [ "./foo.d.ts", "https://deno.land/x/example/types.d.ts" ]
     *        }
     *      );
     */
    types?: string[];
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * The results of a transpile only command, where the `source` contains the
   * emitted source, and `map` optionally contains the source map. */
  export interface TranspileOnlyResult {
    source: string;
    map?: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Takes a set of TypeScript sources and resolves to a map where the key was
   * the original file name provided in sources and the result contains the
   * `source` and optionally the `map` from the transpile operation. This does no
   * type checking and validation, it effectively "strips" the types from the
   * file.
   *
   *      const results =  await Deno.transpileOnly({
   *        "foo.ts": `const foo: string = "foo";`
   *      });
   *
   * @param sources A map where the key is the filename and the value is the text
   *                to transpile. The filename is only used in the transpile and
   *                not resolved, for example to fill in the source name in the
   *                source map.
   * @param options An option object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   *                Many of the options related to type checking and emitting
   *                type declaration files will have no impact on the output.
   */
  export function transpileOnly(
    sources: Record<string, string>,
    options?: CompilerOptions
  ): Promise<Record<string, TranspileOnlyResult>>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Takes a root module name, and optionally a record set of sources. Resolves
   * with a compiled set of modules and possibly diagnostics if the compiler
   * encountered any issues. If just a root name is provided, the modules
   * will be resolved as if the root module had been passed on the command line.
   *
   * If sources are passed, all modules will be resolved out of this object, where
   * the key is the module name and the value is the content. The extension of
   * the module name will be used to determine the media type of the module.
   *
   *      const [ maybeDiagnostics1, output1 ] = await Deno.compile("foo.ts");
   *
   *      const [ maybeDiagnostics2, output2 ] = await Deno.compile("/foo.ts", {
   *        "/foo.ts": `export * from "./bar.ts";`,
   *        "/bar.ts": `export const bar = "bar";`
   *      });
   *
   * @param rootName The root name of the module which will be used as the
   *                 "starting point". If no `sources` is specified, Deno will
   *                 resolve the module externally as if the `rootName` had been
   *                 specified on the command line.
   * @param sources An optional key/value map of sources to be used when resolving
   *                modules, where the key is the module name, and the value is
   *                the source content. The extension of the key will determine
   *                the media type of the file when processing. If supplied,
   *                Deno will not attempt to resolve any modules externally.
   * @param options An optional object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   */
  export function compile(
    rootName: string,
    sources?: Record<string, string>,
    options?: CompilerOptions
  ): Promise<[DiagnosticItem[] | undefined, Record<string, string>]>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * `bundle()` is part the compiler API.  A full description of this functionality
   * can be found in the [manual](https://deno.land/std/manual.md#denobundle).
   *
   * Takes a root module name, and optionally a record set of sources. Resolves
   * with a single JavaScript string (and bundle diagnostics if issues arise with
   * the bundling) that is like the output of a `deno bundle` command. If just
   * a root name is provided, the modules will be resolved as if the root module
   * had been passed on the command line.
   *
   * If sources are passed, all modules will be resolved out of this object, where
   * the key is the module name and the value is the content. The extension of the
   * module name will be used to determine the media type of the module.
   *
   *      //equivalent to "deno bundle foo.ts" from the command line
   *      const [ maybeDiagnostics1, output1 ] = await Deno.bundle("foo.ts");
   *
   *      const [ maybeDiagnostics2, output2 ] = await Deno.bundle("/foo.ts", {
   *        "/foo.ts": `export * from "./bar.ts";`,
   *        "/bar.ts": `export const bar = "bar";`
   *      });
   *
   * @param rootName The root name of the module which will be used as the
   *                 "starting point". If no `sources` is specified, Deno will
   *                 resolve the module externally as if the `rootName` had been
   *                 specified on the command line.
   * @param sources An optional key/value map of sources to be used when resolving
   *                modules, where the key is the module name, and the value is
   *                the source content. The extension of the key will determine
   *                the media type of the file when processing. If supplied,
   *                Deno will not attempt to resolve any modules externally.
   * @param options An optional object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   */
  export function bundle(
    rootName: string,
    sources?: Record<string, string>,
    options?: CompilerOptions
  ): Promise<[DiagnosticItem[] | undefined, string]>;

  /** Returns the script arguments to the program. If for example we run a
   * program:
   *
   *      deno --allow-read https://deno.land/std/examples/cat.ts /etc/passwd
   *
   * Then `Deno.args` will contain:
   *
   *      [ "/etc/passwd" ]
   */
  export const args: string[];

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Represents the stream of signals, implements both `AsyncIterator` and
   * `PromiseLike`. */
  export class SignalStream
    implements AsyncIterableIterator<void>, PromiseLike<void> {
    constructor(signal: typeof Deno.Signal);
    then<T, S>(
      f: (v: void) => T | Promise<T>,
      g?: (v: void) => S | Promise<S>
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
   *      for await (const _ of Deno.signal(Deno.Signal.SIGTERM)) {
   *        console.log("got SIGTERM!");
   *      }
   *
   * You can also use it as a promise. In this case you can only receive the
   * first one.
   *
   *      await Deno.signal(Deno.Signal.SIGTERM);
   *      console.log("SIGTERM received!")
   *
   * If you want to stop receiving the signals, you can use `.dispose()` method
   * of the signal stream object.
   *
   *      const sig = Deno.signal(Deno.Signal.SIGTERM);
   *      setTimeout(() => { sig.dispose(); }, 5000);
   *      for await (const _ of sig) {
   *        console.log("SIGTERM!")
   *      }
   *
   * The above for-await loop exits after 5 seconds when `sig.dispose()` is
   * called.
   *
   * NOTE: This functionality is not yet implemented on Windows.
   */
  export function signal(signo: number): SignalStream;

  /** **UNSTABLE**: new API, yet to be vetted. */
  export const signals: {
    /** Returns the stream of SIGALRM signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGALRM)`. */
    alarm: () => SignalStream;
    /** Returns the stream of SIGCHLD signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGCHLD)`. */
    child: () => SignalStream;
    /** Returns the stream of SIGHUP signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGHUP)`. */
    hungup: () => SignalStream;
    /** Returns the stream of SIGINT signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGINT)`. */
    interrupt: () => SignalStream;
    /** Returns the stream of SIGIO signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGIO)`. */
    io: () => SignalStream;
    /** Returns the stream of SIGPIPE signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGPIPE)`. */
    pipe: () => SignalStream;
    /** Returns the stream of SIGQUIT signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGQUIT)`. */
    quit: () => SignalStream;
    /** Returns the stream of SIGTERM signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGTERM)`. */
    terminate: () => SignalStream;
    /** Returns the stream of SIGUSR1 signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGUSR1)`. */
    userDefined1: () => SignalStream;
    /** Returns the stream of SIGUSR2 signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGUSR2)`. */
    userDefined2: () => SignalStream;
    /** Returns the stream of SIGWINCH signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGWINCH)`. */
    windowChange: () => SignalStream;
  };

  /** **UNSTABLE**: new API. Maybe move `Deno.EOF` here.
   *
   * Special Deno related symbols. */
  export const symbols: {
    /** Symbol to access exposed internal Deno API */
    readonly internal: unique symbol;
    /** A symbol which can be used as a key for a custom method which will be
     * called when `Deno.inspect()` is called, or when the object is logged to
     * the console. */
    readonly customInspect: unique symbol;
    // TODO(ry) move EOF here?
  };
}

// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any, no-var */

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

// This follows the WebIDL at: https://webassembly.github.io/spec/js-api/
// and: https://webassembly.github.io/spec/web-api/
declare namespace WebAssembly {
  interface WebAssemblyInstantiatedSource {
    module: Module;
    instance: Instance;
  }

  /** Compiles a `WebAssembly.Module` from WebAssembly binary code.  This
   * function is useful if it is necessary to a compile a module before it can
   * be instantiated (otherwise, the `WebAssembly.instantiate()` function
   * should be used). */
  function compile(bufferSource: BufferSource): Promise<Module>;

  /** Compiles a `WebAssembly.Module` directly from a streamed underlying
   * source. This function is useful if it is necessary to a compile a module
   * before it can be instantiated (otherwise, the
   * `WebAssembly.instantiateStreaming()` function should be used). */
  function compileStreaming(source: Promise<Response>): Promise<Module>;

  /** Takes the WebAssembly binary code, in the form of a typed array or
   * `ArrayBuffer`, and performs both compilation and instantiation in one step.
   * The returned `Promise` resolves to both a compiled `WebAssembly.Module` and
   * its first `WebAssembly.Instance`. */
  function instantiate(
    bufferSource: BufferSource,
    importObject?: object
  ): Promise<WebAssemblyInstantiatedSource>;

  /** Takes an already-compiled `WebAssembly.Module` and returns a `Promise`
   * that resolves to an `Instance` of that `Module`. This overload is useful if
   * the `Module` has already been compiled. */
  function instantiate(
    module: Module,
    importObject?: object
  ): Promise<Instance>;

  /** Compiles and instantiates a WebAssembly module directly from a streamed
   * underlying source. This is the most efficient, optimized way to load wasm
   * code. */
  function instantiateStreaming(
    source: Promise<Response>,
    importObject?: object
  ): Promise<WebAssemblyInstantiatedSource>;

  /** Validates a given typed array of WebAssembly binary code, returning
   * whether the bytes form a valid wasm module (`true`) or not (`false`). */
  function validate(bufferSource: BufferSource): boolean;

  type ImportExportKind = "function" | "table" | "memory" | "global";

  interface ModuleExportDescriptor {
    name: string;
    kind: ImportExportKind;
  }
  interface ModuleImportDescriptor {
    module: string;
    name: string;
    kind: ImportExportKind;
  }

  class Module {
    constructor(bufferSource: BufferSource);

    /** Given a `Module` and string, returns a copy of the contents of all
     * custom sections in the module with the given string name. */
    static customSections(
      moduleObject: Module,
      sectionName: string
    ): ArrayBuffer;

    /** Given a `Module`, returns an array containing descriptions of all the
     * declared exports. */
    static exports(moduleObject: Module): ModuleExportDescriptor[];

    /** Given a `Module`, returns an array containing descriptions of all the
     * declared imports. */
    static imports(moduleObject: Module): ModuleImportDescriptor[];
  }

  class Instance<T extends object = { [key: string]: any }> {
    constructor(module: Module, importObject?: object);

    /** An object containing as its members all the functions exported from the
     * WebAssembly module instance, to allow them to be accessed and used by
     * JavaScript. */
    readonly exports: T;
  }

  interface MemoryDescriptor {
    initial: number;
    maximum?: number;
  }

  class Memory {
    constructor(descriptor: MemoryDescriptor);

    /** An accessor property that returns the buffer contained in the memory. */
    readonly buffer: ArrayBuffer;

    /** Increases the size of the memory instance by a specified number of
     * WebAssembly pages (each one is 64KB in size). */
    grow(delta: number): number;
  }

  type TableKind = "anyfunc";

  interface TableDescriptor {
    element: TableKind;
    initial: number;
    maximum?: number;
  }

  class Table {
    constructor(descriptor: TableDescriptor);

    /** Returns the length of the table, i.e. the number of elements. */
    readonly length: number;

    /** Accessor function — gets the element stored at a given index. */
    get(index: number): (...args: any[]) => any;

    /** Increases the size of the Table instance by a specified number of
     * elements. */
    grow(delta: number): number;

    /** Sets an element stored at a given index to a given value. */
    set(index: number, value: (...args: any[]) => any): void;
  }

  type ValueType = "i32" | "i64" | "f32" | "f64";

  interface GlobalDescriptor {
    value: ValueType;
    mutable?: boolean;
  }

  /** Represents a global variable instance, accessible from both JavaScript and
   * importable/exportable across one or more `WebAssembly.Module` instances.
   * This allows dynamic linking of multiple modules. */
  class Global {
    constructor(descriptor: GlobalDescriptor, value?: any);

    /** Old-style method that returns the value contained inside the global
     * variable. */
    valueOf(): any;

    /** The value contained inside the global variable — this can be used to
     * directly set and get the global's value. */
    value: any;
  }

  /** Indicates an error during WebAssembly decoding or validation */
  class CompileError extends Error {
    constructor(message: string, fileName?: string, lineNumber?: string);
  }

  /** Indicates an error during module instantiation (besides traps from the
   * start function). */
  class LinkError extends Error {
    constructor(message: string, fileName?: string, lineNumber?: string);
  }

  /** Is thrown whenever WebAssembly specifies a trap. */
  class RuntimeError extends Error {
    constructor(message: string, fileName?: string, lineNumber?: string);
  }
}

/** Sets a timer which executes a function once after the timer expires. */
declare function setTimeout(
  cb: (...args: unknown[]) => void,
  delay?: number,
  ...args: unknown[]
): number;

/** Repeatedly calls a function , with a fixed time delay between each call. */
declare function setInterval(
  cb: (...args: unknown[]) => void,
  delay?: number,
  ...args: unknown[]
): number;
declare function clearTimeout(id?: number): void;
declare function clearInterval(id?: number): void;
declare function queueMicrotask(func: Function): void;

declare var console: Console;
declare var location: Location;

declare function addEventListener(
  type: string,
  callback: EventListenerOrEventListenerObject | null,
  options?: boolean | AddEventListenerOptions | undefined
): void;

declare function dispatchEvent(event: Event): boolean;

declare function removeEventListener(
  type: string,
  callback: EventListenerOrEventListenerObject | null,
  options?: boolean | EventListenerOptions | undefined
): void;

declare interface ImportMeta {
  url: string;
  main: boolean;
}

interface DomIterable<K, V> {
  keys(): IterableIterator<K>;
  values(): IterableIterator<V>;
  entries(): IterableIterator<[K, V]>;
  [Symbol.iterator](): IterableIterator<[K, V]>;
  forEach(
    callback: (value: V, key: K, parent: this) => void,
    thisArg?: any
  ): void;
}

interface ReadableStreamReadDoneResult<T> {
  done: true;
  value?: T;
}

interface ReadableStreamReadValueResult<T> {
  done: false;
  value: T;
}

type ReadableStreamReadResult<T> =
  | ReadableStreamReadValueResult<T>
  | ReadableStreamReadDoneResult<T>;

interface ReadableStreamDefaultReader<R = any> {
  readonly closed: Promise<void>;
  cancel(reason?: any): Promise<void>;
  read(): Promise<ReadableStreamReadResult<R>>;
  releaseLock(): void;
}

interface UnderlyingSource<R = any> {
  cancel?: ReadableStreamErrorCallback;
  pull?: ReadableStreamDefaultControllerCallback<R>;
  start?: ReadableStreamDefaultControllerCallback<R>;
  type?: undefined;
}

interface ReadableStreamErrorCallback {
  (reason: any): void | PromiseLike<void>;
}

interface ReadableStreamDefaultControllerCallback<R> {
  (controller: ReadableStreamDefaultController<R>): void | PromiseLike<void>;
}

interface ReadableStreamDefaultController<R> {
  readonly desiredSize: number;
  enqueue(chunk?: R): void;
  close(): void;
  error(e?: any): void;
}

/** This Streams API interface represents a readable stream of byte data. The
 * Fetch API offers a concrete instance of a ReadableStream through the body
 * property of a Response object. */
interface ReadableStream<R = any> {
  readonly locked: boolean;
  cancel(reason?: any): Promise<void>;
  // TODO(ry) It doesn't seem like Chrome supports this.
  // getReader(options: { mode: "byob" }): ReadableStreamBYOBReader;
  getReader(): ReadableStreamDefaultReader<R>;
  tee(): [ReadableStream<R>, ReadableStream<R>];
}

declare const ReadableStream: {
  prototype: ReadableStream;
  // TODO(ry) This doesn't match lib.dom.d.ts
  new <R = any>(src?: UnderlyingSource<R>): ReadableStream<R>;
};

/** This Streams API interface provides a standard abstraction for writing streaming data to a destination, known as a sink. This object comes with built-in backpressure and queuing. */
interface WritableStream<W = any> {
  readonly locked: boolean;
  abort(reason?: any): Promise<void>;
  getWriter(): WritableStreamDefaultWriter<W>;
}

interface WritableStreamDefaultWriter<W = any> {
  readonly closed: Promise<void>;
  readonly desiredSize: number | null;
  readonly ready: Promise<void>;
  abort(reason?: any): Promise<void>;
  close(): Promise<void>;
  releaseLock(): void;
  write(chunk: W): Promise<void>;
}

interface DOMStringList {
  /** Returns the number of strings in strings. */
  readonly length: number;
  /** Returns true if strings contains string, and false otherwise. */
  contains(string: string): boolean;
  /** Returns the string with index index from strings. */
  item(index: number): string | null;
  [index: number]: string;
}

declare class DOMException extends Error {
  constructor(message?: string, name?: string);
  readonly name: string;
  readonly message: string;
}

/** The location (URL) of the object it is linked to. Changes done on it are
 * reflected on the object it relates to. Both the Document and Window
 * interface have such a linked Location, accessible via Document.location and
 * Window.location respectively. */
declare interface Location {
  /** Returns a DOMStringList object listing the origins of the ancestor
   * browsing contexts, from the parent browsing context to the top-level
   * browsing context. */
  readonly ancestorOrigins: DOMStringList;
  /** Returns the Location object's URL's fragment (includes leading "#" if
   * non-empty).
   *
   * Can be set, to navigate to the same URL with a changed fragment (ignores
   * leading "#"). */
  hash: string;
  /** Returns the Location object's URL's host and port (if different from the
   * default port for the scheme).
   *
   * Can be set, to navigate to the same URL with a changed host and port. */
  host: string;
  /** Returns the Location object's URL's host.
   *
   * Can be set, to navigate to the same URL with a changed host. */
  hostname: string;
  /** Returns the Location object's URL.
   *
   * Can be set, to navigate to the given URL. */
  href: string;
  toString(): string;
  /** Returns the Location object's URL's origin. */
  readonly origin: string;
  /** Returns the Location object's URL's path.
   *
   * Can be set, to navigate to the same URL with a changed path. */
  pathname: string;
  /** Returns the Location object's URL's port.
   *
   * Can be set, to navigate to the same URL with a changed port. */
  port: string;
  /** Returns the Location object's URL's scheme.
   *
   * Can be set, to navigate to the same URL with a changed scheme. */
  protocol: string;
  /** Returns the Location object's URL's query (includes leading "?" if
   * non-empty).
   *
   * Can be set, to navigate to the same URL with a changed query (ignores
   * leading "?"). */
  search: string;
  /**
   * Navigates to the given URL.
   */
  assign(url: string): void;
  /**
   * Reloads the current page.
   */
  reload(): void;
  /** Removes the current page from the session history and navigates to the
   * given URL. */
  replace(url: string): void;
}

type BufferSource = ArrayBufferView | ArrayBuffer;
type BlobPart = BufferSource | Blob | string;

interface BlobPropertyBag {
  type?: string;
  ending?: "transparent" | "native";
}

/** A file-like object of immutable, raw data. Blobs represent data that isn't necessarily in a JavaScript-native format. The File interface is based on Blob, inheriting blob functionality and expanding it to support files on the user's system. */
interface Blob {
  readonly size: number;
  readonly type: string;
  arrayBuffer(): Promise<ArrayBuffer>;
  slice(start?: number, end?: number, contentType?: string): Blob;
  stream(): ReadableStream;
  text(): Promise<string>;
}

declare const Blob: {
  prototype: Blob;
  new (blobParts?: BlobPart[], options?: BlobPropertyBag): Blob;
};

interface FilePropertyBag extends BlobPropertyBag {
  lastModified?: number;
}

/** Provides information about files and allows JavaScript in a web page to
 * access their content. */
interface File extends Blob {
  readonly lastModified: number;
  readonly name: string;
}

declare const File: {
  prototype: File;
  new (fileBits: BlobPart[], fileName: string, options?: FilePropertyBag): File;
};

declare const isConsoleInstance: unique symbol;

declare class Console {
  indentLevel: number;
  [isConsoleInstance]: boolean;
  /** Writes the arguments to stdout */
  log: (...args: unknown[]) => void;
  /** Writes the arguments to stdout */
  debug: (...args: unknown[]) => void;
  /** Writes the arguments to stdout */
  info: (...args: unknown[]) => void;
  /** Writes the properties of the supplied `obj` to stdout */
  dir: (
    obj: unknown,
    options?: Partial<{
      showHidden: boolean;
      depth: number;
      colors: boolean;
      indentLevel: number;
    }>
  ) => void;

  /** From MDN:
   * Displays an interactive tree of the descendant elements of
   * the specified XML/HTML element. If it is not possible to display
   * as an element the JavaScript Object view is shown instead.
   * The output is presented as a hierarchical listing of expandable
   * nodes that let you see the contents of child nodes.
   *
   * Since we write to stdout, we can't display anything interactive
   * we just fall back to `console.dir`.
   */
  dirxml: (
    obj: unknown,
    options?: Partial<{
      showHidden: boolean;
      depth: number;
      colors: boolean;
      indentLevel: number;
    }>
  ) => void;

  /** Writes the arguments to stdout */
  warn: (...args: unknown[]) => void;
  /** Writes the arguments to stdout */
  error: (...args: unknown[]) => void;
  /** Writes an error message to stdout if the assertion is `false`. If the
   * assertion is `true`, nothing happens.
   *
   * ref: https://console.spec.whatwg.org/#assert
   */
  assert: (condition?: boolean, ...args: unknown[]) => void;
  count: (label?: string) => void;
  countReset: (label?: string) => void;
  table: (data: unknown, properties?: string[] | undefined) => void;
  time: (label?: string) => void;
  timeLog: (label?: string, ...args: unknown[]) => void;
  timeEnd: (label?: string) => void;
  group: (...label: unknown[]) => void;
  groupCollapsed: (...label: unknown[]) => void;
  groupEnd: () => void;
  clear: () => void;
  trace: (...args: unknown[]) => void;
  static [Symbol.hasInstance](instance: Console): boolean;
}

type FormDataEntryValue = File | string;

/** Provides a way to easily construct a set of key/value pairs representing
 * form fields and their values, which can then be easily sent using the
 * XMLHttpRequest.send() method. It uses the same format a form would use if the
 * encoding type were set to "multipart/form-data". */
interface FormData extends DomIterable<string, FormDataEntryValue> {
  append(name: string, value: string | Blob, fileName?: string): void;
  delete(name: string): void;
  get(name: string): FormDataEntryValue | null;
  getAll(name: string): FormDataEntryValue[];
  has(name: string): boolean;
  set(name: string, value: string | Blob, fileName?: string): void;
}

declare const FormData: {
  prototype: FormData;
  // TODO(ry) FormData constructor is non-standard.
  // new(form?: HTMLFormElement): FormData;
  new (): FormData;
};

interface Body {
  /** A simple getter used to expose a `ReadableStream` of the body contents. */
  readonly body: ReadableStream<Uint8Array> | null;
  /** Stores a `Boolean` that declares whether the body has been used in a
   * response yet.
   */
  readonly bodyUsed: boolean;
  /** Takes a `Response` stream and reads it to completion. It returns a promise
   * that resolves with an `ArrayBuffer`.
   */
  arrayBuffer(): Promise<ArrayBuffer>;
  /** Takes a `Response` stream and reads it to completion. It returns a promise
   * that resolves with a `Blob`.
   */
  blob(): Promise<Blob>;
  /** Takes a `Response` stream and reads it to completion. It returns a promise
   * that resolves with a `FormData` object.
   */
  formData(): Promise<FormData>;
  /** Takes a `Response` stream and reads it to completion. It returns a promise
   * that resolves with the result of parsing the body text as JSON.
   */
  json(): Promise<any>;
  /** Takes a `Response` stream and reads it to completion. It returns a promise
   * that resolves with a `USVString` (text).
   */
  text(): Promise<string>;
}

type HeadersInit = Headers | string[][] | Record<string, string>;

/** This Fetch API interface allows you to perform various actions on HTTP
 * request and response headers. These actions include retrieving, setting,
 * adding to, and removing. A Headers object has an associated header list,
 * which is initially empty and consists of zero or more name and value pairs.
 *  You can add to this using methods like append() (see Examples.) In all
 * methods of this interface, header names are matched by case-insensitive byte
 * sequence. */
interface Headers {
  append(name: string, value: string): void;
  delete(name: string): void;
  get(name: string): string | null;
  has(name: string): boolean;
  set(name: string, value: string): void;
  forEach(
    callbackfn: (value: string, key: string, parent: Headers) => void,
    thisArg?: any
  ): void;
}

interface Headers extends DomIterable<string, string> {
  /** Appends a new value onto an existing header inside a `Headers` object, or
   * adds the header if it does not already exist.
   */
  append(name: string, value: string): void;
  /** Deletes a header from a `Headers` object. */
  delete(name: string): void;
  /** Returns an iterator allowing to go through all key/value pairs
   * contained in this Headers object. The both the key and value of each pairs
   * are ByteString objects.
   */
  entries(): IterableIterator<[string, string]>;
  /** Returns a `ByteString` sequence of all the values of a header within a
   * `Headers` object with a given name.
   */
  get(name: string): string | null;
  /** Returns a boolean stating whether a `Headers` object contains a certain
   * header.
   */
  has(name: string): boolean;
  /** Returns an iterator allowing to go through all keys contained in
   * this Headers object. The keys are ByteString objects.
   */
  keys(): IterableIterator<string>;
  /** Sets a new value for an existing header inside a Headers object, or adds
   * the header if it does not already exist.
   */
  set(name: string, value: string): void;
  /** Returns an iterator allowing to go through all values contained in
   * this Headers object. The values are ByteString objects.
   */
  values(): IterableIterator<string>;
  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    thisArg?: any
  ): void;
  /** The Symbol.iterator well-known symbol specifies the default
   * iterator for this Headers object
   */
  [Symbol.iterator](): IterableIterator<[string, string]>;
}

declare const Headers: {
  prototype: Headers;
  new (init?: HeadersInit): Headers;
};

type RequestInfo = Request | string;
type RequestCache =
  | "default"
  | "force-cache"
  | "no-cache"
  | "no-store"
  | "only-if-cached"
  | "reload";
type RequestCredentials = "include" | "omit" | "same-origin";
type RequestMode = "cors" | "navigate" | "no-cors" | "same-origin";
type RequestRedirect = "error" | "follow" | "manual";
type ReferrerPolicy =
  | ""
  | "no-referrer"
  | "no-referrer-when-downgrade"
  | "origin"
  | "origin-when-cross-origin"
  | "same-origin"
  | "strict-origin"
  | "strict-origin-when-cross-origin"
  | "unsafe-url";
type BodyInit =
  | Blob
  | BufferSource
  | FormData
  | URLSearchParams
  | ReadableStream<Uint8Array>
  | string;
type RequestDestination =
  | ""
  | "audio"
  | "audioworklet"
  | "document"
  | "embed"
  | "font"
  | "image"
  | "manifest"
  | "object"
  | "paintworklet"
  | "report"
  | "script"
  | "sharedworker"
  | "style"
  | "track"
  | "video"
  | "worker"
  | "xslt";

interface RequestInit {
  /**
   * A BodyInit object or null to set request's body.
   */
  body?: BodyInit | null;
  /**
   * A string indicating how the request will interact with the browser's cache
   * to set request's cache.
   */
  cache?: RequestCache;
  /**
   * A string indicating whether credentials will be sent with the request
   * always, never, or only when sent to a same-origin URL. Sets request's
   * credentials.
   */
  credentials?: RequestCredentials;
  /**
   * A Headers object, an object literal, or an array of two-item arrays to set
   * request's headers.
   */
  headers?: HeadersInit;
  /**
   * A cryptographic hash of the resource to be fetched by request. Sets
   * request's integrity.
   */
  integrity?: string;
  /**
   * A boolean to set request's keepalive.
   */
  keepalive?: boolean;
  /**
   * A string to set request's method.
   */
  method?: string;
  /**
   * A string to indicate whether the request will use CORS, or will be
   * restricted to same-origin URLs. Sets request's mode.
   */
  mode?: RequestMode;
  /**
   * A string indicating whether request follows redirects, results in an error
   * upon encountering a redirect, or returns the redirect (in an opaque
   * fashion). Sets request's redirect.
   */
  redirect?: RequestRedirect;
  /**
   * A string whose value is a same-origin URL, "about:client", or the empty
   * string, to set request's referrer.
   */
  referrer?: string;
  /**
   * A referrer policy to set request's referrerPolicy.
   */
  referrerPolicy?: ReferrerPolicy;
  /**
   * An AbortSignal to set request's signal.
   */
  signal?: AbortSignal | null;
  /**
   * Can only be null. Used to disassociate request from any Window.
   */
  window?: any;
}

/** This Fetch API interface represents a resource request. */
interface Request extends Body {
  /**
   * Returns the cache mode associated with request, which is a string
   * indicating how the request will interact with the browser's cache when
   * fetching.
   */
  readonly cache: RequestCache;
  /**
   * Returns the credentials mode associated with request, which is a string
   * indicating whether credentials will be sent with the request always, never,
   * or only when sent to a same-origin URL.
   */
  readonly credentials: RequestCredentials;
  /**
   * Returns the kind of resource requested by request, e.g., "document" or "script".
   */
  readonly destination: RequestDestination;
  /**
   * Returns a Headers object consisting of the headers associated with request.
   * Note that headers added in the network layer by the user agent will not be
   * accounted for in this object, e.g., the "Host" header.
   */
  readonly headers: Headers;
  /**
   * Returns request's subresource integrity metadata, which is a cryptographic
   * hash of the resource being fetched. Its value consists of multiple hashes
   * separated by whitespace. [SRI]
   */
  readonly integrity: string;
  /**
   * Returns a boolean indicating whether or not request is for a history
   * navigation (a.k.a. back-foward navigation).
   */
  readonly isHistoryNavigation: boolean;
  /**
   * Returns a boolean indicating whether or not request is for a reload
   * navigation.
   */
  readonly isReloadNavigation: boolean;
  /**
   * Returns a boolean indicating whether or not request can outlive the global
   * in which it was created.
   */
  readonly keepalive: boolean;
  /**
   * Returns request's HTTP method, which is "GET" by default.
   */
  readonly method: string;
  /**
   * Returns the mode associated with request, which is a string indicating
   * whether the request will use CORS, or will be restricted to same-origin
   * URLs.
   */
  readonly mode: RequestMode;
  /**
   * Returns the redirect mode associated with request, which is a string
   * indicating how redirects for the request will be handled during fetching. A
   * request will follow redirects by default.
   */
  readonly redirect: RequestRedirect;
  /**
   * Returns the referrer of request. Its value can be a same-origin URL if
   * explicitly set in init, the empty string to indicate no referrer, and
   * "about:client" when defaulting to the global's default. This is used during
   * fetching to determine the value of the `Referer` header of the request
   * being made.
   */
  readonly referrer: string;
  /**
   * Returns the referrer policy associated with request. This is used during
   * fetching to compute the value of the request's referrer.
   */
  readonly referrerPolicy: ReferrerPolicy;
  /**
   * Returns the signal associated with request, which is an AbortSignal object
   * indicating whether or not request has been aborted, and its abort event
   * handler.
   */
  readonly signal: AbortSignal;
  /**
   * Returns the URL of request as a string.
   */
  readonly url: string;
  clone(): Request;
}

declare const Request: {
  prototype: Request;
  new (input: RequestInfo, init?: RequestInit): Request;
};

type ResponseType =
  | "basic"
  | "cors"
  | "default"
  | "error"
  | "opaque"
  | "opaqueredirect";

/** This Fetch API interface represents the response to a request. */
interface Response extends Body {
  readonly headers: Headers;
  readonly ok: boolean;
  readonly redirected: boolean;
  readonly status: number;
  readonly statusText: string;
  readonly trailer: Promise<Headers>;
  readonly type: ResponseType;
  readonly url: string;
  clone(): Response;
}

declare const Response: {
  prototype: Response;

  // TODO(#4667) Response constructor is non-standard.
  // new(body?: BodyInit | null, init?: ResponseInit): Response;
  new (
    url: string,
    status: number,
    statusText: string,
    headersList: Array<[string, string]>,
    rid: number,
    redirected_: boolean,
    type_?: null | ResponseType,
    body_?: null | Body
  ): Response;

  error(): Response;
  redirect(url: string, status?: number): Response;
};

/** Fetch a resource from the network. */
declare function fetch(
  input: Request | URL | string,
  init?: RequestInit
): Promise<Response>;

declare function atob(s: string): string;

/** Creates a base-64 ASCII string from the input string. */
declare function btoa(s: string): string;

declare class TextDecoder {
  /** Returns encoding's name, lowercased. */
  readonly encoding: string;
  /** Returns `true` if error mode is "fatal", and `false` otherwise. */
  readonly fatal: boolean;
  /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
  readonly ignoreBOM = false;
  constructor(
    label?: string,
    options?: { fatal?: boolean; ignoreBOM?: boolean }
  );
  /** Returns the result of running encoding's decoder. */
  decode(input?: BufferSource, options?: { stream?: false }): string;
  readonly [Symbol.toStringTag]: string;
}

declare class TextEncoder {
  /** Returns "utf-8". */
  readonly encoding = "utf-8";
  /** Returns the result of running UTF-8's encoder. */
  encode(input?: string): Uint8Array;
  encodeInto(
    input: string,
    dest: Uint8Array
  ): { read: number; written: number };
  readonly [Symbol.toStringTag]: string;
}

interface URLSearchParams {
  /** Appends a specified key/value pair as a new search parameter.
   *
   *       let searchParams = new URLSearchParams();
   *       searchParams.append('name', 'first');
   *       searchParams.append('name', 'second');
   */
  append(name: string, value: string): void;

  /** Deletes the given search parameter and its associated value,
   * from the list of all search parameters.
   *
   *       let searchParams = new URLSearchParams([['name', 'value']]);
   *       searchParams.delete('name');
   */
  delete(name: string): void;

  /** Returns all the values associated with a given search parameter
   * as an array.
   *
   *       searchParams.getAll('name');
   */
  getAll(name: string): string[];

  /** Returns the first value associated to the given search parameter.
   *
   *       searchParams.get('name');
   */
  get(name: string): string | null;

  /** Returns a Boolean that indicates whether a parameter with the
   * specified name exists.
   *
   *       searchParams.has('name');
   */
  has(name: string): boolean;

  /** Sets the value associated with a given search parameter to the
   * given value. If there were several matching values, this method
   * deletes the others. If the search parameter doesn't exist, this
   * method creates it.
   *
   *       searchParams.set('name', 'value');
   */
  set(name: string, value: string): void;

  /** Sort all key/value pairs contained in this object in place and
   * return undefined. The sort order is according to Unicode code
   * points of the keys.
   *
   *       searchParams.sort();
   */
  sort(): void;

  /** Calls a function for each element contained in this object in
   * place and return undefined. Optionally accepts an object to use
   * as this when executing callback as second argument.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       params.forEach((value, key, parent) => {
   *         console.log(value, key, parent);
   *       });
   *
   */
  forEach(
    callbackfn: (value: string, key: string, parent: this) => void,
    thisArg?: any
  ): void;

  /** Returns an iterator allowing to go through all keys contained
   * in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const key of params.keys()) {
   *         console.log(key);
   *       }
   */
  keys(): IterableIterator<string>;

  /** Returns an iterator allowing to go through all values contained
   * in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const value of params.values()) {
   *         console.log(value);
   *       }
   */
  values(): IterableIterator<string>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const [key, value] of params.entries()) {
   *         console.log(key, value);
   *       }
   */
  entries(): IterableIterator<[string, string]>;

  /** Returns an iterator allowing to go through all key/value
   * pairs contained in this object.
   *
   *       const params = new URLSearchParams([["a", "b"], ["c", "d"]]);
   *       for (const [key, value] of params) {
   *         console.log(key, value);
   *       }
   */
  [Symbol.iterator](): IterableIterator<[string, string]>;

  /** Returns a query string suitable for use in a URL.
   *
   *        searchParams.toString();
   */
  toString(): string;
}

declare const URLSearchParams: {
  prototype: URLSearchParams;
  new (
    init?: string[][] | Record<string, string> | string | URLSearchParams
  ): URLSearchParams;
  toString(): string;
};

/** The URL interface represents an object providing static methods used for creating object URLs. */
interface URL {
  hash: string;
  host: string;
  hostname: string;
  href: string;
  toString(): string;
  readonly origin: string;
  password: string;
  pathname: string;
  port: string;
  protocol: string;
  search: string;
  readonly searchParams: URLSearchParams;
  username: string;
  toJSON(): string;
}

declare const URL: {
  prototype: URL;
  new (url: string, base?: string | URL): URL;
  createObjectURL(object: any): string;
  revokeObjectURL(url: string): void;
};

interface MessageEventInit extends EventInit {
  data?: any;
  origin?: string;
  lastEventId?: string;
}

declare class MessageEvent extends Event {
  readonly data: any;
  readonly origin: string;
  readonly lastEventId: string;
  constructor(type: string, eventInitDict?: MessageEventInit);
}

interface ErrorEventInit extends EventInit {
  message?: string;
  filename?: string;
  lineno?: number;
  colno?: number;
  error?: any;
}

declare class ErrorEvent extends Event {
  readonly message: string;
  readonly filename: string;
  readonly lineno: number;
  readonly colno: number;
  readonly error: any;
  constructor(type: string, eventInitDict?: ErrorEventInit);
}

interface PostMessageOptions {
  transfer?: any[];
}

declare class Worker extends EventTarget {
  onerror?: (e: ErrorEvent) => void;
  onmessage?: (e: MessageEvent) => void;
  onmessageerror?: (e: MessageEvent) => void;
  constructor(
    specifier: string,
    options?: {
      type?: "classic" | "module";
      name?: string;
    }
  );
  postMessage(message: any, transfer: ArrayBuffer[]): void;
  postMessage(message: any, options?: PostMessageOptions): void;
  terminate(): void;
}

declare namespace performance {
  /** Returns a current time from Deno's start in milliseconds.
   *
   * Use the flag --allow-hrtime return a precise value.
   *
   *       const t = performance.now();
   *       console.log(`${t} ms since start!`);
   */
  export function now(): number;
}

interface EventInit {
  bubbles?: boolean;
  cancelable?: boolean;
  composed?: boolean;
}

/** An event which takes place in the DOM. */
declare class Event {
  constructor(type: string, eventInitDict?: EventInit);
  /** Returns true or false depending on how event was initialized. True if
   * event goes through its target's ancestors in reverse tree order, and
   * false otherwise. */
  readonly bubbles: boolean;
  cancelBubble: boolean;
  /** Returns true or false depending on how event was initialized. Its return
   * value does not always carry meaning, but true can indicate that part of the
   * operation during which event was dispatched, can be canceled by invoking
   * the preventDefault() method. */
  readonly cancelable: boolean;
  /** Returns true or false depending on how event was initialized. True if
   * event invokes listeners past a ShadowRoot node that is the root of its
   * target, and false otherwise. */
  readonly composed: boolean;
  /** Returns the object whose event listener's callback is currently being
   * invoked. */
  readonly currentTarget: EventTarget | null;
  /** Returns true if preventDefault() was invoked successfully to indicate
   * cancellation, and false otherwise. */
  readonly defaultPrevented: boolean;
  /** Returns the event's phase, which is one of NONE, CAPTURING_PHASE,
   * AT_TARGET, and BUBBLING_PHASE. */
  readonly eventPhase: number;
  /** Returns true if event was dispatched by the user agent, and false
   * otherwise. */
  readonly isTrusted: boolean;
  /** Returns the object to which event is dispatched (its target). */
  readonly target: EventTarget | null;
  /** Returns the event's timestamp as the number of milliseconds measured
   * relative to the time origin. */
  readonly timeStamp: number;
  /** Returns the type of event, e.g. "click", "hashchange", or "submit". */
  readonly type: string;
  /** Returns the invocation target objects of event's path (objects on which
   * listeners will be invoked), except for any nodes in shadow trees of which
   * the shadow root's mode is "closed" that are not reachable from event's
   * currentTarget. */
  composedPath(): EventTarget[];
  /** If invoked when the cancelable attribute value is true, and while
   * executing a listener for the event with passive set to false, signals to
   * the operation that caused event to be dispatched that it needs to be
   * canceled. */
  preventDefault(): void;
  /** Invoking this method prevents event from reaching any registered event
   * listeners after the current one finishes running and, when dispatched in a
   * tree, also prevents event from reaching any other objects. */
  stopImmediatePropagation(): void;
  /** When dispatched in a tree, invoking this method prevents event from
   * reaching any objects other than the current object. */
  stopPropagation(): void;
  readonly AT_TARGET: number;
  readonly BUBBLING_PHASE: number;
  readonly CAPTURING_PHASE: number;
  readonly NONE: number;
  static readonly AT_TARGET: number;
  static readonly BUBBLING_PHASE: number;
  static readonly CAPTURING_PHASE: number;
  static readonly NONE: number;
}

/**
 * EventTarget is a DOM interface implemented by objects that can receive events
 * and may have listeners for them.
 */
declare class EventTarget {
  /** Appends an event listener for events whose type attribute value is type.
   * The callback argument sets the callback that will be invoked when the event
   * is dispatched.
   *
   * The options argument sets listener-specific options. For compatibility this
   * can be a boolean, in which case the method behaves exactly as if the value
   * was specified as options's capture.
   *
   * When set to true, options's capture prevents callback from being invoked
   * when the event's eventPhase attribute value is BUBBLING_PHASE. When false
   * (or not present), callback will not be invoked when event's eventPhase
   * attribute value is CAPTURING_PHASE. Either way, callback will be invoked if
   * event's eventPhase attribute value is AT_TARGET.
   *
   * When set to true, options's passive indicates that the callback will not
   * cancel the event by invoking preventDefault(). This is used to enable
   * performance optimizations described in § 2.8 Observing event listeners.
   *
   * When set to true, options's once indicates that the callback will only be
   * invoked once after which the event listener will be removed.
   *
   * The event listener is appended to target's event listener list and is not
   * appended if it has the same type, callback, and capture. */
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject | null,
    options?: boolean | AddEventListenerOptions
  ): void;
  /** Dispatches a synthetic event event to target and returns true if either
   * event's cancelable attribute value is false or its preventDefault() method
   * was not invoked, and false otherwise. */
  dispatchEvent(event: Event): boolean;
  /** Removes the event listener in target's event listener list with the same
   * type, callback, and options. */
  removeEventListener(
    type: string,
    callback: EventListenerOrEventListenerObject | null,
    options?: EventListenerOptions | boolean
  ): void;
  [Symbol.toStringTag]: string;
}

interface EventListener {
  (evt: Event): void | Promise<void>;
}

interface EventListenerObject {
  handleEvent(evt: Event): void | Promise<void>;
}

declare type EventListenerOrEventListenerObject =
  | EventListener
  | EventListenerObject;

interface AddEventListenerOptions extends EventListenerOptions {
  once?: boolean;
  passive?: boolean;
}

interface EventListenerOptions {
  capture?: boolean;
}

/** Events measuring progress of an underlying process, like an HTTP request
 * (for an XMLHttpRequest, or the loading of the underlying resource of an
 * <img>, <audio>, <video>, <style> or <link>). */
interface ProgressEvent<T extends EventTarget = EventTarget> extends Event {
  readonly lengthComputable: boolean;
  readonly loaded: number;
  readonly target: T | null;
  readonly total: number;
}

interface CustomEventInit<T = any> extends EventInit {
  detail?: T;
}

declare class CustomEvent<T = any> extends Event {
  constructor(typeArg: string, eventInitDict?: CustomEventInit<T>);
  /** Returns any custom data event was created with. Typically used for
   * synthetic events. */
  readonly detail: T;
}

/** A controller object that allows you to abort one or more DOM requests as and
 * when desired. */
declare class AbortController {
  /** Returns the AbortSignal object associated with this object. */
  readonly signal: AbortSignal;
  /** Invoking this method will set this object's AbortSignal's aborted flag and
   * signal to any observers that the associated activity is to be aborted. */
  abort(): void;
}

interface AbortSignalEventMap {
  abort: Event;
}

/** A signal object that allows you to communicate with a DOM request (such as a
 * Fetch) and abort it if required via an AbortController object. */
interface AbortSignal extends EventTarget {
  /** Returns true if this AbortSignal's AbortController has signaled to abort,
   * and false otherwise. */
  readonly aborted: boolean;
  onabort: ((this: AbortSignal, ev: Event) => any) | null;
  addEventListener<K extends keyof AbortSignalEventMap>(
    type: K,
    listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
    options?: boolean | AddEventListenerOptions
  ): void;
  addEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions
  ): void;
  removeEventListener<K extends keyof AbortSignalEventMap>(
    type: K,
    listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
    options?: boolean | EventListenerOptions
  ): void;
  removeEventListener(
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions
  ): void;
}

declare const AbortSignal: {
  prototype: AbortSignal;
  new (): AbortSignal;
};

// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-explicit-any */

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.shared_globals" />
/// <reference lib="esnext" />

declare interface Window extends EventTarget {
  readonly window: Window & typeof globalThis;
  readonly self: Window & typeof globalThis;
  onload: ((this: Window, ev: Event) => any) | null;
  onunload: ((this: Window, ev: Event) => any) | null;
  location: Location;
  crypto: Crypto;
  close: () => void;
  readonly closed: boolean;
  Deno: typeof Deno;
}

declare const window: Window & typeof globalThis;
declare const self: Window & typeof globalThis;
declare const onload: ((this: Window, ev: Event) => any) | null;
declare const onunload: ((this: Window, ev: Event) => any) | null;
declare const crypto: Crypto;

declare interface Crypto {
  readonly subtle: null;
  getRandomValues<
    T extends
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | null
  >(
    array: T
  ): T;
}

/* eslint-enable @typescript-eslint/no-explicit-any */