// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  /** The current process id of the runtime. */
  export let pid: number;

  /** Reflects the NO_COLOR environment variable.
   *
   * See: https://no-color.org/ */
  export let noColor: boolean;

  export type TestFunction = () => void | Promise<void>;

  export interface TestDefinition {
    fn: TestFunction;
    name: string;
    ignore?: boolean;
    disableOpSanitizer?: boolean;
    disableResourceSanitizer?: boolean;
  }

  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module, or explicitly
   * when `Deno.runTests` is used */
  export function test(t: TestDefinition): void;
  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module, or explicitly
   * when `Deno.runTests` is used */
  export function test(fn: TestFunction): void;
  /** Register a test which will be run when `deno test` is used on the command
   * line and the containing module looks like a test module, or explicitly
   * when `Deno.runTests` is used */
  export function test(name: string, fn: TestFunction): void;

  enum TestStatus {
    Passed = "passed",
    Failed = "failed",
    Ignored = "ignored",
  }

  interface TestResult {
    name: string;
    status: TestStatus;
    duration?: number;
    error?: Error;
  }

  interface TestStats {
    filtered: number;
    ignored: number;
    measured: number;
    passed: number;
    failed: number;
  }

  export enum TestEvent {
    Start = "start",
    TestStart = "testStart",
    TestEnd = "testEnd",
    End = "end",
  }

  interface TestEventStart {
    kind: TestEvent.Start;
    tests: number;
  }

  interface TestEventTestStart {
    kind: TestEvent.TestStart;
    name: string;
  }

  interface TestEventTestEnd {
    kind: TestEvent.TestEnd;
    result: TestResult;
  }

  interface TestEventEnd {
    kind: TestEvent.End;
    stats: TestStats;
    duration: number;
    results: TestResult[];
  }

  interface TestReporter {
    start(event: TestEventStart): Promise<void>;
    testStart(msg: TestEventTestStart): Promise<void>;
    testEnd(msg: TestEventTestEnd): Promise<void>;
    end(event: TestEventEnd): Promise<void>;
  }

  export class ConsoleTestReporter implements TestReporter {
    constructor();
    start(event: TestEventStart): Promise<void>;
    testStart(msg: TestEventTestStart): Promise<void>;
    testEnd(msg: TestEventTestEnd): Promise<void>;
    end(event: TestEventEnd): Promise<void>;
  }

  export interface RunTestsOptions {
    /** If `true`, Deno will exit with status code 1 if there was
     * test failure. Defaults to `true`. */
    exitOnFail?: boolean;
    /** If `true`, Deno will exit upon first test failure Defaults to `false`. */
    failFast?: boolean;
    /** String or RegExp used to filter test to run. Only test with names
     * matching provided `String` or `RegExp` will be run. */
    only?: string | RegExp;
    /** String or RegExp used to skip tests to run. Tests with names
     * matching provided `String` or `RegExp` will not be run. */
    skip?: string | RegExp;
    /** Disable logging of the results. Defaults to `false`. */
    disableLog?: boolean;
    /** Custom reporter class. If not provided uses console reporter. */
    reporter?: TestReporter;
  }

  /** Run any tests which have been registered. Always resolves
   * asynchronously. */
  export function runTests(
    opts?: RunTestsOptions
  ): Promise<{
    results: TestResult[];
    stats: TestStats;
    duration: number;
  }>;

  /** Get the `loadavg`. Requires `allow-env` permission.
   *
   *       console.log(Deno.loadavg());
   */
  export function loadavg(): number[];

  /** Get the `hostname` of the machine the Deno process is running on.
   *
   *       console.log(Deno.hostname());
   *
   *  Requires `allow-env` permission.
   */
  export function hostname(): string;

  /** Get the OS release. Requires `allow-env` permission.
   *
   *       console.log(Deno.osRelease());
   */
  export function osRelease(): string;

  /** Exit the Deno process with optional exit code. If no exit code is supplied
   * then Deno will exit with return code of 0.
   *
   *       Deno.exit(5);
   */
  export function exit(code?: number): never;

  /** Without any parameters, this will return a snapshot of the environment
   * variables at invocation. Changing a property in the object will set that
   * variable in the environment for the process. The environment object will
   * only accept `string`s as values.
   *
   * Passing in a `string` key parameter will return the value for that environment
   * variable, or undefined if that key doesn't exist.
   *
   *       const myEnv = Deno.env();
   *       console.log(myEnv.SHELL);
   *       myEnv.TEST_VAR = "HELLO";
   *       const newEnv = Deno.env();
   *       console.log(myEnv.TEST_VAR === newEnv.TEST_VAR);  //outputs "true"
   *       console.log(Deno.env("TEST_VAR"));  //outputs "HELLO"
   *       console.log(Deno.env("MADE_UP_VAR"));  //outputs "Undefined"
   *
   * Requires `allow-env` permission. */
  export function env(): {
    [index: string]: string;
  };

  /** See overloaded parent function Deno.env() */
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
   * **UNSTABLE**: New API. Maybe needs permissions.
   *
   * If `mask` is provided, sets the process umask. Always returns what the umask
   * was before the call.
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

  /** Turns `r` into async iterator.
   *
   *      for await (const chunk of toAsyncIterator(reader)) {
   *        console.log(chunk);
   *      }
   */
  export function toAsyncIterator(r: Reader): AsyncIterableIterator<Uint8Array>;

  /** Synchronously open a file and return an instance of the `File` object.
   *
   *       const file = Deno.openSync("/foo/bar.txt", { read: true, write: true });
   *
   * Requires `allow-read` and `allow-write` permissions depending on openMode.
   */
  export function openSync(path: string, options?: OpenOptions): File;

  /** Synchronously open a file and return an instance of the `File` object.
   *
   *       const file = Deno.openSync("/foo/bar.txt", "r");
   *
   * Requires `allow-read` and `allow-write` permissions depending on openMode.
   */
  export function openSync(path: string, openMode?: OpenMode): File;

  /** Open a file and resolve to an instance of the `File` object.
   *
   *     const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
   *
   * Requires `allow-read` and `allow-write` permissions depending on openMode.
   */
  export function open(path: string, options?: OpenOptions): Promise<File>;

  /** Open a file and resolves to an instance of `Deno.File`.
   *
   *     const file = await Deno.open("/foo/bar.txt, "w+");
   *
   * Requires `allow-read` and `allow-write` permissions depending on openMode.
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

  /** Synchronously read from a file ID into an array buffer.
   *
   * Returns `number | EOF` for the operation.
   *
   *      const file = Deno.openSync("/foo/bar.txt");
   *      const buf = new Uint8Array(100);
   *      const nread = Deno.readSync(file.rid, buf);
   *      const text = new TextDecoder().decode(buf);
   */
  export function readSync(rid: number, p: Uint8Array): number | EOF;

  /** Read from a resource ID into an array buffer.
   *
   * Resolves to the `number | EOF` for the operation.
   *
   *       const file = await Deno.open("/foo/bar.txt");
   *       const buf = new Uint8Array(100);
   *       const nread = await Deno.read(file.rid, buf);
   *       const text = new TextDecoder().decode(buf);
   */
  export function read(rid: number, p: Uint8Array): Promise<number | EOF>;

  /** Synchronously write to the resource ID the contents of the array buffer.
   *
   * Resolves to the number of bytes written.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       const file = Deno.openSync("/foo/bar.txt");
   *       Deno.writeSync(file.rid, data);
   */
  export function writeSync(rid: number, p: Uint8Array): number;

  /** Write to the resource ID the contents of the array buffer.
   *
   * Resolves to the number of bytes written.
   *
   *      const encoder = new TextEncoder();
   *      const data = encoder.encode("Hello world\n");
   *      const file = await Deno.open("/foo/bar.txt");
   *      await Deno.write(file.rid, data);
   */
  export function write(rid: number, p: Uint8Array): Promise<number>;

  /** Synchronously seek a file ID to the given offset under mode given by `whence`.
   *
   *       const file = Deno.openSync("/foo/bar.txt");
   *       Deno.seekSync(file.rid, 0, 0);
   */
  export function seekSync(
    rid: number,
    offset: number,
    whence: SeekMode
  ): number;

  /** Seek a file ID to the given offset under mode given by `whence`.
   *
   *      const file = await Deno.open("/foo/bar.txt");
   *      await Deno.seek(file.rid, 0, 0);
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
   *  Set TTY to be under raw mode or not. */
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

  /** Read `r` until `Deno.EOF` and resolves to the content as
   * `Uint8Array`. */
  export function readAll(r: Reader): Promise<Uint8Array>;

  /** Read `r` until `Deno.EOF` and returns the content as `Uint8Array`. */
  export function readAllSync(r: SyncReader): Uint8Array;

  /** Write all the content of `arr` to `w`. */
  export function writeAll(w: Writer, arr: Uint8Array): Promise<void>;

  /** Synchronously write all the content of `arr` to `w`. */
  export function writeAllSync(w: SyncWriter, arr: Uint8Array): void;

  export interface MkdirOptions {
    /** Defaults to `false`. If set to `true`, means that any intermediate
     * directories will also be created (as with the shell command `mkdir -p`).
     * Intermediate directories are created with the same permissions.
     * When recursive is set to `true`, succeeds silently (without changing any
     * permissions) if a directory already exists at the path. */
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
   *
   * Requires `allow-write` permission. */
  export function mkdirSync(path: string, options?: MkdirOptions): void;

  /** @deprecated */
  export function mkdirSync(
    path: string,
    recursive?: boolean,
    mode?: number
  ): void;

  /** Creates a new directory with the specified path.
   *
   *       await Deno.mkdir("new_dir");
   *       await Deno.mkdir("nested/directories", { recursive: true });
   *
   * Requires `allow-write` permission. */
  export function mkdir(path: string, options?: MkdirOptions): Promise<void>;

  /** @deprecated */
  export function mkdir(
    path: string,
    recursive?: boolean,
    mode?: number
  ): Promise<void>;

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

  /** Synchronously creates a new temporary directory in the directory `dir`,
   * its name beginning with `prefix` and ending with `suffix`.
   *
   * It returns the full path to the newly created directory.
   *
   * If `dir` is unspecified, uses the default directory for temporary files.
   * Multiple programs calling this function simultaneously will create different
   * directories. It is the caller's responsibility to remove the directory when
   * no longer needed.
   *
   *       const tempDirName0 = Deno.makeTempDirSync();
   *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });
   *
   * Requires `allow-write` permission. */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDirSync(options?: MakeTempOptions): string;

  /** Creates a new temporary directory in the directory `dir`, its name
   * beginning with `prefix` and ending with `suffix`.
   *
   * It resolves to the full path to the newly created directory.
   *
   * If `dir` is unspecified, uses the default directory for temporary files.
   * Multiple programs calling this function simultaneously will create different
   * directories. It is the caller's responsibility to remove the directory when
   * no longer needed.
   *
   *       const tempDirName0 = await Deno.makeTempDir();
   *       const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' });
   *
   * Requires `allow-write` permission. */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDir(options?: MakeTempOptions): Promise<string>;

  /** Synchronously creates a new temporary file in the directory `dir`, its name
   * beginning with `prefix` and ending with `suffix`.
   *
   * It returns the full path to the newly created file.
   *
   * If `dir` is unspecified, uses the default directory for temporary files.
   * Multiple programs calling this function simultaneously will create different
   * files. It is the caller's responsibility to remove the file when
   * no longer needed.
   *
   *       const tempFileName0 = Deno.makeTempFileSync();
   *       const tempFileName1 = Deno.makeTempFileSync({ prefix: 'my_temp' });
   *
   * Requires `allow-write` permission. */
  export function makeTempFileSync(options?: MakeTempOptions): string;

  /** Creates a new temporary file in the directory `dir`, its name
   * beginning with `prefix` and ending with `suffix`.
   *
   * It resolves to the full path to the newly created file.
   *
   * If `dir` is unspecified, uses the default directory for temporary files.
   * Multiple programs calling this function simultaneously will create different
   * files. It is the caller's responsibility to remove the file when
   * no longer needed.
   *
   *       const tempFileName0 = await Deno.makeTempFile();
   *       const tempFileName1 = await Deno.makeTempFile({ prefix: 'my_temp' });
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
   * Synchronously changes the access and modification times of a file system
   * object referenced by `path`. Given times are either in seconds (UNIX epoch
   * time) or as `Date` objects.
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
   * Changes the access and modification times of a file system object
   * referenced by `path`. Given times are either in seconds (UNIX epoch time)
   * or as `Date` objects.
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

  /** Synchronously removes the named file or directory. Throws error if
   * permission denied, path not found, or path is a non-empty directory and
   * the `recursive` option isn't set to `true`.
   *
   *       Deno.removeSync("/path/to/dir/or/file", { recursive: false });
   *
   * Requires `allow-write` permission. */
  export function removeSync(path: string, options?: RemoveOptions): void;

  /** Removes the named file or directory. Throws error if permission denied,
   * path not found, or path is a non-empty directory and the `recursive`
   * option isn't set to `true`.
   *
   *       await Deno.remove("/path/to/dir/or/file", { recursive: false });
   *
   * Requires `allow-write` permission. */
  export function remove(path: string, options?: RemoveOptions): Promise<void>;

  /** Synchronously renames (moves) `oldpath` to `newpath`. If `newpath` already
   * exists and is not a directory, `renameSync()` replaces it. OS-specific
   * restrictions may apply when `oldpath` and `newpath` are in different
   * directories.
   *
   *       Deno.renameSync("old/path", "new/path");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function renameSync(oldpath: string, newpath: string): void;

  /** Renames (moves) `oldpath` to `newpath`. If `newpath` already exists and is
   * not a directory, `rename()` replaces it. OS-specific restrictions may apply
   * when `oldpath` and `newpath` are in different directories.
   *
   *       await Deno.rename("old/path", "new/path");
   *
   * Requires `allow-read` and `allow-write`. */
  export function rename(oldpath: string, newpath: string): Promise<void>;

  /** Reads and returns the entire contents of a file.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = Deno.readFileSync("hello.txt");
   *       console.log(decoder.decode(data));
   *
   * Requires `allow-read` permission. */
  export function readFileSync(path: string): Uint8Array;

  /** Reads and resolves to the entire contents of a file.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = await Deno.readFile("hello.txt");
   *       console.log(decoder.decode(data));
   *
   * Requires `allow-read` permission. */
  export function readFile(path: string): Promise<Uint8Array>;

  /** A FileInfo describes a file and is returned by `stat`, `lstat`,
   * `statSync`, `lstatSync`. A list of FileInfo is returned by `readdir`,
   * `readdirSync`. */
  export interface FileInfo {
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
    /** The file or directory name. */
    name: string | null;
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
    /** Returns whether this is info for a regular file. This result is mutually
     * exclusive to `FileInfo.isDirectory` and `FileInfo.isSymlink`. */
    isFile(): boolean;
    /** Returns whether this is info for a regular directory. This result is
     * mutually exclusive to `FileInfo.isFile` and `FileInfo.isSymlink`. */
    isDirectory(): boolean;
    /** Returns whether this is info for a symlink. This result is
     * mutually exclusive to `FileInfo.isFile` and `FileInfo.isDirectory`. */
    isSymlink(): boolean;
  }

  /** Returns absolute normalized path with, symbolic links resolved.
   *
   *       const realPath = Deno.realpathSync("./some/path");
   *
   * Requires `allow-read` permission. */
  export function realpathSync(path: string): string;

  /** Resolves to the absolute normalized path, with symbolic links resolved.
   *
   *       const realPath = await Deno.realpath("./some/path");
   *
   * Requires `allow-read` permission. */
  export function realpath(path: string): Promise<string>;

  /** UNSTABLE: need to consider streaming case
   *
   * Synchronously reads the directory given by `path` and returns an array of
   * `Deno.FileInfo`.
   *
   *       const files = Deno.readdirSync("/");
   *
   * Requires `allow-read` permission. */
  export function readdirSync(path: string): FileInfo[];

  /** UNSTABLE: Maybe need to return an `AsyncIterable`.
   *
   * Reads the directory given by `path` and resolves to an array of `Deno.FileInfo`.
   *
   *       const files = await Deno.readdir("/");
   *
   * Requires `allow-read` permission. */
  export function readdir(path: string): Promise<FileInfo[]>;

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

  /** Returns the destination of the named symbolic link.
   *
   *       const targetPath = Deno.readlinkSync("symlink/path");
   *
   * Requires `allow-read` permission. */
  export function readlinkSync(path: string): string;

  /** Resolves to the destination of the named symbolic link.
   *
   *       const targetPath = await Deno.readlink("symlink/path");
   *
   * Requires `allow-read` permission. */
  export function readlink(path: string): Promise<string>;

  /** Resolves to a `Deno.FileInfo` for the specified `path`. If `path` is a
   * symlink, information for the symlink will be returned.
   *
   *       const fileInfo = await Deno.lstat("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires `allow-read` permission. */
  export function lstat(path: string): Promise<FileInfo>;

  /** Synchronously returns a `Deno.FileInfo` for the specified `path`. If
   * `path` is a symlink, information for the symlink will be returned.
   *
   *       const fileInfo = Deno.lstatSync("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires `allow-read` permission. */
  export function lstatSync(path: string): FileInfo;

  /** Resolves to a `Deno.FileInfo` for the specified `path`. Will always
   * follow symlinks.
   *
   *       const fileInfo = await Deno.stat("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires `allow-read` permission. */
  export function stat(path: string): Promise<FileInfo>;

  /** Synchronously returns a `Deno.FileInfo` for the specified `path`. Will
   * always follow symlinks.
   *
   *       const fileInfo = Deno.statSync("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires `allow-read` permission. */
  export function statSync(path: string): FileInfo;

  /** Creates `newpath` as a hard link to `oldpath`.
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
   * Creates `newpath` as a symbolic link to `oldpath`. The type argument can be
   * set to `dir` or `file`. Is only available on Windows and ignored on other
   * platforms.
   *
   *       Deno.symlinkSync("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function symlinkSync(
    oldpath: string,
    newpath: string,
    type?: string
  ): void;

  /** **UNSTABLE**: `type` argument may be changed to "dir" | "file"
   *
   * Creates `newpath` as a symbolic link to `oldpath`. The type argument can be
   * set to `dir` or `file`. Is only available on Windows and ignored on other
   * platforms.
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

  /** Synchronously write data to the given path, by default creating a new
   * file if needed, else overwriting.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       Deno.writeFileSync("hello.txt", data);
   *
   * Requires `allow-write` permission, and `allow-read` if create is `false`.
   */
  export function writeFileSync(
    path: string,
    data: Uint8Array,
    options?: WriteFileOptions
  ): void;

  /** Write data to the given path, by default creating a new file if needed,
   * else overwriting.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       await Deno.writeFile("hello.txt", data);
   *
   * Requires `allow-write` permission, and `allow-read` if create is `false`.
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
    filename: string;
    /** The line number in the file. It is assumed to be 1-indexed. */
    line: number;
    /** The column number in the file. It is assumed to be 1-indexed. */
    column: number;
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
   *         location: "file://my/module.ts",
   *         line: 5,
   *         column: 15
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
   * specified `len`.
   *
   *       Deno.truncateSync("hello.txt", 10);
   *
   * Requires `allow-write` permission. */
  export function truncateSync(name: string, len?: number): void;

  /** Truncates or extends the specified file, to reach the specified `len`.
   *
   *       await Deno.truncate("hello.txt", 10);
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

  /** **UNSTABLE**: Maybe should remove `how` parameter maybe remove
   * `ShutdownMode` entirely.
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
    [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, Addr]>;
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

    [Symbol.asyncIterator](): AsyncIterator<Conn>;
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
  /** **UNSTABLE**: new API
   *
   * Listen announces on the local transport address.
   *
   *      Deno.listen({ port: 80 })
   *      Deno.listen({ hostname: "192.0.2.1", port: 80 })
   *      Deno.listen({ hostname: "[2001:db8::1]", port: 80 });
   *      Deno.listen({ hostname: "golang.org", port: 80, transport: "tcp" });
   *
   * Requires `allow-net` permission. */
  export function listen(
    options: ListenOptions & { transport?: "tcp" }
  ): Listener;
  /** **UNSTABLE**: new API
   *
   * Listen announces on the local transport address.
   *
   *     Deno.listen({ address: "/foo/bar.sock", transport: "unix" })
   *
   * Requires `allow-read` permission. */
  export function listen(
    options: UnixListenOptions & { transport: "unix" }
  ): Listener;
  /** **UNSTABLE**: new API
   *
   * Listen announces on the local transport address.
   *
   *      Deno.listen({ port: 80, transport: "udp" })
   *      Deno.listen({ hostname: "golang.org", port: 80, transport: "udp" });
   *
   * Requires `allow-net` permission. */
  export function listen(
    options: ListenOptions & { transport: "udp" }
  ): DatagramConn;
  /** **UNSTABLE**: new API
   *
   * Listen announces on the local transport address.
   *
   *     Deno.listen({ address: "/foo/bar.sock", transport: "unixpacket" })
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
   *      Deno.listenTLS({ port: 443, certFile: "./my_server.crt", keyFile: "./my_server.key" });
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
   * transport (default is "tcp").
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

  /** **UNSTABLE**: potentially broken.
   *
   * Receive metrics from the privileged side of Deno.
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

  /** **UNSTABLE**: reconsider return type.
   *
   * Returns a map of open _file like_ resource ids along with their string
   * representations. */
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

  export interface ProcessStatus {
    success: boolean;
    code?: number;
    signal?: number;
  }

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

  /** Spawns new subprocess.
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
   * called. */
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
