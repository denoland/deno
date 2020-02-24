// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  /** The current process id of the runtime. */
  export let pid: number;

  /** Reflects the NO_COLOR environment variable: https://no-color.org/ */
  export let noColor: boolean;

  export type TestFunction = () => void | Promise<void>;

  export interface TestDefinition {
    fn: TestFunction;
    name: string;
  }

  export function test(t: TestDefinition): void;
  export function test(fn: TestFunction): void;
  export function test(name: string, fn: TestFunction): void;

  export interface RunTestsOptions {
    exitOnFail?: boolean;
    only?: RegExp;
    skip?: RegExp;
    disableLog?: boolean;
  }

  export function runTests(opts?: RunTestsOptions): Promise<void>;

  /** Check if running in terminal.
   *
   *       console.log(Deno.isTTY().stdout);
   */
  export function isTTY(): {
    stdin: boolean;
    stdout: boolean;
    stderr: boolean;
  };

  /** Get the loadavg. Requires the `--allow-env` flag.
   *
   *       console.log(Deno.loadavg());
   */
  export function loadavg(): number[];

  /** Get the hostname. Requires the `--allow-env` flag.
   *
   *       console.log(Deno.hostname());
   */
  export function hostname(): string;

  /** Get the OS release. Requires the `--allow-env` flag.
   *
   *       console.log(Deno.osRelease());
   */
  export function osRelease(): string;

  /** Exit the Deno process with optional exit code. */
  export function exit(code?: number): never;

  /** Returns a snapshot of the environment variables at invocation. Mutating a
   * property in the object will set that variable in the environment for
   * the process. The environment object will only accept `string`s
   * as values.
   *
   *       const myEnv = Deno.env();
   *       console.log(myEnv.SHELL);
   *       myEnv.TEST_VAR = "HELLO";
   *       const newEnv = Deno.env();
   *       console.log(myEnv.TEST_VAR == newEnv.TEST_VAR);
   */
  export function env(): {
    [index: string]: string;
  };

  /** Returns the value of an environment variable at invocation.
   * If the variable is not present, `undefined` will be returned.
   *
   *       const myEnv = Deno.env();
   *       console.log(myEnv.SHELL);
   *       myEnv.TEST_VAR = "HELLO";
   *       const newEnv = Deno.env();
   *       console.log(myEnv.TEST_VAR == newEnv.TEST_VAR);
   */
  export function env(key: string): string | undefined;

  /** UNSTABLE */
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
    | "video";

  // TODO(ry) markdown in jsdoc broken https://deno.land/typedoc/index.html#dir
  /**
   * UNSTABLE: Might rename method dir and type alias DirKind.
   *
   * Returns the user and platform specific directories.
   * Requires the `--allow-env` flag.
   * Returns null if there is no applicable directory or if any other error
   * occurs.
   *
   * Argument values: "home", "cache", "config", "executable", "data",
   * "data_local", "audio", "desktop", "document", "download", "font", "picture",
   * "public", "template", "video"
   *
   * "cache"
   *
   * |Platform | Value                               | Example                      |
   * | ------- | ----------------------------------- | ---------------------------- |
   * | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
   * | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
   * | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
   *
   * "config"
   *
   * |Platform | Value                                 | Example                          |
   * | ------- | ------------------------------------- | -------------------------------- |
   * | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config              |
   * | macOS   | `$HOME`/Library/Preferences           | /Users/Alice/Library/Preferences |
   * | Windows | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming   |
   *
   * "executable"
   *
   * |Platform | Value                                                           | Example                |
   * | ------- | --------------------------------------------------------------- | -----------------------|
   * | Linux   | `XDG_BIN_HOME` or `$XDG_DATA_HOME`/../bin or `$HOME`/.local/bin | /home/alice/.local/bin |
   * | macOS   | -                                                               | -                      |
   * | Windows | -                                                               | -                      |
   *
   * "data"
   *
   * |Platform | Value                                    | Example                                  |
   * | ------- | ---------------------------------------- | ---------------------------------------- |
   * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
   * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
   * | Windows | `{FOLDERID_RoamingAppData}`              | C:\Users\Alice\AppData\Roaming           |
   *
   * "data_local"
   *
   * |Platform | Value                                    | Example                                  |
   * | ------- | ---------------------------------------- | ---------------------------------------- |
   * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
   * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
   * | Windows | `{FOLDERID_LocalAppData}`                | C:\Users\Alice\AppData\Local             |
   *
   * "audio"
   *
   * |Platform | Value              | Example              |
   * | ------- | ------------------ | -------------------- |
   * | Linux   | `XDG_MUSIC_DIR`    | /home/alice/Music    |
   * | macOS   | `$HOME`/Music      | /Users/Alice/Music   |
   * | Windows | `{FOLDERID_Music}` | C:\Users\Alice\Music |
   *
   * "desktop"
   *
   * |Platform | Value                | Example                |
   * | ------- | -------------------- | ---------------------- |
   * | Linux   | `XDG_DESKTOP_DIR`    | /home/alice/Desktop    |
   * | macOS   | `$HOME`/Desktop      | /Users/Alice/Desktop   |
   * | Windows | `{FOLDERID_Desktop}` | C:\Users\Alice\Desktop |
   *
   * "document"
   *
   * |Platform | Value                  | Example                  |
   * | ------- | ---------------------- | ------------------------ |
   * | Linux   | `XDG_DOCUMENTS_DIR`    | /home/alice/Documents    |
   * | macOS   | `$HOME`/Documents      | /Users/Alice/Documents   |
   * | Windows | `{FOLDERID_Documents}` | C:\Users\Alice\Documents |
   *
   * "download"
   *
   * |Platform | Value                  | Example                  |
   * | ------- | ---------------------- | ------------------------ |
   * | Linux   | `XDG_DOWNLOAD_DIR`     | /home/alice/Downloads    |
   * | macOS   | `$HOME`/Downloads      | /Users/Alice/Downloads   |
   * | Windows | `{FOLDERID_Downloads}` | C:\Users\Alice\Downloads |
   *
   * "font"
   *
   * |Platform | Value                                                | Example                        |
   * | ------- | ---------------------------------------------------- | ------------------------------ |
   * | Linux   | `$XDG_DATA_HOME`/fonts or `$HOME`/.local/share/fonts | /home/alice/.local/share/fonts |
   * | macOS   | `$HOME/Library/Fonts`                                | /Users/Alice/Library/Fonts     |
   * | Windows | –                                                    | –                              |
   *
   * "picture"
   *
   * |Platform | Value                 | Example                 |
   * | ------- | --------------------- | ----------------------- |
   * | Linux   | `XDG_PICTURES_DIR`    | /home/alice/Pictures    |
   * | macOS   | `$HOME`/Pictures      | /Users/Alice/Pictures   |
   * | Windows | `{FOLDERID_Pictures}` | C:\Users\Alice\Pictures |
   *
   * "public"
   *
   * |Platform | Value                 | Example             |
   * | ------- | --------------------- | ------------------- |
   * | Linux   | `XDG_PUBLICSHARE_DIR` | /home/alice/Public  |
   * | macOS   | `$HOME`/Public        | /Users/Alice/Public |
   * | Windows | `{FOLDERID_Public}`   | C:\Users\Public     |
   *
   * "template"
   *
   * |Platform | Value                  | Example                                                    |
   * | ------- | ---------------------- | ---------------------------------------------------------- |
   * | Linux   | `XDG_TEMPLATES_DIR`    | /home/alice/Templates                                      |
   * | macOS   | –                      | –                                                          |
   * | Windows | `{FOLDERID_Templates}` | C:\Users\Alice\AppData\Roaming\Microsoft\Windows\Templates |
   *
   * "video"
   *
   * |Platform | Value               | Example               |
   * | ------- | ------------------- | --------------------- |
   * | Linux   | `XDG_VIDEOS_DIR`    | /home/alice/Videos    |
   * | macOS   | `$HOME`/Movies      | /Users/Alice/Movies   |
   * | Windows | `{FOLDERID_Videos}` | C:\Users\Alice\Videos |
   */
  export function dir(kind: DirKind): string | null;

  /**
   * Returns the path to the current deno executable.
   * Requires the `--allow-env` flag.
   */
  export function execPath(): string;

  /**
   * UNSTABLE: maybe needs permissions.
   *
   * `cwd()` Return a string representing the current working directory.
   * If the current directory can be reached via multiple paths
   * (due to symbolic links), `cwd()` may return
   * any one of them.
   * throws `NotFound` exception if directory not available
   */
  export function cwd(): string;

  /**
   * UNSTABLE: maybe needs permissions.
   *
   * `chdir()` Change the current working directory to path.
   * throws `NotFound` exception if directory not available
   */
  export function chdir(directory: string): void;

  /** UNSTABLE: might move to Deno.symbols */
  export const EOF: unique symbol;

  /** UNSTABLE: might move to Deno.symbols  */
  export type EOF = typeof EOF;

  /** UNSTABLE: maybe remove "SEEK_" prefix. Maybe capitalization wrong. */
  export enum SeekMode {
    SEEK_START = 0,
    SEEK_CURRENT = 1,
    SEEK_END = 2
  }

  /** UNSTABLE: Make Reader into iterator of some sort */
  export interface Reader {
    /** Reads up to p.byteLength bytes into `p`. It resolves to the number
     * of bytes read (`0` < `n` <= `p.byteLength`) and rejects if any error encountered.
     * Even if `read()` returns `n` < `p.byteLength`, it may use all of `p` as
     * scratch space during the call. If some data is available but not
     * `p.byteLength` bytes, `read()` conventionally returns what is available
     * instead of waiting for more.
     *
     * When `read()` encounters end-of-file condition, it returns EOF symbol.
     *
     * When `read()` encounters an error, it rejects with an error.
     *
     * Callers should always process the `n` > `0` bytes returned before
     * considering the EOF. Doing so correctly handles I/O errors that happen
     * after reading some bytes and also both of the allowed EOF behaviors.
     *
     * Implementations must not retain `p`.
     */
    read(p: Uint8Array): Promise<number | EOF>;
  }
  export interface SyncReader {
    readSync(p: Uint8Array): number | EOF;
  }

  export interface Writer {
    /** Writes `p.byteLength` bytes from `p` to the underlying data
     * stream. It resolves to the number of bytes written from `p` (`0` <= `n` <=
     * `p.byteLength`) and any error encountered that caused the write to stop
     * early. `write()` must return a non-null error if it returns `n` <
     * `p.byteLength`. write() must not modify the slice data, even temporarily.
     *
     * Implementations must not retain `p`.
     */
    write(p: Uint8Array): Promise<number>;
  }
  export interface SyncWriter {
    writeSync(p: Uint8Array): number;
  }
  export interface Closer {
    close(): void;
  }
  export interface Seeker {
    /** Seek sets the offset for the next `read()` or `write()` to offset,
     * interpreted according to `whence`: `SeekStart` means relative to the start
     * of the file, `SeekCurrent` means relative to the current offset, and
     * `SeekEnd` means relative to the end. Seek returns the new offset relative
     * to the start of the file and an error, if any.
     *
     * Seeking to an offset before the start of the file is an error. Seeking to
     * any positive offset is legal, but the behavior of subsequent I/O operations
     * on the underlying object is implementation-dependent.
     */
    seek(offset: number, whence: SeekMode): Promise<void>;
  }
  export interface SyncSeeker {
    seekSync(offset: number, whence: SeekMode): void;
  }
  export interface ReadCloser extends Reader, Closer {}
  export interface WriteCloser extends Writer, Closer {}
  export interface ReadSeeker extends Reader, Seeker {}
  export interface WriteSeeker extends Writer, Seeker {}
  export interface ReadWriteCloser extends Reader, Writer, Closer {}
  export interface ReadWriteSeeker extends Reader, Writer, Seeker {}
  /** Copies from `src` to `dst` until either `EOF` is reached on `src`
   * or an error occurs. It returns the number of bytes copied and the first
   * error encountered while copying, if any.
   *
   * Because `copy()` is defined to read from `src` until `EOF`, it does not
   * treat an `EOF` from `read()` as an error to be reported.
   */
  export function copy(dst: Writer, src: Reader): Promise<number>;
  /** Turns `r` into async iterator.
   *
   *      for await (const chunk of toAsyncIterator(reader)) {
   *          console.log(chunk)
   *      }
   */
  export function toAsyncIterator(r: Reader): AsyncIterableIterator<Uint8Array>;

  // @url js/files.d.ts

  /** Open a file and return an instance of the `File` object
   *  synchronously.
   *
   *       const file = Deno.openSYNC("/foo/bar.txt", { read: true, write: true });
   *
   * Requires allow-read or allow-write or both depending on mode.
   */
  export function openSync(filename: string, options?: OpenOptions): File;

  /** Open a file and return an instance of the `File` object
   *  synchronously.
   *
   *       const file = Deno.openSync("/foo/bar.txt", "r");
   *
   * Requires allow-read or allow-write or both depending on mode.
   */
  export function openSync(filename: string, mode?: OpenMode): File;

  /** Open a file and return an instance of the `File` object.
   *
   *     const file = await Deno.open("/foo/bar.txt", { read: true, write: true });
   *
   * Requires allow-read or allow-write or both depending on mode.
   */
  export function open(filename: string, options?: OpenOptions): Promise<File>;

  /** Open a file and return an instance of the `File` object.
   *
   *     const file = await Deno.open("/foo/bar.txt, "w+");
   *
   * Requires allow-read or allow-write or both depending on mode.
   */
  export function open(filename: string, mode?: OpenMode): Promise<File>;

  /** Creates a file if none exists or truncates an existing file and returns
   *  an instance of the `File` object synchronously.
   *
   *       const file = Deno.createSync("/foo/bar.txt");
   *
   * Requires allow-read and allow-write.
   */
  export function createSync(filename: string): File;
  /** Creates a file if none exists or truncates an existing file and returns
   *  an instance of the `File` object.
   *
   *       const file = await Deno.create("/foo/bar.txt");
   *
   * Requires allow-read and allow-write.
   */
  export function create(filename: string): Promise<File>;

  /** Read synchronously from a file ID into an array buffer.
   *
   * Return `number | EOF` for the operation.
   *
   *      const file = Deno.openSync("/foo/bar.txt");
   *      const buf = new Uint8Array(100);
   *      const nread = Deno.readSync(file.rid, buf);
   *      const text = new TextDecoder().decode(buf);
   *
   */
  export function readSync(rid: number, p: Uint8Array): number | EOF;

  /** Read from a resource ID into an array buffer.
   *
   * Resolves with the `number | EOF` for the operation.
   *
   *       const file = await Deno.open("/foo/bar.txt");
   *       const buf = new Uint8Array(100);
   *       const nread = await Deno.read(file.rid, buf);
   *       const text = new TextDecoder().decode(buf);
   */
  export function read(rid: number, p: Uint8Array): Promise<number | EOF>;

  /** Write synchronously to the resource ID the contents of the array buffer.
   *
   * Resolves with the number of bytes written.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       const file = Deno.openSync("/foo/bar.txt");
   *       Deno.writeSync(file.rid, data);
   */
  export function writeSync(rid: number, p: Uint8Array): number;

  /** Write to the resource ID the contents of the array buffer.
   *
   * Resolves with the number of bytes written.
   *
   *      const encoder = new TextEncoder();
   *      const data = encoder.encode("Hello world\n");
   *      const file = await Deno.open("/foo/bar.txt");
   *      await Deno.write(file.rid, data);
   *
   */
  export function write(rid: number, p: Uint8Array): Promise<number>;

  /** Seek a file ID synchronously to the given offset under mode given by `whence`.
   *
   *       const file = Deno.openSync("/foo/bar.txt");
   *       Deno.seekSync(file.rid, 0, 0);
   */
  export function seekSync(rid: number, offset: number, whence: SeekMode): void;

  /** Seek a file ID to the given offset under mode given by `whence`.
   *
   *      (async () => {
   *        const file = await Deno.open("/foo/bar.txt");
   *        await Deno.seek(file.rid, 0, 0);
   *      })();
   */
  export function seek(
    rid: number,
    offset: number,
    whence: SeekMode
  ): Promise<void>;

  /** Close the given resource ID. */
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
    seek(offset: number, whence: SeekMode): Promise<void>;
    seekSync(offset: number, whence: SeekMode): void;
    close(): void;
  }
  /** An instance of `File` for stdin. */
  export const stdin: File;
  /** An instance of `File` for stdout. */
  export const stdout: File;
  /** An instance of `File` for stderr. */
  export const stderr: File;

  export interface OpenOptions {
    /** Sets the option for read access. This option, when true, will indicate that the file should be read-able if opened. */
    read?: boolean;
    /** Sets the option for write access.
     * This option, when true, will indicate that the file should be write-able if opened.
     * If the file already exists, any write calls on it will overwrite its contents, without truncating it.
     */
    write?: boolean;
    /* Sets the option for creating a new file.
     * This option indicates whether a new file will be created if the file does not yet already exist.
     * In order for the file to be created, write or append access must be used.
     */
    create?: boolean;
    /** Sets the option for truncating a previous file.
     * If a file is successfully opened with this option set it will truncate the file to 0 length if it already exists.
     * The file must be opened with write access for truncate to work.
     */
    truncate?: boolean;
    /**Sets the option for the append mode.
     * This option, when true, means that writes will append to a file instead of overwriting previous contents.
     * Note that setting { write: true, append: true } has the same effect as setting only { append: true }.
     */
    append?: boolean;
    /** Sets the option to always create a new file.
     * This option indicates whether a new file will be created. No file is allowed to exist at the target location, also no (dangling) symlink.
     * If { createNew: true } is set, create and truncate are ignored.
     */
    createNew?: boolean;
  }

  export type OpenMode =
    /** Read-only. Default. Starts at beginning of file. */
    | "r"
    /** Read-write. Start at beginning of file. */
    | "r+"
    /** Write-only. Opens and truncates existing file or creates new one for
     * writing only.
     */
    | "w"
    /** Read-write. Opens and truncates existing file or creates new one for
     * writing and reading.
     */
    | "w+"
    /** Write-only. Opens existing file or creates new one. Each write appends
     * content to the end of file.
     */
    | "a"
    /** Read-write. Behaves like "a" and allows to read from file. */
    | "a+"
    /** Write-only. Exclusive create - creates new file only if one doesn't exist
     * already.
     */
    | "x"
    /** Read-write. Behaves like `x` and allows to read from file. */
    | "x+";

  // @url js/buffer.d.ts

  /** A Buffer is a variable-sized buffer of bytes with read() and write()
   * methods. Based on https://golang.org/pkg/bytes/#Buffer
   */
  export class Buffer implements Reader, SyncReader, Writer, SyncWriter {
    private buf;
    private off;
    constructor(ab?: ArrayBuffer);
    /** bytes() returns a slice holding the unread portion of the buffer.
     * The slice is valid for use only until the next buffer modification (that
     * is, only until the next call to a method like read(), write(), reset(), or
     * truncate()). The slice aliases the buffer content at least until the next
     * buffer modification, so immediate changes to the slice will affect the
     * result of future reads.
     */
    bytes(): Uint8Array;
    /** toString() returns the contents of the unread portion of the buffer
     * as a string. Warning - if multibyte characters are present when data is
     * flowing through the buffer, this method may result in incorrect strings
     * due to a character being split.
     */
    toString(): string;
    /** empty() returns whether the unread portion of the buffer is empty. */
    empty(): boolean;
    /** length is a getter that returns the number of bytes of the unread
     * portion of the buffer
     */
    readonly length: number;
    /** Returns the capacity of the buffer's underlying byte slice, that is,
     * the total space allocated for the buffer's data.
     */
    readonly capacity: number;
    /** truncate() discards all but the first n unread bytes from the buffer but
     * continues to use the same allocated storage.  It throws if n is negative or
     * greater than the length of the buffer.
     */
    truncate(n: number): void;
    /** reset() resets the buffer to be empty, but it retains the underlying
     * storage for use by future writes. reset() is the same as truncate(0)
     */
    reset(): void;
    /** _tryGrowByReslice() is a version of grow for the fast-case
     * where the internal buffer only needs to be resliced. It returns the index
     * where bytes should be written and whether it succeeded.
     * It returns -1 if a reslice was not needed.
     */
    private _tryGrowByReslice;
    private _reslice;
    /** readSync() reads the next len(p) bytes from the buffer or until the buffer
     * is drained. The return value n is the number of bytes read. If the
     * buffer has no data to return, eof in the response will be true.
     */
    readSync(p: Uint8Array): number | EOF;
    read(p: Uint8Array): Promise<number | EOF>;
    writeSync(p: Uint8Array): number;
    write(p: Uint8Array): Promise<number>;
    /** _grow() grows the buffer to guarantee space for n more bytes.
     * It returns the index where bytes should be written.
     * If the buffer can't grow it will throw with Error.
     */
    private _grow;
    /** grow() grows the buffer's capacity, if necessary, to guarantee space for
     * another n bytes. After grow(n), at least n bytes can be written to the
     * buffer without another allocation. If n is negative, grow() will panic. If
     * the buffer can't grow it will throw Error.
     * Based on https://golang.org/pkg/bytes/#Buffer.Grow
     */
    grow(n: number): void;
    /** readFrom() reads data from r until EOF and appends it to the buffer,
     * growing the buffer as needed. It returns the number of bytes read. If the
     * buffer becomes too large, readFrom will panic with Error.
     * Based on https://golang.org/pkg/bytes/#Buffer.ReadFrom
     */
    readFrom(r: Reader): Promise<number>;
    /** Sync version of `readFrom`
     */
    readFromSync(r: SyncReader): number;
  }

  /** Read `r` until EOF and return the content as `Uint8Array` */
  export function readAll(r: Reader): Promise<Uint8Array>;

  /** Read synchronously `r` until EOF and return the content as `Uint8Array`  */
  export function readAllSync(r: SyncReader): Uint8Array;

  /** Write all the content of `arr` to `w` */
  export function writeAll(w: Writer, arr: Uint8Array): Promise<void>;

  /** Write synchronously all the content of `arr` to `w` */
  export function writeAllSync(w: SyncWriter, arr: Uint8Array): void;

  export interface MkdirOption {
    recursive?: boolean;
    mode?: number;
  }

  /** Creates a new directory with the specified path synchronously.
   * If `recursive` is set to true, nested directories will be created (also known
   * as "mkdir -p").
   * `mode` sets permission bits (before umask) on UNIX and does nothing on
   * Windows.
   *
   *       Deno.mkdirSync("new_dir");
   *       Deno.mkdirSync("nested/directories", { recursive: true });
   *
   * Requires allow-write.
   */
  export function mkdirSync(path: string, options?: MkdirOption): void;

  /** Deprecated */
  export function mkdirSync(
    path: string,
    recursive?: boolean,
    mode?: number
  ): void;

  /** Creates a new directory with the specified path.
   * If `recursive` is set to true, nested directories will be created (also known
   * as "mkdir -p").
   * `mode` sets permission bits (before umask) on UNIX and does nothing on
   * Windows.
   *
   *       await Deno.mkdir("new_dir");
   *       await Deno.mkdir("nested/directories", { recursive: true });
   *
   * Requires allow-write.
   */
  export function mkdir(path: string, options?: MkdirOption): Promise<void>;

  /** Deprecated */
  export function mkdir(
    path: string,
    recursive?: boolean,
    mode?: number
  ): Promise<void>;

  // @url js/make_temp.d.ts

  export interface MakeTempOptions {
    dir?: string;
    prefix?: string;
    suffix?: string;
  }

  /** makeTempDirSync is the synchronous version of `makeTempDir`.
   *
   *       const tempDirName0 = Deno.makeTempDirSync();
   *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });
   *
   * Requires allow-write.
   */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDirSync(options?: MakeTempOptions): string;

  /** makeTempDir creates a new temporary directory in the directory `dir`, its
   * name beginning with `prefix` and ending with `suffix`.
   * It returns the full path to the newly created directory.
   * If `dir` is unspecified, tempDir uses the default directory for temporary
   * files. Multiple programs calling tempDir simultaneously will not choose the
   * same directory. It is the caller's responsibility to remove the directory
   * when no longer needed.
   *
   *       const tempDirName0 = await Deno.makeTempDir();
   *       const tempDirName1 = await Deno.makeTempDir({ prefix: 'my_temp' });
   *
   * Requires allow-write.
   */
  // TODO(ry) Doesn't check permissions.
  export function makeTempDir(options?: MakeTempOptions): Promise<string>;

  /** makeTempFileSync is the synchronous version of `makeTempFile`.
   *
   *       const tempFileName0 = Deno.makeTempFileSync();
   *       const tempFileName1 = Deno.makeTempFileSync({ prefix: 'my_temp' });
   */
  export function makeTempFileSync(options?: MakeTempOptions): string;

  /** makeTempFile creates a new temporary file in the directory `dir`, its
   * name beginning with `prefix` and ending with `suffix`.
   * It returns the full path to the newly created file.
   * If `dir` is unspecified, tempFile uses the default directory for temporary
   * files. Multiple programs calling tempFile simultaneously will not choose the
   * same directory. It is the caller's responsibility to remove the file
   * when no longer needed.
   *
   *       const tempFileName0 = await Deno.makeTempFile();
   *       const tempFileName1 = await Deno.makeTempFile({ prefix: 'my_temp' });
   */
  export function makeTempFile(options?: MakeTempOptions): Promise<string>;

  /** Changes the permission of a specific file/directory of specified path
   * synchronously.
   *
   *       Deno.chmodSync("/path/to/file", 0o666);
   *
   * Needs allow-write
   */
  export function chmodSync(path: string, mode: number): void;

  /** Changes the permission of a specific file/directory of specified path.
   *
   *       await Deno.chmod("/path/to/file", 0o666);
   *
   * Needs allow-write
   */
  export function chmod(path: string, mode: number): Promise<void>;

  /**
   * Change owner of a regular file or directory synchronously. Unix only at the moment.
   *
   * Needs allow-write permission.
   *
   * @param path path to the file
   * @param uid user id of the new owner
   * @param gid group id of the new owner
   */
  export function chownSync(path: string, uid: number, gid: number): void;

  /**
   * Change owner of a regular file or directory asynchronously. Unix only at the moment.
   *
   * Needs allow-write permission.
   *
   * @param path path to the file
   * @param uid user id of the new owner
   * @param gid group id of the new owner
   */
  export function chown(path: string, uid: number, gid: number): Promise<void>;

  /** UNSTABLE: needs investigation into high precision time.
   *
   * Synchronously changes the access and modification times of a file system
   * object referenced by `filename`. Given times are either in seconds
   * (Unix epoch time) or as `Date` objects.
   *
   *       Deno.utimeSync("myfile.txt", 1556495550, new Date());
   *
   * Requires allow-write.
   */
  export function utimeSync(
    filename: string,
    atime: number | Date,
    mtime: number | Date
  ): void;

  /** UNSTABLE: needs investigation into high precision time.
   *
   * Changes the access and modification times of a file system object
   * referenced by `filename`. Given times are either in seconds
   * (Unix epoch time) or as `Date` objects.
   *
   *       await Deno.utime("myfile.txt", 1556495550, new Date());
   *
   * Requires allow-write.
   */
  export function utime(
    filename: string,
    atime: number | Date,
    mtime: number | Date
  ): Promise<void>;

  /** UNSTABLE: rename to RemoveOptions */
  export interface RemoveOption {
    recursive?: boolean;
  }

  /** Removes the named file or directory synchronously. Would throw
   * error if permission denied, not found, or directory not empty if `recursive`
   * set to false.
   * `recursive` is set to false by default.
   *
   *       Deno.removeSync("/path/to/dir/or/file", {recursive: false});
   *
   * Requires allow-write permission.
   */

  export function removeSync(path: string, options?: RemoveOption): void;
  /** Removes the named file or directory. Would throw error if
   * permission denied, not found, or directory not empty if `recursive` set
   * to false.
   * `recursive` is set to false by default.
   *
   *       await Deno.remove("/path/to/dir/or/file", {recursive: false});
   *
   * Requires allow-write permission.
   */
  export function remove(path: string, options?: RemoveOption): Promise<void>;

  /** Synchronously renames (moves) `oldpath` to `newpath`. If `newpath` already
   * exists and is not a directory, `renameSync()` replaces it. OS-specific
   * restrictions may apply when `oldpath` and `newpath` are in different
   * directories.
   *
   *       Deno.renameSync("old/path", "new/path");
   *
   * Requires allow-read and allow-write.
   */
  export function renameSync(oldpath: string, newpath: string): void;

  /** Renames (moves) `oldpath` to `newpath`. If `newpath` already exists and is
   * not a directory, `rename()` replaces it. OS-specific restrictions may apply
   * when `oldpath` and `newpath` are in different directories.
   *
   *       await Deno.rename("old/path", "new/path");
   *
   * Requires allow-read and allow-write.
   */
  export function rename(oldpath: string, newpath: string): Promise<void>;

  // @url js/read_file.d.ts

  /** Read the entire contents of a file synchronously.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = Deno.readFileSync("hello.txt");
   *       console.log(decoder.decode(data));
   *
   * Requires allow-read.
   */
  export function readFileSync(filename: string): Uint8Array;

  /** Read the entire contents of a file.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = await Deno.readFile("hello.txt");
   *       console.log(decoder.decode(data));
   *
   * Requires allow-read.
   */
  export function readFile(filename: string): Promise<Uint8Array>;

  /** UNSTABLE: 'len' maybe should be 'length' or 'size'.
   *
   * A FileInfo describes a file and is returned by `stat`, `lstat`,
   * `statSync`, `lstatSync`.
   */
  export interface FileInfo {
    /** UNSTABLE: 'len' maybe should be 'length' or 'size'.
     *
     * The size of the file, in bytes. */
    len: number;
    /** The last modification time of the file. This corresponds to the `mtime`
     * field from `stat` on Unix and `ftLastWriteTime` on Windows. This may not
     * be available on all platforms.
     */
    modified: number | null;
    /** The last access time of the file. This corresponds to the `atime`
     * field from `stat` on Unix and `ftLastAccessTime` on Windows. This may not
     * be available on all platforms.
     */
    accessed: number | null;
    /** The last access time of the file. This corresponds to the `birthtime`
     * field from `stat` on Unix and `ftCreationTime` on Windows. This may not
     * be available on all platforms.
     */
    created: number | null;
    /** The file or directory name. */
    name: string | null;
    /** ID of the device containing the file. Unix only. */
    dev: number | null;
    /** Inode number. Unix only. */
    ino: number | null;
    /** UNSTABLE: Match behavior with Go on windows for mode.
     *
     * The underlying raw st_mode bits that contain the standard Unix permissions
     * for this file/directory.
     */
    mode: number | null;
    /** Number of hard links pointing to this file. Unix only. */
    nlink: number | null;
    /** User ID of the owner of this file. Unix only. */
    uid: number | null;
    /** User ID of the owner of this file. Unix only. */
    gid: number | null;
    /** Device ID of this file. Unix only. */
    rdev: number | null;
    /** Blocksize for filesystem I/O. Unix only. */
    blksize: number | null;
    /** Number of blocks allocated to the file, in 512-byte units. Unix only. */
    blocks: number | null;
    /** Returns whether this is info for a regular file. This result is mutually
     * exclusive to `FileInfo.isDirectory` and `FileInfo.isSymlink`.
     */
    isFile(): boolean;
    /** Returns whether this is info for a regular directory. This result is
     * mutually exclusive to `FileInfo.isFile` and `FileInfo.isSymlink`.
     */
    isDirectory(): boolean;
    /** Returns whether this is info for a symlink. This result is
     * mutually exclusive to `FileInfo.isFile` and `FileInfo.isDirectory`.
     */
    isSymlink(): boolean;
  }

  // @url js/realpath.d.ts

  /** Returns absolute normalized path with symbolic links resolved
   * synchronously.
   *
   *       const realPath = Deno.realpathSync("./some/path");
   *
   * Requires allow-read.
   */
  export function realpathSync(path: string): string;

  /** Returns absolute normalized path with symbolic links resolved.
   *
   *       const realPath = await Deno.realpath("./some/path");
   *
   * Requires allow-read.
   */
  export function realpath(path: string): Promise<string>;

  /** UNSTABLE: Unstable rename to readdirSync.
   *
   * Reads the directory given by path and returns a list of file info
   * synchronously.
   *
   *       const files = Deno.readDirSync("/");
   *
   * Requires allow-read.
   */
  export function readDirSync(path: string): FileInfo[];

  /** UNSTABLE: Unstable rename to readdir. Maybe need to return AsyncIterable.
   *
   * Reads the directory given by path and returns a list of file info.
   *
   *       const files = await Deno.readDir("/");
   *
   * Requires allow-read.
   */
  export function readDir(path: string): Promise<FileInfo[]>;

  /** Copies the contents of a file to another by name synchronously.
   * Creates a new file if target does not exists, and if target exists,
   * overwrites original content of the target file.
   *
   * It would also copy the permission of the original file
   * to the destination.
   *
   *       Deno.copyFileSync("from.txt", "to.txt");
   *
   * Needs allow-read and allow-write permissions.
   */
  export function copyFileSync(from: string, to: string): void;
  /** Copies the contents of a file to another by name.
   *
   * Creates a new file if target does not exists, and if target exists,
   * overwrites original content of the target file.
   *
   * It would also copy the permission of the original file
   * to the destination.
   *
   *       await Deno.copyFile("from.txt", "to.txt");
   *
   * Needs allow-read and allow-write permissions.
   */
  export function copyFile(from: string, to: string): Promise<void>;

  // @url js/read_link.d.ts

  /** Returns the destination of the named symbolic link synchronously.
   *
   *       const targetPath = Deno.readlinkSync("symlink/path");
   *
   * Requires allow-read.
   */
  export function readlinkSync(name: string): string;

  /** Returns the destination of the named symbolic link.
   *
   *       const targetPath = await Deno.readlink("symlink/path");
   *
   * Requires allow-read.
   */
  export function readlink(name: string): Promise<string>;

  interface StatResponse {
    isFile: boolean;
    isSymlink: boolean;
    len: number;
    modified: number;
    accessed: number;
    created: number;
    name: string | null;
    dev: number;
    ino: number;
    mode: number;
    nlink: number;
    uid: number;
    gid: number;
    rdev: number;
    blksize: number;
    blocks: number;
  }
  /** Queries the file system for information on the path provided. If the given
   * path is a symlink information about the symlink will be returned.
   *
   *       const fileInfo = await Deno.lstat("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires allow-read permission.
   */
  export function lstat(filename: string): Promise<FileInfo>;

  /** Queries the file system for information on the path provided synchronously.
   * If the given path is a symlink information about the symlink will be
   * returned.
   *
   *       const fileInfo = Deno.lstatSync("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires allow-read permission.
   */
  export function lstatSync(filename: string): FileInfo;

  /** Queries the file system for information on the path provided. `stat` Will
   * always follow symlinks.
   *
   *       const fileInfo = await Deno.stat("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires allow-read permission.
   */
  export function stat(filename: string): Promise<FileInfo>;
  /** Queries the file system for information on the path provided synchronously.
   * `statSync` Will always follow symlinks.
   *
   *       const fileInfo = Deno.statSync("hello.txt");
   *       assert(fileInfo.isFile());
   *
   * Requires allow-read permission.
   */
  export function statSync(filename: string): FileInfo;

  /** Synchronously creates `newname` as a hard link to `oldname`.
   *
   *       Deno.linkSync("old/name", "new/name");
   *
   * Requires allow-read and allow-write permissions.
   */
  export function linkSync(oldname: string, newname: string): void;

  /** Creates `newname` as a hard link to `oldname`.
   *
   *       await Deno.link("old/name", "new/name");
   *
   * Requires allow-read and allow-write permissions.
   */
  export function link(oldname: string, newname: string): Promise<void>;

  /** UNSTABLE type argument may be changed to "dir" | "file"
   *
   * Synchronously creates `newname` as a symbolic link to `oldname`. The type
   * argument can be set to `dir` or `file` and is only available on Windows
   * (ignored on other platforms).
   *
   *       Deno.symlinkSync("old/name", "new/name");
   *
   * Requires allow-read and allow-write permissions.
   */
  export function symlinkSync(
    oldname: string,
    newname: string,
    type?: string
  ): void;

  /** UNSTABLE type argument may be changed to "dir" | "file"
   *
   * Creates `newname` as a symbolic link to `oldname`. The type argument can be
   * set to `dir` or `file` and is only available on Windows (ignored on other
   * platforms).
   *
   *       await Deno.symlink("old/name", "new/name");
   *
   * Requires allow-read and allow-write permissions.
   */
  export function symlink(
    oldname: string,
    newname: string,
    type?: string
  ): Promise<void>;

  /** Options for writing to a file.
   * `perm` would change the file's permission if set.
   * `create` decides if the file should be created if not exists (default: true)
   * `append` decides if the file should be appended (default: false)
   */
  export interface WriteFileOptions {
    perm?: number;
    create?: boolean;
    append?: boolean;
  }

  /** Write a new file, with given filename and data synchronously.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       Deno.writeFileSync("hello.txt", data);
   *
   * Requires allow-write and allow-read if create is false.
   */
  export function writeFileSync(
    filename: string,
    data: Uint8Array,
    options?: WriteFileOptions
  ): void;

  /** Write a new file, with given filename and data.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       await Deno.writeFile("hello.txt", data);
   *
   * Requires allow-write and allow-read if create is false.
   */
  export function writeFile(
    filename: string,
    data: Uint8Array,
    options?: WriteFileOptions
  ): Promise<void>;

  /** UNSTABLE: Should not have same name as window.location type. */
  interface Location {
    /** The full url for the module, e.g. `file://some/file.ts` or
     * `https://some/file.ts`. */
    filename: string;
    /** The line number in the file.  It is assumed to be 1-indexed. */
    line: number;
    /** The column number in the file.  It is assumed to be 1-indexed. */
    column: number;
  }

  /** UNSTABLE: new API, not yet vetted.
   *
   * Given a current location in a module, lookup the source location and
   * return it.
   *
   * When Deno transpiles code, it keep source maps of the transpiled code.  This
   * function can be used to lookup the original location.  This is automatically
   * done when accessing the `.stack` of an error, or when an uncaught error is
   * logged.  This function can be used to perform the lookup for creating better
   * error handling.
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
   *
   */
  export function applySourceMap(location: Location): Location;

  /* eslint-disable @typescript-eslint/no-unused-vars */
  namespace Err {
    class NotFound extends Error {
      constructor(msg: string);
    }
    class PermissionDenied extends Error {
      constructor(msg: string);
    }
    class ConnectionRefused extends Error {
      constructor(msg: string);
    }
    class ConnectionReset extends Error {
      constructor(msg: string);
    }
    class ConnectionAborted extends Error {
      constructor(msg: string);
    }
    class NotConnected extends Error {
      constructor(msg: string);
    }
    class AddrInUse extends Error {
      constructor(msg: string);
    }
    class AddrNotAvailable extends Error {
      constructor(msg: string);
    }
    class BrokenPipe extends Error {
      constructor(msg: string);
    }
    class AlreadyExists extends Error {
      constructor(msg: string);
    }
    class InvalidData extends Error {
      constructor(msg: string);
    }
    class TimedOut extends Error {
      constructor(msg: string);
    }
    class Interrupted extends Error {
      constructor(msg: string);
    }
    class WriteZero extends Error {
      constructor(msg: string);
    }
    class Other extends Error {
      constructor(msg: string);
    }
    class UnexpectedEof extends Error {
      constructor(msg: string);
    }
    class BadResource extends Error {
      constructor(msg: string);
    }
    class Http extends Error {
      constructor(msg: string);
    }
  }
  /* eslint-enable @typescript-eslint/no-unused-vars */

  /** UNSTABLE: potentially want names to overlap more with browser.
   *
   * Permissions as granted by the caller
   * See: https://w3c.github.io/permissions/#permission-registry
   */
  export type PermissionName =
    | "run"
    | "read"
    | "write"
    | "net"
    | "env"
    | "plugin"
    | "hrtime";
  /** https://w3c.github.io/permissions/#status-of-a-permission */
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
  /** See: https://w3c.github.io/permissions/#permission-descriptor */
  type PermissionDescriptor =
    | RunPermissionDescriptor
    | ReadWritePermissionDescriptor
    | NetPermissionDescriptor
    | EnvPermissionDescriptor
    | PluginPermissionDescriptor
    | HrtimePermissionDescriptor;

  export class Permissions {
    /** Queries the permission.
     *       const status = await Deno.permissions.query({ name: "read", path: "/etc" });
     *       if (status.state === "granted") {
     *         data = await Deno.readFile("/etc/passwd");
     *       }
     */
    query(d: PermissionDescriptor): Promise<PermissionStatus>;
    /** Revokes the permission.
     *       const status = await Deno.permissions.revoke({ name: "run" });
     *       assert(status.state !== "granted")
     */
    revoke(d: PermissionDescriptor): Promise<PermissionStatus>;
    /** Requests the permission.
     *       const status = await Deno.permissions.request({ name: "env" });
     *       if (status.state === "granted") {
     *         console.log(Deno.homeDir());
     *       } else {
     *         console.log("'env' permission is denied.");
     *       }
     */
    request(desc: PermissionDescriptor): Promise<PermissionStatus>;
  }
  /** UNSTABLE: maybe move to navigator.permissions to match web API. */
  export const permissions: Permissions;

  /** https://w3c.github.io/permissions/#permissionstatus */
  export class PermissionStatus {
    state: PermissionState;
    constructor(state: PermissionState);
  }

  /** Truncates or extends the specified file synchronously, updating the size of
   * this file to become size.
   *
   *       Deno.truncateSync("hello.txt", 10);
   *
   * Requires allow-write.
   */
  export function truncateSync(name: string, len?: number): void;
  /**
   * Truncates or extends the specified file, updating the size of this file to
   * become size.
   *
   *       await Deno.truncate("hello.txt", 10);
   *
   * Requires allow-write.
   */
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

  /** UNSTABLE: New API, not yet vetted.
   *
   * Open and initalize a plugin.
   * Requires the `--allow-plugin` flag.
   *
   *        const plugin = Deno.openPlugin("./path/to/some/plugin.so");
   *        const some_op = plugin.ops.some_op;
   *        const response = some_op.dispatch(new Uint8Array([1,2,3,4]));
   *        console.log(`Response from plugin ${response}`);
   */
  export function openPlugin(filename: string): Plugin;

  export type Transport = "tcp" | "udp";

  export interface Addr {
    transport: Transport;
    hostname: string;
    port: number;
  }

  export interface UDPAddr {
    transport?: Transport;
    hostname?: string;
    port: number;
  }

  /** UNSTABLE: Maybe remove ShutdownMode entirely. */
  export enum ShutdownMode {
    // See http://man7.org/linux/man-pages/man2/shutdown.2.html
    // Corresponding to SHUT_RD, SHUT_WR, SHUT_RDWR
    Read = 0,
    Write,
    ReadWrite // TODO(ry) panics on ReadWrite.
  }

  /** UNSTABLE: Maybe should remove how parameter maybe remove ShutdownMode
   * entirely.
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

  /** UNSTABLE: new API
   * Waits for the next message to the passed rid and writes it on the passed buffer.
   * Returns the number of bytes written and the remote address.
   */
  export function recvfrom(rid: number, p: Uint8Array): Promise<[number, Addr]>;

  /** UNSTABLE: new API
   * A socket is a generic transport listener for message-oriented protocols
   */
  export interface UDPConn extends AsyncIterator<[Uint8Array, Addr]> {
    /** UNSTABLE: new API
     * Waits for and resolves to the next message to the `Socket`. */
    receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;

    /** UNSTABLE: new API
     * Sends a message to the target. */
    send(p: Uint8Array, addr: UDPAddr): Promise<void>;

    /** UNSTABLE: new API
     * Close closes the socket. Any pending message promises will be rejected
     * with errors.
     */
    close(): void;

    /** Return the address of the `Socket`. */
    addr: Addr;

    [Symbol.asyncIterator](): AsyncIterator<[Uint8Array, Addr]>;
  }

  /** A Listener is a generic network listener for stream-oriented protocols. */
  export interface Listener extends AsyncIterator<Conn> {
    /** Waits for and resolves to the next connection to the `Listener`. */
    accept(): Promise<Conn>;
    /** Close closes the listener. Any pending accept promises will be rejected
     * with errors.
     */
    close(): void;
    /** Return the address of the `Listener`. */
    addr: Addr;
    [Symbol.asyncIterator](): AsyncIterator<Conn>;
  }

  export interface Conn extends Reader, Writer, Closer {
    /**
     * The local address of the connection.
     */
    localAddr: Addr;
    /**
     * The remote address of the connection.
     */
    remoteAddr: Addr;
    /** The resource ID of the connection. */
    rid: number;
    /** Shuts down (`shutdown(2)`) the reading side of the TCP connection. Most
     * callers should just use `close()`.
     */
    closeRead(): void;
    /** Shuts down (`shutdown(2)`) the writing side of the TCP connection. Most
     * callers should just use `close()`.
     */
    closeWrite(): void;
  }

  export interface ListenOptions {
    port: number;
    hostname?: string;
    transport?: Transport;
  }

  /** UNSTABLE: new API
   *
   * Listen announces on the local transport address.
   *
   * Requires the allow-net permission.
   *
   * @param options
   * @param options.port The port to connect to. (Required.)
   * @param options.hostname A literal IP address or host name that can be
   *   resolved to an IP address. If not specified, defaults to 0.0.0.0
   * @param options.transport Defaults to "tcp". Later we plan to add "tcp4",
   *   "tcp6", "udp", "udp4", "udp6", "ip", "ip4", "ip6", "unix", "unixgram" and
   *   "unixpacket".
   *
   * Examples:
   *
   *     listen({ port: 80 })
   *     listen({ hostname: "192.0.2.1", port: 80 })
   *     listen({ hostname: "[2001:db8::1]", port: 80 });
   *     listen({ hostname: "golang.org", port: 80, transport: "tcp" })
   */
  export function listen(
    options: ListenOptions & { transport?: "tcp" }
  ): Listener;
  export function listen(
    options: ListenOptions & { transport: "udp" }
  ): UDPConn;
  export function listen(options: ListenOptions): Listener | UDPConn;

  export interface ListenTLSOptions {
    port: number;
    hostname?: string;
    transport?: Transport;
    certFile: string;
    keyFile: string;
  }

  /** Listen announces on the local transport address over TLS (transport layer security).
   *
   * @param options
   * @param options.port The port to connect to. (Required.)
   * @param options.hostname A literal IP address or host name that can be
   *   resolved to an IP address. If not specified, defaults to 0.0.0.0
   * @param options.certFile Server certificate file
   * @param options.keyFile Server public key file
   *
   * Examples:
   *
   *     Deno.listenTLS({ port: 443, certFile: "./my_server.crt", keyFile: "./my_server.key" })
   */
  export function listenTLS(options: ListenTLSOptions): Listener;

  export interface ConnectOptions {
    port: number;
    hostname?: string;
    transport?: Transport;
  }

  /**
   * Connects to the address on the named transport.
   *
   * @param options
   * @param options.port The port to connect to. (Required.)
   * @param options.hostname A literal IP address or host name that can be
   *   resolved to an IP address. If not specified, defaults to 127.0.0.1
   * @param options.transport Defaults to "tcp". Later we plan to add "tcp4",
   *   "tcp6", "udp", "udp4", "udp6", "ip", "ip4", "ip6", "unix", "unixgram" and
   *   "unixpacket".
   *
   * Examples:
   *
   *     connect({ port: 80 })
   *     connect({ hostname: "192.0.2.1", port: 80 })
   *     connect({ hostname: "[2001:db8::1]", port: 80 });
   *     connect({ hostname: "golang.org", port: 80, transport: "tcp" })
   */
  export function connect(options: ConnectOptions): Promise<Conn>;

  export interface ConnectTLSOptions {
    port: number;
    hostname?: string;
    certFile?: string;
  }

  /**
   * Establishes a secure connection over TLS (transport layer security).
   */
  export function connectTLS(options: ConnectTLSOptions): Promise<Conn>;

  /** UNSTABLE: not sure if broken or not */
  export interface Metrics {
    opsDispatched: number;
    opsCompleted: number;
    bytesSentControl: number;
    bytesSentData: number;
    bytesReceived: number;
  }

  /** UNSTABLE: potentially broken.
   *
   * Receive metrics from the privileged side of Deno.
   *
   *      > console.table(Deno.metrics())
   *      ┌──────────────────┬────────┐
   *      │     (index)      │ Values │
   *      ├──────────────────┼────────┤
   *      │  opsDispatched   │   9    │
   *      │   opsCompleted   │   9    │
   *      │ bytesSentControl │  504   │
   *      │  bytesSentData   │   0    │
   *      │  bytesReceived   │  856   │
   *      └──────────────────┴────────┘
   */
  export function metrics(): Metrics;

  /** UNSTABLE: reconsider representation. */
  interface ResourceMap {
    [rid: number]: string;
  }

  /** UNSTABLE: reconsider return type.
   *
   * Returns a map of open _file like_ resource ids along with their string
   * representation.
   */
  export function resources(): ResourceMap;

  /** UNSTABLE: new API. Needs docs. */
  export interface FsEvent {
    kind: "any" | "access" | "create" | "modify" | "remove";
    paths: string[];
  }

  /** UNSTABLE: new API. Needs docs.
   *
   * recursive option is true by default.
   */
  export function fsEvents(
    paths: string | string[],
    options?: { recursive: boolean }
  ): AsyncIterableIterator<FsEvent>;

  /** How to handle subprocess stdio.
   *
   * "inherit" The default if unspecified. The child inherits from the
   * corresponding parent descriptor.
   *
   * "piped"  A new pipe should be arranged to connect the parent and child
   * subprocesses.
   *
   * "null" This stream will be ignored. This is the equivalent of attaching the
   * stream to /dev/null.
   */
  type ProcessStdio = "inherit" | "piped" | "null";

  /** UNSTABLE: the signo parameter maybe shouldn't be number.
   *
   * Send a signal to process under given PID. Unix only at this moment.
   * If pid is negative, the signal will be sent to the process group identified
   * by -pid.
   *
   * Requires the `--allow-run` flag.
   *
   * Currently no-op on Windows. TODO Should throw on windows instead of silently succeeding.
   */
  export function kill(pid: number, signo: number): void;

  /** UNSTABLE: There are some issues to work out with respect to when and how
   * the process should be closed.
   */
  export class Process {
    readonly rid: number;
    readonly pid: number;
    readonly stdin?: WriteCloser;
    readonly stdout?: ReadCloser;
    readonly stderr?: ReadCloser;
    status(): Promise<ProcessStatus>;
    /** Buffer the stdout and return it as Uint8Array after EOF.
     * You must set stdout to "piped" when creating the process.
     * This calls close() on stdout after its done.
     */
    output(): Promise<Uint8Array>;
    /** Buffer the stderr and return it as Uint8Array after EOF.
     * You must set stderr to "piped" when creating the process.
     * This calls close() on stderr after its done.
     */
    stderrOutput(): Promise<Uint8Array>;
    close(): void;
    kill(signo: number): void;
  }

  export interface ProcessStatus {
    success: boolean;
    code?: number;
    signal?: number;
  }

  /** UNSTABLE:  Maybe rename args to argv to differentiate from Deno.args Note
   * the first element needs to be a path to the binary.
   */
  export interface RunOptions {
    args: string[];
    cwd?: string;
    env?: {
      [key: string]: string;
    };
    stdout?: ProcessStdio | number;
    stderr?: ProcessStdio | number;
    stdin?: ProcessStdio | number;
  }

  /**
   * Spawns new subprocess.
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
   */
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
    SIGSYS = 31
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
    SIGUSR2 = 31
  }
  /** UNSTABLE: make platform independent.
   *
   * Signals numbers. This is platform dependent.
   */
  export const Signal: typeof MacOSSignal | typeof LinuxSignal;

  /** UNSTABLE: rename to InspectOptions */
  type ConsoleOptions = Partial<{
    showHidden: boolean;
    depth: number;
    colors: boolean;
    indentLevel: number;
  }>;

  /** UNSTABLE: ConsoleOptions rename to InspectOptions. Also the exact form of
   * string output subject to change.
   *
   * `inspect()` converts input into string that has the same format
   * as printed by `console.log(...)`;
   */
  export function inspect(value: unknown, options?: ConsoleOptions): string;

  export type OperatingSystem = "mac" | "win" | "linux";

  export type Arch = "x64" | "arm64";

  /** Build related information */
  interface BuildInfo {
    /** The CPU architecture. */
    arch: Arch;
    /** The operating system. */
    os: OperatingSystem;
  }

  export const build: BuildInfo;

  interface Version {
    deno: string;
    v8: string;
    typescript: string;
  }
  export const version: Version;

  /** The log category for a diagnostic message */
  export enum DiagnosticCategory {
    Log = 0,
    Debug = 1,
    Info = 2,
    Error = 3,
    Warning = 4,
    Suggestion = 5
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

    /** Information related to the diagnostic.  This is present when there is a
     * suggestion or other additional diagnostic information */
    relatedInformation?: DiagnosticItem[];

    /** The text of the source line related to the diagnostic */
    sourceLine?: string;

    /** The line number that is related to the diagnostic */
    lineNumber?: number;

    /** The name of the script resource related to the diagnostic */
    scriptResourceName?: string;

    /** The start position related to the diagnostic */
    startPosition?: number;

    /** The end position related to the diagnostic */
    endPosition?: number;

    /** The category of the diagnostic */
    category: DiagnosticCategory;

    /** A number identifier */
    code: number;

    /** The the start column of the sourceLine related to the diagnostic */
    startColumn?: number;

    /** The end column of the sourceLine related to the diagnostic */
    endColumn?: number;
  }

  export interface Diagnostic {
    /** An array of diagnostic items. */
    items: DiagnosticItem[];
  }

  /** UNSTABLE: new API, yet to be vetted.
   *
   * A specific subset TypeScript compiler options that can be supported by
   * the Deno TypeScript compiler.
   */
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
     * destructuring when targeting ES5 or ES3.  Defaults to `false`. */
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

    /** List of library files to be included in the compilation.  If omitted,
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

    /** Specify the module format for the emitted code.  Defaults to
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
     * `Deno.compile` and only changes the emitted file names.  Defaults to
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

    /** List of names of type definitions to include. Defaults to `undefined`. */
    types?: string[];
  }

  /** UNSTABLE: new API, yet to be vetted.
   *
   * The results of a transpile only command, where the `source` contains the
   * emitted source, and `map` optionally contains the source map.
   */
  export interface TranspileOnlyResult {
    source: string;
    map?: string;
  }

  /** UNSTABLE: new API, yet to be vetted.
   *
   * Takes a set of TypeScript sources and resolves with a map where the key was
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
   *                to transpile.  The filename is only used in the transpile and
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

  /** UNSTABLE: new API, yet to be vetted.
   *
   * Takes a root module name, any optionally a record set of sources. Resolves
   * with a compiled set of modules.  If just a root name is provided, the modules
   * will be resolved as if the root module had been passed on the command line.
   *
   * If sources are passed, all modules will be resolved out of this object, where
   * the key is the module name and the value is the content.  The extension of
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
   *                 "starting point".  If no `sources` is specified, Deno will
   *                 resolve the module externally as if the `rootName` had been
   *                 specified on the command line.
   * @param sources An optional key/value map of sources to be used when resolving
   *                modules, where the key is the module name, and the value is
   *                the source content.  The extension of the key will determine
   *                the media type of the file when processing.  If supplied,
   *                Deno will not attempt to resolve any modules externally.
   * @param options An optional object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   */
  export function compile(
    rootName: string,
    sources?: Record<string, string>,
    options?: CompilerOptions
  ): Promise<[DiagnosticItem[] | undefined, Record<string, string>]>;

  /** UNSTABLE: new API, yet to be vetted.
   *
   * Takes a root module name, and optionally a record set of sources. Resolves
   * with a single JavaScript string that is like the output of a `deno bundle`
   * command.  If just a root name is provided, the modules will be resolved as if
   * the root module had been passed on the command line.
   *
   * If sources are passed, all modules will be resolved out of this object, where
   * the key is the module name and the value is the content. The extension of the
   * module name will be used to determine the media type of the module.
   *
   *      const [ maybeDiagnostics1, output1 ] = await Deno.bundle("foo.ts");
   *
   *      const [ maybeDiagnostics2, output2 ] = await Deno.bundle("/foo.ts", {
   *        "/foo.ts": `export * from "./bar.ts";`,
   *        "/bar.ts": `export const bar = "bar";`
   *      });
   *
   * @param rootName The root name of the module which will be used as the
   *                 "starting point".  If no `sources` is specified, Deno will
   *                 resolve the module externally as if the `rootName` had been
   *                 specified on the command line.
   * @param sources An optional key/value map of sources to be used when resolving
   *                modules, where the key is the module name, and the value is
   *                the source content.  The extension of the key will determine
   *                the media type of the file when processing.  If supplied,
   *                Deno will not attempt to resolve any modules externally.
   * @param options An optional object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   */
  export function bundle(
    rootName: string,
    sources?: Record<string, string>,
    options?: CompilerOptions
  ): Promise<[DiagnosticItem[] | undefined, string]>;

  /** Returns the script arguments to the program. If for example we run a program
   *
   *   deno --allow-read https://deno.land/std/examples/cat.ts /etc/passwd
   *
   * Then Deno.args will contain just
   *
   *   [ "/etc/passwd" ]
   */
  export const args: string[];

  /** UNSTABLE new API.
   *
   * SignalStream represents the stream of signals, implements both
   * AsyncIterator and PromiseLike
   */
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

  /** UNSTABLE new API.
   *
   * Returns the stream of the given signal number. You can use it as an async
   * iterator.
   *
   *     for await (const _ of Deno.signal(Deno.Signal.SIGTERM)) {
   *       console.log("got SIGTERM!");
   *     }
   *
   * You can also use it as a promise. In this case you can only receive the
   * first one.
   *
   *     await Deno.signal(Deno.Signal.SIGTERM);
   *     console.log("SIGTERM received!")
   *
   * If you want to stop receiving the signals, you can use .dispose() method
   * of the signal stream object.
   *
   *     const sig = Deno.signal(Deno.Signal.SIGTERM);
   *     setTimeout(() => { sig.dispose(); }, 5000);
   *     for await (const _ of sig) {
   *       console.log("SIGTERM!")
   *     }
   *
   * The above for-await loop exits after 5 seconds when sig.dispose() is called.
   */
  export function signal(signo: number): SignalStream;

  /** UNSTABLE new API. */
  export const signals: {
    /** Returns the stream of SIGALRM signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGALRM). */
    alarm: () => SignalStream;
    /** Returns the stream of SIGCHLD signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGCHLD). */
    child: () => SignalStream;
    /** Returns the stream of SIGHUP signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGHUP). */
    hungup: () => SignalStream;
    /** Returns the stream of SIGINT signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGINT). */
    interrupt: () => SignalStream;
    /** Returns the stream of SIGIO signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGIO). */
    io: () => SignalStream;
    /** Returns the stream of SIGPIPE signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGPIPE). */
    pipe: () => SignalStream;
    /** Returns the stream of SIGQUIT signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGQUIT). */
    quit: () => SignalStream;
    /** Returns the stream of SIGTERM signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGTERM). */
    terminate: () => SignalStream;
    /** Returns the stream of SIGUSR1 signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGUSR1). */
    userDefined1: () => SignalStream;
    /** Returns the stream of SIGUSR2 signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGUSR2). */
    userDefined2: () => SignalStream;
    /** Returns the stream of SIGWINCH signals.
     * This method is the shorthand for Deno.signal(Deno.Signal.SIGWINCH). */
    windowChange: () => SignalStream;
  };

  /** UNSTABLE: new API. Maybe move EOF here.
   *
   * Special Deno related symbols.
   */
  export const symbols: {
    /** Symbol to access exposed internal Deno API */
    readonly internal: unique symbol;
    /** A symbol which can be used as a key for a custom method which will be called
     * when `Deno.inspect()` is called, or when the object is logged to the console.
     */
    readonly customInspect: unique symbol;
    // TODO(ry) move EOF here?
  };
}
