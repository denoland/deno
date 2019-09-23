// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/* eslint-disable @typescript-eslint/no-explicit-any */
/* eslint-disable @typescript-eslint/no-empty-interface */

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace Deno {
  // @url js/os.d.ts

  /** The current process id of the runtime. */
  export let pid: number;
  /** Reflects the NO_COLOR environment variable: https://no-color.org/ */
  export let noColor: boolean;
  /** Check if running in terminal.
   *
   *       console.log(Deno.isTTY().stdout);
   */
  export function isTTY(): {
    stdin: boolean;
    stdout: boolean;
    stderr: boolean;
  };
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
  /**
   * Returns the current user's home directory.
   * Requires the `--allow-env` flag.
   */
  export function homeDir(): string;
  /**
   * Returns the path to the current deno executable.
   * Requires the `--allow-env` flag.
   */
  export function execPath(): string;

  // @url js/dir.d.ts

  /**
   * `cwd()` Return a string representing the current working directory.
   * If the current directory can be reached via multiple paths
   * (due to symbolic links), `cwd()` may return
   * any one of them.
   * throws `NotFound` exception if directory not available
   */
  export function cwd(): string;
  /**
   * `chdir()` Change the current working directory to path.
   * throws `NotFound` exception if directory not available
   */
  export function chdir(directory: string): void;

  // @url js/io.d.ts

  export const EOF: null;
  export type EOF = null;
  export enum SeekMode {
    SEEK_START = 0,
    SEEK_CURRENT = 1,
    SEEK_END = 2
  }
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
   *       const file = Deno.openSync("/foo/bar.txt");
   */
  export function openSync(filename: string, mode?: OpenMode): File;
  /** Open a file and return an instance of the `File` object.
   *
   *       (async () => {
   *         const file = await Deno.open("/foo/bar.txt");
   *       })();
   */
  export function open(filename: string, mode?: OpenMode): Promise<File>;
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
  /** Read from a file ID into an array buffer.
   *
   * Resolves with the `number | EOF` for the operation.
   *
   *       (async () => {
   *         const file = await Deno.open("/foo/bar.txt");
   *         const buf = new Uint8Array(100);
   *         const nread = await Deno.read(file.rid, buf);
   *         const text = new TextDecoder().decode(buf);
   *       })();
   */
  export function read(rid: number, p: Uint8Array): Promise<number | EOF>;
  /** Write synchronously to the file ID the contents of the array buffer.
   *
   * Resolves with the number of bytes written.
   *
   *       const encoder = new TextEncoder();
   *       const data = encoder.encode("Hello world\n");
   *       const file = Deno.openSync("/foo/bar.txt");
   *       Deno.writeSync(file.rid, data);
   */
  export function writeSync(rid: number, p: Uint8Array): number;
  /** Write to the file ID the contents of the array buffer.
   *
   * Resolves with the number of bytes written.
   *
   *      (async () => {
   *        const encoder = new TextEncoder();
   *        const data = encoder.encode("Hello world\n");
   *        const file = await Deno.open("/foo/bar.txt");
   *        await Deno.write(file.rid, data);
   *      })();
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
  /** Close the file ID. */
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
  export type OpenMode =
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
     * If the buffer can't grow it will throw with ErrTooLarge.
     */
    private _grow;
    /** grow() grows the buffer's capacity, if necessary, to guarantee space for
     * another n bytes. After grow(n), at least n bytes can be written to the
     * buffer without another allocation. If n is negative, grow() will panic. If
     * the buffer can't grow it will throw ErrTooLarge.
     * Based on https://golang.org/pkg/bytes/#Buffer.Grow
     */
    grow(n: number): void;
    /** readFrom() reads data from r until EOF and appends it to the buffer,
     * growing the buffer as needed. It returns the number of bytes read. If the
     * buffer becomes too large, readFrom will panic with ErrTooLarge.
     * Based on https://golang.org/pkg/bytes/#Buffer.ReadFrom
     */
    readFrom(r: Reader): Promise<number>;
    /** Sync version of `readFrom`
     */
    readFromSync(r: SyncReader): number;
  }
  /** Read `r` until EOF and return the content as `Uint8Array`.
   */
  export function readAll(r: Reader): Promise<Uint8Array>;
  /** Read synchronously `r` until EOF and return the content as `Uint8Array`.
   */
  export function readAllSync(r: SyncReader): Uint8Array;
  /** Write all the content of `arr` to `w`.
   */
  export function writeAll(w: Writer, arr: Uint8Array): Promise<void>;
  /** Write synchronously all the content of `arr` to `w`.
   */
  export function writeAllSync(w: SyncWriter, arr: Uint8Array): void;

  // @url js/mkdir.d.ts

  /** Creates a new directory with the specified path synchronously.
   * If `recursive` is set to true, nested directories will be created (also known
   * as "mkdir -p").
   * `mode` sets permission bits (before umask) on UNIX and does nothing on
   * Windows.
   *
   *       Deno.mkdirSync("new_dir");
   *       Deno.mkdirSync("nested/directories", true);
   */
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
   *       await Deno.mkdir("nested/directories", true);
   */
  export function mkdir(
    path: string,
    recursive?: boolean,
    mode?: number
  ): Promise<void>;

  // @url js/make_temp_dir.d.ts

  export interface MakeTempDirOptions {
    dir?: string;
    prefix?: string;
    suffix?: string;
  }
  /** makeTempDirSync is the synchronous version of `makeTempDir`.
   *
   *       const tempDirName0 = Deno.makeTempDirSync();
   *       const tempDirName1 = Deno.makeTempDirSync({ prefix: 'my_temp' });
   */
  export function makeTempDirSync(options?: MakeTempDirOptions): string;
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
   */
  export function makeTempDir(options?: MakeTempDirOptions): Promise<string>;

  // @url js/chmod.d.ts

  /** Changes the permission of a specific file/directory of specified path
   * synchronously.
   *
   *       Deno.chmodSync("/path/to/file", 0o666);
   */
  export function chmodSync(path: string, mode: number): void;
  /** Changes the permission of a specific file/directory of specified path.
   *
   *       await Deno.chmod("/path/to/file", 0o666);
   */
  export function chmod(path: string, mode: number): Promise<void>;

  // @url js/chown.d.ts

  /**
   * Change owner of a regular file or directory synchronously. Unix only at the moment.
   * @param path path to the file
   * @param uid user id of the new owner
   * @param gid group id of the new owner
   */
  export function chownSync(path: string, uid: number, gid: number): void;
  /**
   * Change owner of a regular file or directory asynchronously. Unix only at the moment.
   * @param path path to the file
   * @param uid user id of the new owner
   * @param gid group id of the new owner
   */
  export function chown(path: string, uid: number, gid: number): Promise<void>;

  // @url js/utime.d.ts

  /** Synchronously changes the access and modification times of a file system
   * object referenced by `filename`. Given times are either in seconds
   * (Unix epoch time) or as `Date` objects.
   *
   *       Deno.utimeSync("myfile.txt", 1556495550, new Date());
   */
  export function utimeSync(
    filename: string,
    atime: number | Date,
    mtime: number | Date
  ): void;
  /** Changes the access and modification times of a file system object
   * referenced by `filename`. Given times are either in seconds
   * (Unix epoch time) or as `Date` objects.
   *
   *       await Deno.utime("myfile.txt", 1556495550, new Date());
   */
  export function utime(
    filename: string,
    atime: number | Date,
    mtime: number | Date
  ): Promise<void>;

  // @url js/remove.d.ts

  export interface RemoveOption {
    recursive?: boolean;
  }
  /** Removes the named file or directory synchronously. Would throw
   * error if permission denied, not found, or directory not empty if `recursive`
   * set to false.
   * `recursive` is set to false by default.
   *
   *       Deno.removeSync("/path/to/dir/or/file", {recursive: false});
   */
  export function removeSync(path: string, options?: RemoveOption): void;
  /** Removes the named file or directory. Would throw error if
   * permission denied, not found, or directory not empty if `recursive` set
   * to false.
   * `recursive` is set to false by default.
   *
   *       await Deno.remove("/path/to/dir/or/file", {recursive: false});
   */
  export function remove(path: string, options?: RemoveOption): Promise<void>;

  // @url js/rename.d.ts

  /** Synchronously renames (moves) `oldpath` to `newpath`. If `newpath` already
   * exists and is not a directory, `renameSync()` replaces it. OS-specific
   * restrictions may apply when `oldpath` and `newpath` are in different
   * directories.
   *
   *       Deno.renameSync("old/path", "new/path");
   */
  export function renameSync(oldpath: string, newpath: string): void;
  /** Renames (moves) `oldpath` to `newpath`. If `newpath` already exists and is
   * not a directory, `rename()` replaces it. OS-specific restrictions may apply
   * when `oldpath` and `newpath` are in different directories.
   *
   *       await Deno.rename("old/path", "new/path");
   */
  export function rename(oldpath: string, newpath: string): Promise<void>;

  // @url js/read_file.d.ts

  /** Read the entire contents of a file synchronously.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = Deno.readFileSync("hello.txt");
   *       console.log(decoder.decode(data));
   */
  export function readFileSync(filename: string): Uint8Array;
  /** Read the entire contents of a file.
   *
   *       const decoder = new TextDecoder("utf-8");
   *       const data = await Deno.readFile("hello.txt");
   *       console.log(decoder.decode(data));
   */
  export function readFile(filename: string): Promise<Uint8Array>;

  // @url js/file_info.d.ts

  /** A FileInfo describes a file and is returned by `stat`, `lstat`,
   * `statSync`, `lstatSync`.
   */
  export interface FileInfo {
    /** The size of the file, in bytes. */
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
    /** The underlying raw st_mode bits that contain the standard Unix permissions
     * for this file/directory. TODO Match behavior with Go on windows for mode.
     */
    mode: number | null;
    /** The file or directory name. */
    name: string | null;
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

  // @url js/read_dir.d.ts

  /** Reads the directory given by path and returns a list of file info
   * synchronously.
   *
   *       const files = Deno.readDirSync("/");
   */
  export function readDirSync(path: string): FileInfo[];
  /** Reads the directory given by path and returns a list of file info.
   *
   *       const files = await Deno.readDir("/");
   */
  export function readDir(path: string): Promise<FileInfo[]>;

  // @url js/copy_file.d.ts

  /** Copies the contents of a file to another by name synchronously.
   * Creates a new file if target does not exists, and if target exists,
   * overwrites original content of the target file.
   *
   * It would also copy the permission of the original file
   * to the destination.
   *
   *       Deno.copyFileSync("from.txt", "to.txt");
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
   */
  export function copyFile(from: string, to: string): Promise<void>;

  // @url js/read_link.d.ts

  /** Returns the destination of the named symbolic link synchronously.
   *
   *       const targetPath = Deno.readlinkSync("symlink/path");
   */
  export function readlinkSync(name: string): string;
  /** Returns the destination of the named symbolic link.
   *
   *       const targetPath = await Deno.readlink("symlink/path");
   */
  export function readlink(name: string): Promise<string>;

  // @url js/stat.d.ts

  interface StatResponse {
    isFile: boolean;
    isSymlink: boolean;
    len: number;
    modified: number;
    accessed: number;
    created: number;
    mode: number;
    hasMode: boolean;
    name: string | null;
  }
  /** Queries the file system for information on the path provided. If the given
   * path is a symlink information about the symlink will be returned.
   *
   *       const fileInfo = await Deno.lstat("hello.txt");
   *       assert(fileInfo.isFile());
   */
  export function lstat(filename: string): Promise<FileInfo>;
  /** Queries the file system for information on the path provided synchronously.
   * If the given path is a symlink information about the symlink will be
   * returned.
   *
   *       const fileInfo = Deno.lstatSync("hello.txt");
   *       assert(fileInfo.isFile());
   */
  export function lstatSync(filename: string): FileInfo;
  /** Queries the file system for information on the path provided. `stat` Will
   * always follow symlinks.
   *
   *       const fileInfo = await Deno.stat("hello.txt");
   *       assert(fileInfo.isFile());
   */
  export function stat(filename: string): Promise<FileInfo>;
  /** Queries the file system for information on the path provided synchronously.
   * `statSync` Will always follow symlinks.
   *
   *       const fileInfo = Deno.statSync("hello.txt");
   *       assert(fileInfo.isFile());
   */
  export function statSync(filename: string): FileInfo;

  // @url js/link.d.ts

  /** Synchronously creates `newname` as a hard link to `oldname`.
   *
   *       Deno.linkSync("old/name", "new/name");
   */
  export function linkSync(oldname: string, newname: string): void;
  /** Creates `newname` as a hard link to `oldname`.
   *
   *       await Deno.link("old/name", "new/name");
   */
  export function link(oldname: string, newname: string): Promise<void>;

  // @url js/symlink.d.ts

  /** Synchronously creates `newname` as a symbolic link to `oldname`. The type
   * argument can be set to `dir` or `file` and is only available on Windows
   * (ignored on other platforms).
   *
   *       Deno.symlinkSync("old/name", "new/name");
   */
  export function symlinkSync(
    oldname: string,
    newname: string,
    type?: string
  ): void;
  /** Creates `newname` as a symbolic link to `oldname`. The type argument can be
   * set to `dir` or `file` and is only available on Windows (ignored on other
   * platforms).
   *
   *       await Deno.symlink("old/name", "new/name");
   */
  export function symlink(
    oldname: string,
    newname: string,
    type?: string
  ): Promise<void>;

  // @url js/write_file.d.ts

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
   */
  export function writeFile(
    filename: string,
    data: Uint8Array,
    options?: WriteFileOptions
  ): Promise<void>;

  // @url js/error_stack.d.ts

  interface Location {
    /** The full url for the module, e.g. `file://some/file.ts` or
     * `https://some/file.ts`. */
    filename: string;
    /** The line number in the file.  It is assumed to be 1-indexed. */
    line: number;
    /** The column number in the file.  It is assumed to be 1-indexed. */
    column: number;
  }
  /** Given a current location in a module, lookup the source location and
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

  // @url js/errors.d.ts

  /** A Deno specific error.  The `kind` property is set to a specific error code
   * which can be used to in application logic.
   *
   *       try {
   *         somethingThatMightThrow();
   *       } catch (e) {
   *         if (
   *           e instanceof Deno.DenoError &&
   *           e.kind === Deno.ErrorKind.Overflow
   *         ) {
   *           console.error("Overflow error!");
   *         }
   *       }
   *
   */
  export class DenoError<T extends ErrorKind> extends Error {
    readonly kind: T;
    constructor(kind: T, msg: string);
  }
  export enum ErrorKind {
    NoError = 0,
    NotFound = 1,
    PermissionDenied = 2,
    ConnectionRefused = 3,
    ConnectionReset = 4,
    ConnectionAborted = 5,
    NotConnected = 6,
    AddrInUse = 7,
    AddrNotAvailable = 8,
    BrokenPipe = 9,
    AlreadyExists = 10,
    WouldBlock = 11,
    InvalidInput = 12,
    InvalidData = 13,
    TimedOut = 14,
    Interrupted = 15,
    WriteZero = 16,
    Other = 17,
    UnexpectedEof = 18,
    BadResource = 19,
    CommandFailed = 20,
    EmptyHost = 21,
    IdnaError = 22,
    InvalidPort = 23,
    InvalidIpv4Address = 24,
    InvalidIpv6Address = 25,
    InvalidDomainCharacter = 26,
    RelativeUrlWithoutBase = 27,
    RelativeUrlWithCannotBeABaseBase = 28,
    SetHostOnCannotBeABaseUrl = 29,
    Overflow = 30,
    HttpUser = 31,
    HttpClosed = 32,
    HttpCanceled = 33,
    HttpParse = 34,
    HttpOther = 35,
    TooLarge = 36,
    InvalidUri = 37,
    InvalidSeekMode = 38,
    OpNotAvailable = 39,
    WorkerInitFailed = 40,
    UnixError = 41,
    NoAsyncSupport = 42,
    NoSyncSupport = 43,
    ImportMapError = 44,
    InvalidPath = 45,
    ImportPrefixMissing = 46,
    UnsupportedFetchScheme = 47,
    TooManyRedirects = 48,
    Diagnostic = 49,
    JSError = 50
  }

  // @url js/permissions.d.ts

  /** Permissions as granted by the caller */
  export interface Permissions {
    read: boolean;
    write: boolean;
    net: boolean;
    env: boolean;
    run: boolean;
    hrtime: boolean;
  }
  export type Permission = keyof Permissions;
  /** Inspect granted permissions for the current program.
   *
   *       if (Deno.permissions().read) {
   *         const file = await Deno.readFile("example.test");
   *         // ...
   *       }
   */
  export function permissions(): Permissions;
  /** Revoke a permission. When the permission was already revoked nothing changes
   *
   *       if (Deno.permissions().read) {
   *         const file = await Deno.readFile("example.test");
   *         Deno.revokePermission('read');
   *       }
   *       Deno.readFile("example.test"); // -> error or permission prompt
   */
  export function revokePermission(permission: Permission): void;

  // @url js/truncate.d.ts

  /** Truncates or extends the specified file synchronously, updating the size of
   * this file to become size.
   *
   *       Deno.truncateSync("hello.txt", 10);
   */
  export function truncateSync(name: string, len?: number): void;
  /**
   * Truncates or extends the specified file, updating the size of this file to
   * become size.
   *
   *       await Deno.truncate("hello.txt", 10);
   */
  export function truncate(name: string, len?: number): Promise<void>;

  // @url js/net.d.ts

  type Transport = "tcp";
  interface Addr {
    transport: Transport;
    address: string;
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
    addr(): Addr;
    [Symbol.asyncIterator](): AsyncIterator<Conn>;
  }
  export interface Conn extends Reader, Writer, Closer {
    /** The local address of the connection. */
    localAddr: string;
    /** The remote address of the connection. */
    remoteAddr: string;
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

  /** Listen announces on the local transport address.
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
  export function listen(options: ListenOptions): Listener;

  export interface DialOptions {
    port: number;
    hostname?: string;
    transport?: Transport;
  }

  /** Dial connects to the address on the named transport.
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
   *     dial({ port: 80 })
   *     dial({ hostname: "192.0.2.1", port: 80 })
   *     dial({ hostname: "[2001:db8::1]", port: 80 });
   *     dial({ hostname: "golang.org", port: 80, transport: "tcp" })
   */
  export function dial(options: DialOptions): Promise<Conn>;

  export interface DialTLSOptions {
    port: number;
    hostname?: string;
  }

  /**
   * dialTLS establishes a secure connection over TLS (transport layer security).
   */
  export function dialTLS(options: DialTLSOptions): Promise<Conn>;

  // @url js/metrics.d.ts
  export interface Metrics {
    opsDispatched: number;
    opsCompleted: number;
    bytesSentControl: number;
    bytesSentData: number;
    bytesReceived: number;
  }
  /** Receive metrics from the privileged side of Deno.
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

  // @url js/resources.d.ts

  interface ResourceMap {
    [rid: number]: string;
  }
  /** Returns a map of open _file like_ resource ids along with their string
   * representation.
   */
  export function resources(): ResourceMap;

  // @url js/process.d.ts

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
  /** Send a signal to process under given PID. Unix only at this moment.
   * If pid is negative, the signal will be sent to the process group identified
   * by -pid.
   * Requires the `--allow-run` flag.
   */
  export function kill(pid: number, signo: number): void;
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
  /** Signals numbers. This is platform dependent.
   */
  export const Signal: typeof MacOSSignal | typeof LinuxSignal;
  export {};

  // @url js/console.d.ts

  type ConsoleOptions = Partial<{
    showHidden: boolean;
    depth: number;
    colors: boolean;
    indentLevel: number;
  }>;
  /** A symbol which can be used as a key for a custom method which will be called
   * when `Deno.inspect()` is called, or when the object is logged to the console.
   */
  export const customInspect: unique symbol;
  /**
   * `inspect()` converts input into string that has the same format
   * as printed by `console.log(...)`;
   */
  export function inspect(value: unknown, options?: ConsoleOptions): string;

  // @url js/build.d.ts

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

  // @url js/version.d.ts

  interface Version {
    deno: string;
    v8: string;
    typescript: string;
  }
  export const version: Version;
  export {};

  // @url js/deno.d.ts

  export const args: string[];
}

// @url js/globals.ts

declare interface Window {
  window: Window & typeof globalThis;
  atob: typeof textEncoding.atob;
  btoa: typeof textEncoding.btoa;
  fetch: typeof fetchTypes.fetch;
  clearTimeout: typeof timers.clearTimeout;
  clearInterval: typeof timers.clearInterval;
  console: consoleTypes.Console;
  setTimeout: typeof timers.setTimeout;
  setInterval: typeof timers.setInterval;
  location: domTypes.Location;
  onload: Function | undefined;
  crypto: Crypto;
  Blob: typeof blob.DenoBlob;
  File: domTypes.DomFileConstructor;
  CustomEvent: typeof customEvent.CustomEvent;
  Event: typeof event.Event;
  EventTarget: typeof eventTarget.EventTarget;
  URL: typeof url.URL;
  URLSearchParams: typeof urlSearchParams.URLSearchParams;
  Headers: domTypes.HeadersConstructor;
  FormData: domTypes.FormDataConstructor;
  TextEncoder: typeof textEncoding.TextEncoder;
  TextDecoder: typeof textEncoding.TextDecoder;
  Request: domTypes.RequestConstructor;
  Response: typeof fetchTypes.Response;
  performance: performanceUtil.Performance;
  onmessage: (e: { data: any }) => void;
  workerMain: typeof workers.workerMain;
  workerClose: typeof workers.workerClose;
  postMessage: typeof workers.postMessage;
  Worker: typeof workers.WorkerImpl;
  addEventListener: (
    type: string,
    callback: (event: domTypes.Event) => void | null,
    options?: boolean | domTypes.AddEventListenerOptions | undefined
  ) => void;
  dispatchEvent: (event: domTypes.Event) => boolean;
  removeEventListener: (
    type: string,
    callback: (event: domTypes.Event) => void | null,
    options?: boolean | domTypes.EventListenerOptions | undefined
  ) => void;
  queueMicrotask: (task: () => void) => void;
  Deno: typeof Deno;
}

declare const window: Window & typeof globalThis;
declare const atob: typeof textEncoding.atob;
declare const btoa: typeof textEncoding.btoa;
declare const fetch: typeof fetchTypes.fetch;
declare const clearTimeout: typeof timers.clearTimeout;
declare const clearInterval: typeof timers.clearInterval;
declare const console: consoleTypes.Console;
declare const setTimeout: typeof timers.setTimeout;
declare const setInterval: typeof timers.setInterval;
declare const location: domTypes.Location;
declare const onload: Function | undefined;
declare const crypto: Crypto;
declare const Blob: typeof blob.DenoBlob;
declare const File: domTypes.DomFileConstructor;
declare const CustomEventInit: typeof customEvent.CustomEventInit;
declare const CustomEvent: typeof customEvent.CustomEvent;
declare const EventInit: typeof event.EventInit;
declare const Event: typeof event.Event;
declare const EventListener: typeof eventTarget.EventListener;
declare const EventTarget: typeof eventTarget.EventTarget;
declare const URL: typeof url.URL;
declare const URLSearchParams: typeof urlSearchParams.URLSearchParams;
declare const Headers: domTypes.HeadersConstructor;
declare const FormData: domTypes.FormDataConstructor;
declare const TextEncoder: typeof textEncoding.TextEncoder;
declare const TextDecoder: typeof textEncoding.TextDecoder;
declare const Request: domTypes.RequestConstructor;
declare const Response: typeof fetchTypes.Response;
declare const performance: performanceUtil.Performance;
declare let onmessage: (e: { data: any }) => void;
declare const workerMain: typeof workers.workerMain;
declare const workerClose: typeof workers.workerClose;
declare const postMessage: typeof workers.postMessage;
declare const Worker: typeof workers.WorkerImpl;
declare const addEventListener: (
  type: string,
  callback: (event: domTypes.Event) => void | null,
  options?: boolean | domTypes.AddEventListenerOptions | undefined
) => void;
declare const dispatchEvent: (event: domTypes.Event) => boolean;
declare const removeEventListener: (
  type: string,
  callback: (event: domTypes.Event) => void | null,
  options?: boolean | domTypes.EventListenerOptions | undefined
) => void;

declare type Blob = domTypes.Blob;
declare type Body = domTypes.Body;
declare type File = domTypes.DomFile;
declare type CustomEventInit = domTypes.CustomEventInit;
declare type CustomEvent = domTypes.CustomEvent;
declare type EventInit = domTypes.EventInit;
declare type Event = domTypes.Event;
declare type EventListener = domTypes.EventListener;
declare type EventTarget = domTypes.EventTarget;
declare type URL = url.URL;
declare type URLSearchParams = domTypes.URLSearchParams;
declare type Headers = domTypes.Headers;
declare type FormData = domTypes.FormData;
declare type TextEncoder = textEncoding.TextEncoder;
declare type TextDecoder = textEncoding.TextDecoder;
declare type Request = domTypes.Request;
declare type Response = domTypes.Response;
declare type Worker = workers.Worker;

declare interface ImportMeta {
  url: string;
  main: boolean;
}

declare interface Crypto {
  readonly subtle: null;
  getRandomValues: <
    T extends
      | Int8Array
      | Uint8Array
      | Uint8ClampedArray
      | Int16Array
      | Uint16Array
      | Int32Array
      | Uint32Array
  >(
    typedArray: T
  ) => T;
}

declare namespace domTypes {
  // @url js/dom_types.d.ts

  export type BufferSource = ArrayBufferView | ArrayBuffer;
  export type HeadersInit =
    | Headers
    | Array<[string, string]>
    | Record<string, string>;
  export type URLSearchParamsInit =
    | string
    | string[][]
    | Record<string, string>;
  type BodyInit =
    | Blob
    | BufferSource
    | FormData
    | URLSearchParams
    | ReadableStream
    | string;
  export type RequestInfo = Request | string;
  type ReferrerPolicy =
    | ""
    | "no-referrer"
    | "no-referrer-when-downgrade"
    | "origin-only"
    | "origin-when-cross-origin"
    | "unsafe-url";
  export type BlobPart = BufferSource | Blob | string;
  export type FormDataEntryValue = DomFile | string;
  export interface DomIterable<K, V> {
    keys(): IterableIterator<K>;
    values(): IterableIterator<V>;
    entries(): IterableIterator<[K, V]>;
    [Symbol.iterator](): IterableIterator<[K, V]>;
    forEach(
      callback: (value: V, key: K, parent: this) => void,
      thisArg?: any
    ): void;
  }
  type EndingType = "transparent" | "native";
  export interface BlobPropertyBag {
    type?: string;
    ending?: EndingType;
  }
  interface AbortSignalEventMap {
    abort: ProgressEvent;
  }
  export enum NodeType {
    ELEMENT_NODE = 1,
    TEXT_NODE = 3,
    DOCUMENT_FRAGMENT_NODE = 11
  }
  export const eventTargetHost: unique symbol;
  export const eventTargetListeners: unique symbol;
  export const eventTargetMode: unique symbol;
  export const eventTargetNodeType: unique symbol;
  export interface EventTarget {
    [eventTargetHost]: EventTarget | null;
    [eventTargetListeners]: { [type in string]: EventListener[] };
    [eventTargetMode]: string;
    [eventTargetNodeType]: NodeType;
    addEventListener(
      type: string,
      callback: (event: Event) => void | null,
      options?: boolean | AddEventListenerOptions
    ): void;
    dispatchEvent(event: Event): boolean;
    removeEventListener(
      type: string,
      callback?: (event: Event) => void | null,
      options?: EventListenerOptions | boolean
    ): void;
  }
  export interface ProgressEventInit extends EventInit {
    lengthComputable?: boolean;
    loaded?: number;
    total?: number;
  }
  export interface URLSearchParams {
    /**
     * Appends a specified key/value pair as a new search parameter.
     */
    append(name: string, value: string): void;
    /**
     * Deletes the given search parameter, and its associated value,
     * from the list of all search parameters.
     */
    delete(name: string): void;
    /**
     * Returns the first value associated to the given search parameter.
     */
    get(name: string): string | null;
    /**
     * Returns all the values association with a given search parameter.
     */
    getAll(name: string): string[];
    /**
     * Returns a Boolean indicating if such a search parameter exists.
     */
    has(name: string): boolean;
    /**
     * Sets the value associated to a given search parameter to the given value.
     * If there were several values, delete the others.
     */
    set(name: string, value: string): void;
    /**
     * Sort all key/value pairs contained in this object in place
     * and return undefined. The sort order is according to Unicode
     * code points of the keys.
     */
    sort(): void;
    /**
     * Returns a query string suitable for use in a URL.
     */
    toString(): string;
    /**
     * Iterates over each name-value pair in the query
     * and invokes the given function.
     */
    forEach(
      callbackfn: (value: string, key: string, parent: URLSearchParams) => void,
      thisArg?: any
    ): void;
  }
  export interface EventListener {
    handleEvent(event: Event): void;
    readonly callback: (event: Event) => void | null;
    readonly options: boolean | AddEventListenerOptions;
  }
  export interface EventInit {
    bubbles?: boolean;
    cancelable?: boolean;
    composed?: boolean;
  }
  export interface CustomEventInit extends EventInit {
    detail?: any;
  }
  export enum EventPhase {
    NONE = 0,
    CAPTURING_PHASE = 1,
    AT_TARGET = 2,
    BUBBLING_PHASE = 3
  }
  export interface EventPath {
    item: EventTarget;
    itemInShadowTree: boolean;
    relatedTarget: EventTarget | null;
    rootOfClosedTree: boolean;
    slotInClosedTree: boolean;
    target: EventTarget | null;
    touchTargetList: EventTarget[];
  }
  export interface Event {
    readonly type: string;
    target: EventTarget | null;
    currentTarget: EventTarget | null;
    composedPath(): EventPath[];
    eventPhase: number;
    stopPropagation(): void;
    stopImmediatePropagation(): void;
    readonly bubbles: boolean;
    readonly cancelable: boolean;
    preventDefault(): void;
    readonly defaultPrevented: boolean;
    readonly composed: boolean;
    isTrusted: boolean;
    readonly timeStamp: Date;
    dispatched: boolean;
    readonly initialized: boolean;
    inPassiveListener: boolean;
    cancelBubble: boolean;
    cancelBubbleImmediately: boolean;
    path: EventPath[];
    relatedTarget: EventTarget | null;
  }
  export interface CustomEvent extends Event {
    readonly detail: any;
    initCustomEvent(
      type: string,
      bubbles?: boolean,
      cancelable?: boolean,
      detail?: any | null
    ): void;
  }
  export interface DomFile extends Blob {
    readonly lastModified: number;
    readonly name: string;
  }
  export interface DomFileConstructor {
    new (
      bits: BlobPart[],
      filename: string,
      options?: FilePropertyBag
    ): DomFile;
    prototype: DomFile;
  }
  export interface FilePropertyBag extends BlobPropertyBag {
    lastModified?: number;
  }
  interface ProgressEvent extends Event {
    readonly lengthComputable: boolean;
    readonly loaded: number;
    readonly total: number;
  }
  export interface EventListenerOptions {
    capture: boolean;
  }
  export interface AddEventListenerOptions extends EventListenerOptions {
    once: boolean;
    passive: boolean;
  }
  interface AbortSignal extends EventTarget {
    readonly aborted: boolean;
    onabort: ((this: AbortSignal, ev: ProgressEvent) => any) | null;
    addEventListener<K extends keyof AbortSignalEventMap>(
      type: K,
      listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
      options?: boolean | AddEventListenerOptions
    ): void;
    addEventListener(
      type: string,
      listener: EventListener,
      options?: boolean | AddEventListenerOptions
    ): void;
    removeEventListener<K extends keyof AbortSignalEventMap>(
      type: K,
      listener: (this: AbortSignal, ev: AbortSignalEventMap[K]) => any,
      options?: boolean | EventListenerOptions
    ): void;
    removeEventListener(
      type: string,
      listener: EventListener,
      options?: boolean | EventListenerOptions
    ): void;
  }
  export interface ReadableStream {
    readonly locked: boolean;
    cancel(): Promise<void>;
    getReader(): ReadableStreamReader;
    tee(): [ReadableStream, ReadableStream];
  }
  export interface ReadableStreamReader {
    cancel(): Promise<void>;
    read(): Promise<any>;
    releaseLock(): void;
  }
  export interface FormData extends DomIterable<string, FormDataEntryValue> {
    append(name: string, value: string | Blob, fileName?: string): void;
    delete(name: string): void;
    get(name: string): FormDataEntryValue | null;
    getAll(name: string): FormDataEntryValue[];
    has(name: string): boolean;
    set(name: string, value: string | Blob, fileName?: string): void;
  }
  export interface FormDataConstructor {
    new (): FormData;
    prototype: FormData;
  }
  /** A blob object represents a file-like object of immutable, raw data. */
  export interface Blob {
    /** The size, in bytes, of the data contained in the `Blob` object. */
    readonly size: number;
    /** A string indicating the media type of the data contained in the `Blob`.
     * If the type is unknown, this string is empty.
     */
    readonly type: string;
    /** Returns a new `Blob` object containing the data in the specified range of
     * bytes of the source `Blob`.
     */
    slice(start?: number, end?: number, contentType?: string): Blob;
  }
  export interface Body {
    /** A simple getter used to expose a `ReadableStream` of the body contents. */
    readonly body: ReadableStream | null;
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
  export interface Headers extends DomIterable<string, string> {
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
  export interface HeadersConstructor {
    new (init?: HeadersInit): Headers;
    prototype: Headers;
  }
  type RequestCache =
    | "default"
    | "no-store"
    | "reload"
    | "no-cache"
    | "force-cache"
    | "only-if-cached";
  type RequestCredentials = "omit" | "same-origin" | "include";
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
  type RequestMode = "navigate" | "same-origin" | "no-cors" | "cors";
  type RequestRedirect = "follow" | "error" | "manual";
  type ResponseType =
    | "basic"
    | "cors"
    | "default"
    | "error"
    | "opaque"
    | "opaqueredirect";
  export interface RequestInit {
    body?: BodyInit | null;
    cache?: RequestCache;
    credentials?: RequestCredentials;
    headers?: HeadersInit;
    integrity?: string;
    keepalive?: boolean;
    method?: string;
    mode?: RequestMode;
    redirect?: RequestRedirect;
    referrer?: string;
    referrerPolicy?: ReferrerPolicy;
    signal?: AbortSignal | null;
    window?: any;
  }
  export interface ResponseInit {
    headers?: HeadersInit;
    status?: number;
    statusText?: string;
  }
  export interface RequestConstructor {
    new (input: RequestInfo, init?: RequestInit): Request;
    prototype: Request;
  }
  export interface Request extends Body {
    /** Returns the cache mode associated with request, which is a string
     * indicating how the the request will interact with the browser's cache when
     * fetching.
     */
    readonly cache?: RequestCache;
    /** Returns the credentials mode associated with request, which is a string
     * indicating whether credentials will be sent with the request always, never,
     * or only when sent to a same-origin URL.
     */
    readonly credentials?: RequestCredentials;
    /** Returns the kind of resource requested by request, (e.g., `document` or
     * `script`).
     */
    readonly destination?: RequestDestination;
    /** Returns a Headers object consisting of the headers associated with
     * request.
     *
     * Note that headers added in the network layer by the user agent
     * will not be accounted for in this object, (e.g., the `Host` header).
     */
    readonly headers: Headers;
    /** Returns request's subresource integrity metadata, which is a cryptographic
     * hash of the resource being fetched. Its value consists of multiple hashes
     * separated by whitespace. [SRI]
     */
    readonly integrity?: string;
    /** Returns a boolean indicating whether or not request is for a history
     * navigation (a.k.a. back-forward navigation).
     */
    readonly isHistoryNavigation?: boolean;
    /** Returns a boolean indicating whether or not request is for a reload
     * navigation.
     */
    readonly isReloadNavigation?: boolean;
    /** Returns a boolean indicating whether or not request can outlive the global
     * in which it was created.
     */
    readonly keepalive?: boolean;
    /** Returns request's HTTP method, which is `GET` by default. */
    readonly method: string;
    /** Returns the mode associated with request, which is a string indicating
     * whether the request will use CORS, or will be restricted to same-origin
     * URLs.
     */
    readonly mode?: RequestMode;
    /** Returns the redirect mode associated with request, which is a string
     * indicating how redirects for the request will be handled during fetching.
     *
     * A request will follow redirects by default.
     */
    readonly redirect?: RequestRedirect;
    /** Returns the referrer of request. Its value can be a same-origin URL if
     * explicitly set in init, the empty string to indicate no referrer, and
     * `about:client` when defaulting to the global's default.
     *
     * This is used during fetching to determine the value of the `Referer`
     * header of the request being made.
     */
    readonly referrer?: string;
    /** Returns the referrer policy associated with request. This is used during
     * fetching to compute the value of the request's referrer.
     */
    readonly referrerPolicy?: ReferrerPolicy;
    /** Returns the signal associated with request, which is an AbortSignal object
     * indicating whether or not request has been aborted, and its abort event
     * handler.
     */
    readonly signal?: AbortSignal;
    /** Returns the URL of request as a string. */
    readonly url: string;
    clone(): Request;
  }
  export interface Response extends Body {
    /** Contains the `Headers` object associated with the response. */
    readonly headers: Headers;
    /** Contains a boolean stating whether the response was successful (status in
     * the range 200-299) or not.
     */
    readonly ok: boolean;
    /** Indicates whether or not the response is the result of a redirect; that
     * is, its URL list has more than one entry.
     */
    readonly redirected: boolean;
    /** Contains the status code of the response (e.g., `200` for a success). */
    readonly status: number;
    /** Contains the status message corresponding to the status code (e.g., `OK`
     * for `200`).
     */
    readonly statusText: string;
    readonly trailer: Promise<Headers>;
    /** Contains the type of the response (e.g., `basic`, `cors`). */
    readonly type: ResponseType;
    /** Contains the URL of the response. */
    readonly url: string;
    /** Creates a clone of a `Response` object. */
    clone(): Response;
  }
  export interface Location {
    /**
     * Returns a DOMStringList object listing the origins of the ancestor browsing
     * contexts, from the parent browsing context to the top-level browsing
     * context.
     */
    readonly ancestorOrigins: string[];
    /**
     * Returns the Location object's URL's fragment (includes leading "#" if
     * non-empty).
     * Can be set, to navigate to the same URL with a changed fragment (ignores
     * leading "#").
     */
    hash: string;
    /**
     * Returns the Location object's URL's host and port (if different from the
     * default port for the scheme).  Can be set, to navigate to the same URL with
     * a changed host and port.
     */
    host: string;
    /**
     * Returns the Location object's URL's host.  Can be set, to navigate to the
     * same URL with a changed host.
     */
    hostname: string;
    /**
     * Returns the Location object's URL.  Can be set, to navigate to the given
     * URL.
     */
    href: string;
    /** Returns the Location object's URL's origin. */
    readonly origin: string;
    /**
     * Returns the Location object's URL's path.
     * Can be set, to navigate to the same URL with a changed path.
     */
    pathname: string;
    /**
     * Returns the Location object's URL's port.
     * Can be set, to navigate to the same URL with a changed port.
     */
    port: string;
    /**
     * Returns the Location object's URL's scheme.
     * Can be set, to navigate to the same URL with a changed scheme.
     */
    protocol: string;
    /**
     * Returns the Location object's URL's query (includes leading "?" if
     * non-empty). Can be set, to navigate to the same URL with a changed query
     * (ignores leading "?").
     */
    search: string;
    /**
     * Navigates to the given URL.
     */
    assign(url: string): void;
    /**
     * Reloads the current page.
     */
    reload(): void;
    /** @deprecated */
    reload(forcedReload: boolean): void;
    /**
     * Removes the current page from the session history and navigates to the
     * given URL.
     */
    replace(url: string): void;
  }
}

declare namespace blob {
  // @url js/blob.d.ts

  export const bytesSymbol: unique symbol;
  export const blobBytesWeakMap: WeakMap<domTypes.Blob, Uint8Array>;
  export class DenoBlob implements domTypes.Blob {
    private readonly [bytesSymbol];
    readonly size: number;
    readonly type: string;
    /** A blob object represents a file-like object of immutable, raw data. */
    constructor(
      blobParts?: domTypes.BlobPart[],
      options?: domTypes.BlobPropertyBag
    );
    slice(start?: number, end?: number, contentType?: string): DenoBlob;
  }
}

declare namespace consoleTypes {
  // @url js/console.d.ts

  type ConsoleOptions = Partial<{
    showHidden: boolean;
    depth: number;
    colors: boolean;
    indentLevel: number;
  }>;
  export class CSI {
    static kClear: string;
    static kClearScreenDown: string;
  }
  const isConsoleInstance: unique symbol;
  export class Console {
    private printFunc;
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
  /** A symbol which can be used as a key for a custom method which will be called
   * when `Deno.inspect()` is called, or when the object is logged to the console.
   */
  export const customInspect: unique symbol;
  /**
   * `inspect()` converts input into string that has the same format
   * as printed by `console.log(...)`;
   */
  export function inspect(value: unknown, options?: ConsoleOptions): string;
}

declare namespace event {
  // @url js/event.d.ts

  export const eventAttributes: WeakMap<object, any>;
  export class EventInit implements domTypes.EventInit {
    bubbles: boolean;
    cancelable: boolean;
    composed: boolean;
    constructor({
      bubbles,
      cancelable,
      composed
    }?: {
      bubbles?: boolean | undefined;
      cancelable?: boolean | undefined;
      composed?: boolean | undefined;
    });
  }
  export class Event implements domTypes.Event {
    isTrusted: boolean;
    private _canceledFlag;
    private _dispatchedFlag;
    private _initializedFlag;
    private _inPassiveListenerFlag;
    private _stopImmediatePropagationFlag;
    private _stopPropagationFlag;
    private _path;
    constructor(type: string, eventInitDict?: domTypes.EventInit);
    readonly bubbles: boolean;
    cancelBubble: boolean;
    cancelBubbleImmediately: boolean;
    readonly cancelable: boolean;
    readonly composed: boolean;
    currentTarget: domTypes.EventTarget;
    readonly defaultPrevented: boolean;
    dispatched: boolean;
    eventPhase: number;
    readonly initialized: boolean;
    inPassiveListener: boolean;
    path: domTypes.EventPath[];
    relatedTarget: domTypes.EventTarget;
    target: domTypes.EventTarget;
    readonly timeStamp: Date;
    readonly type: string;
    /** Returns the event’s path (objects on which listeners will be
     * invoked). This does not include nodes in shadow trees if the
     * shadow root was created with its ShadowRoot.mode closed.
     *
     *      event.composedPath();
     */
    composedPath(): domTypes.EventPath[];
    /** Cancels the event (if it is cancelable).
     * See https://dom.spec.whatwg.org/#set-the-canceled-flag
     *
     *      event.preventDefault();
     */
    preventDefault(): void;
    /** Stops the propagation of events further along in the DOM.
     *
     *      event.stopPropagation();
     */
    stopPropagation(): void;
    /** For this particular event, no other listener will be called.
     * Neither those attached on the same element, nor those attached
     * on elements which will be traversed later (in capture phase,
     * for instance).
     *
     *      event.stopImmediatePropagation();
     */
    stopImmediatePropagation(): void;
  }
}

declare namespace customEvent {
  // @url js/custom_event.d.ts

  export const customEventAttributes: WeakMap<object, any>;
  export class CustomEventInit extends event.EventInit
    implements domTypes.CustomEventInit {
    detail: any;
    constructor({
      bubbles,
      cancelable,
      composed,
      detail
    }: domTypes.CustomEventInit);
  }
  export class CustomEvent extends event.Event implements domTypes.CustomEvent {
    constructor(type: string, customEventInitDict?: domTypes.CustomEventInit);
    readonly detail: any;
    initCustomEvent(
      type: string,
      bubbles?: boolean,
      cancelable?: boolean,
      detail?: any
    ): void;
    readonly [Symbol.toStringTag]: string;
  }
}

declare namespace eventTarget {
  // @url js/event_target.d.ts

  export class EventListenerOptions implements domTypes.EventListenerOptions {
    _capture: boolean;
    constructor({ capture }?: { capture?: boolean | undefined });
    readonly capture: boolean;
  }
  export class AddEventListenerOptions extends EventListenerOptions
    implements domTypes.AddEventListenerOptions {
    _passive: boolean;
    _once: boolean;
    constructor({
      capture,
      passive,
      once
    }?: {
      capture?: boolean | undefined;
      passive?: boolean | undefined;
      once?: boolean | undefined;
    });
    readonly passive: boolean;
    readonly once: boolean;
  }
  export class EventListener implements domTypes.EventListener {
    allEvents: domTypes.Event[];
    atEvents: domTypes.Event[];
    bubbledEvents: domTypes.Event[];
    capturedEvents: domTypes.Event[];
    private _callback;
    private _options;
    constructor(
      callback: (event: domTypes.Event) => void | null,
      options: boolean | domTypes.AddEventListenerOptions
    );
    handleEvent(event: domTypes.Event): void;
    readonly callback: (event: domTypes.Event) => void | null;
    readonly options: domTypes.AddEventListenerOptions | boolean;
  }
  export const eventTargetAssignedSlot: unique symbol;
  export const eventTargetHasActivationBehavior: unique symbol;
  export class EventTarget implements domTypes.EventTarget {
    [domTypes.eventTargetHost]: domTypes.EventTarget | null;
    [domTypes.eventTargetListeners]: {
      [type in string]: domTypes.EventListener[]
    };
    [domTypes.eventTargetMode]: string;
    [domTypes.eventTargetNodeType]: domTypes.NodeType;
    private [eventTargetAssignedSlot];
    private [eventTargetHasActivationBehavior];
    addEventListener(
      type: string,
      callback: (event: domTypes.Event) => void | null,
      options?: domTypes.AddEventListenerOptions | boolean
    ): void;
    removeEventListener(
      type: string,
      callback: (event: domTypes.Event) => void | null,
      options?: domTypes.EventListenerOptions | boolean
    ): void;
    dispatchEvent(event: domTypes.Event): boolean;
    readonly [Symbol.toStringTag]: string;
  }
}

declare namespace io {
  // @url js/io.d.ts

  export const EOF: null;
  export type EOF = null;
  export enum SeekMode {
    SEEK_START = 0,
    SEEK_CURRENT = 1,
    SEEK_END = 2
  }
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
}

declare namespace fetchTypes {
  // @url js/fetch.d.ts

  class Body implements domTypes.Body, domTypes.ReadableStream, io.ReadCloser {
    private rid;
    readonly contentType: string;
    bodyUsed: boolean;
    private _bodyPromise;
    private _data;
    readonly locked: boolean;
    readonly body: null | Body;
    constructor(rid: number, contentType: string);
    private _bodyBuffer;
    arrayBuffer(): Promise<ArrayBuffer>;
    blob(): Promise<domTypes.Blob>;
    formData(): Promise<domTypes.FormData>;
    json(): Promise<any>;
    text(): Promise<string>;
    read(p: Uint8Array): Promise<number | io.EOF>;
    close(): void;
    cancel(): Promise<void>;
    getReader(): domTypes.ReadableStreamReader;
    tee(): [domTypes.ReadableStream, domTypes.ReadableStream];
    [Symbol.asyncIterator](): AsyncIterableIterator<Uint8Array>;
  }
  export class Response implements domTypes.Response {
    readonly url: string;
    readonly status: number;
    statusText: string;
    readonly type = "basic";
    readonly redirected: boolean;
    headers: domTypes.Headers;
    readonly trailer: Promise<domTypes.Headers>;
    bodyUsed: boolean;
    readonly body: Body;
    constructor(
      url: string,
      status: number,
      headersList: Array<[string, string]>,
      rid: number,
      redirected_: boolean,
      body_?: null | Body
    );
    arrayBuffer(): Promise<ArrayBuffer>;
    blob(): Promise<domTypes.Blob>;
    formData(): Promise<domTypes.FormData>;
    json(): Promise<any>;
    text(): Promise<string>;
    readonly ok: boolean;
    clone(): domTypes.Response;
  }
  /** Fetch a resource from the network. */
  export function fetch(
    input: domTypes.Request | string,
    init?: domTypes.RequestInit
  ): Promise<Response>;
}

declare namespace textEncoding {
  // @url js/text_encoding.d.ts

  export function atob(s: string): string;
  /** Creates a base-64 ASCII string from the input string. */
  export function btoa(s: string): string;
  export interface TextDecodeOptions {
    stream?: false;
  }
  export interface TextDecoderOptions {
    fatal?: boolean;
    ignoreBOM?: false;
  }
  export class TextDecoder {
    private _encoding;
    /** Returns encoding's name, lowercased. */
    readonly encoding: string;
    /** Returns `true` if error mode is "fatal", and `false` otherwise. */
    readonly fatal: boolean;
    /** Returns `true` if ignore BOM flag is set, and `false` otherwise. */
    readonly ignoreBOM = false;
    constructor(label?: string, options?: TextDecoderOptions);
    /** Returns the result of running encoding's decoder. */
    decode(input?: domTypes.BufferSource, options?: TextDecodeOptions): string;
    readonly [Symbol.toStringTag]: string;
  }
  interface TextEncoderEncodeIntoResult {
    read: number;
    written: number;
  }
  export class TextEncoder {
    /** Returns "utf-8". */
    readonly encoding = "utf-8";
    /** Returns the result of running UTF-8's encoder. */
    encode(input?: string): Uint8Array;
    encodeInto(input: string, dest: Uint8Array): TextEncoderEncodeIntoResult;
    readonly [Symbol.toStringTag]: string;
  }
}

declare namespace timers {
  // @url js/timers.d.ts

  export type Args = unknown[];
  /** Sets a timer which executes a function once after the timer expires. */
  export function setTimeout(
    cb: (...args: Args) => void,
    delay?: number,
    ...args: Args
  ): number;
  /** Repeatedly calls a function , with a fixed time delay between each call. */
  export function setInterval(
    cb: (...args: Args) => void,
    delay?: number,
    ...args: Args
  ): number;
  export function clearTimeout(id?: number): void;
  export function clearInterval(id?: number): void;
}

declare namespace urlSearchParams {
  // @url js/url_search_params.d.ts

  export class URLSearchParams {
    private params;
    private url;
    constructor(init?: string | string[][] | Record<string, string>);
    private updateSteps;
    /** Appends a specified key/value pair as a new search parameter.
     *
     *       searchParams.append('name', 'first');
     *       searchParams.append('name', 'second');
     */
    append(name: string, value: string): void;
    /** Deletes the given search parameter and its associated value,
     * from the list of all search parameters.
     *
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
     *       searchParams.forEach((value, key, parent) => {
     *         console.log(value, key, parent);
     *       });
     *
     */
    forEach(
      callbackfn: (value: string, key: string, parent: URLSearchParams) => void,
      thisArg?: any
    ): void;
    /** Returns an iterator allowing to go through all keys contained
     * in this object.
     *
     *       for (const key of searchParams.keys()) {
     *         console.log(key);
     *       }
     */
    keys(): Iterable<string>;
    /** Returns an iterator allowing to go through all values contained
     * in this object.
     *
     *       for (const value of searchParams.values()) {
     *         console.log(value);
     *       }
     */
    values(): Iterable<string>;
    /** Returns an iterator allowing to go through all key/value
     * pairs contained in this object.
     *
     *       for (const [key, value] of searchParams.entries()) {
     *         console.log(key, value);
     *       }
     */
    entries(): Iterable<[string, string]>;
    /** Returns an iterator allowing to go through all key/value
     * pairs contained in this object.
     *
     *       for (const [key, value] of searchParams[Symbol.iterator]()) {
     *         console.log(key, value);
     *       }
     */
    [Symbol.iterator](): Iterable<[string, string]>;
    /** Returns a query string suitable for use in a URL.
     *
     *        searchParams.toString();
     */
    toString(): string;
    private _handleStringInitialization;
    private _handleArrayInitialization;
  }
}

declare namespace url {
  // @url js/url.d.ts

  export const blobURLMap: Map<string, domTypes.Blob>;
  export class URL {
    private _parts;
    private _searchParams;
    private _updateSearchParams;
    hash: string;
    host: string;
    hostname: string;
    href: string;
    readonly origin: string;
    password: string;
    pathname: string;
    port: string;
    protocol: string;
    search: string;
    username: string;
    readonly searchParams: urlSearchParams.URLSearchParams;
    constructor(url: string, base?: string | URL);
    toString(): string;
    toJSON(): string;
    static createObjectURL(b: domTypes.Blob): string;
    static revokeObjectURL(url: string): void;
  }
}

declare namespace workers {
  // @url js/workers.d.ts

  export function encodeMessage(data: any): Uint8Array;
  export function decodeMessage(dataIntArray: Uint8Array): any;
  export let onmessage: (e: { data: any }) => void;
  export function postMessage(data: any): void;
  export function getMessage(): Promise<any>;
  export let isClosing: boolean;
  export function workerClose(): void;
  export function workerMain(): Promise<void>;
  export interface Worker {
    onerror?: () => void;
    onmessage?: (e: { data: any }) => void;
    onmessageerror?: () => void;
    postMessage(data: any): void;
    closed: Promise<void>;
  }
  export interface WorkerOptions {}
  /** Extended Deno Worker initialization options.
   * `noDenoNamespace` hides global `window.Deno` namespace for
   * spawned worker and nested workers spawned by it (default: false).
   */
  export interface DenoWorkerOptions extends WorkerOptions {
    noDenoNamespace?: boolean;
  }
  export class WorkerImpl implements Worker {
    private readonly rid;
    private isClosing;
    private readonly isClosedPromise;
    onerror?: () => void;
    onmessage?: (data: any) => void;
    onmessageerror?: () => void;
    constructor(specifier: string, options?: DenoWorkerOptions);
    readonly closed: Promise<void>;
    postMessage(data: any): void;
    private run;
  }
}

declare namespace performanceUtil {
  // @url js/performance.d.ts

  export class Performance {
    /** Returns a current time from Deno's start in milliseconds.
     *
     * Use the flag --allow-hrtime return a precise value.
     *
     *       const t = performance.now();
     *       console.log(`${t} ms since start!`);
     */
    now(): number;
  }
}

// @url js/lib.web_assembly.d.ts

// This follows the WebIDL at: https://webassembly.github.io/spec/js-api/
// And follow on WebIDL at: https://webassembly.github.io/spec/web-api/

/* eslint-disable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */

declare namespace WebAssembly {
  interface WebAssemblyInstantiatedSource {
    module: Module;
    instance: Instance;
  }

  /** Compiles a `WebAssembly.Module` from WebAssembly binary code.  This
   * function is useful if it is necessary to a compile a module before it can
   * be instantiated (otherwise, the `WebAssembly.instantiate()` function
   * should be used). */
  function compile(bufferSource: domTypes.BufferSource): Promise<Module>;

  /** Compiles a `WebAssembly.Module` directly from a streamed underlying
   * source. This function is useful if it is necessary to a compile a module
   * before it can be instantiated (otherwise, the
   * `WebAssembly.instantiateStreaming()` function should be used). */
  function compileStreaming(
    source: Promise<domTypes.Response>
  ): Promise<Module>;

  /** Takes the WebAssembly binary code, in the form of a typed array or
   * `ArrayBuffer`, and performs both compilation and instantiation in one step.
   * The returned `Promise` resolves to both a compiled `WebAssembly.Module` and
   * its first `WebAssembly.Instance`. */
  function instantiate(
    bufferSource: domTypes.BufferSource,
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
    source: Promise<domTypes.Response>,
    importObject?: object
  ): Promise<WebAssemblyInstantiatedSource>;

  /** Validates a given typed array of WebAssembly binary code, returning
   * whether the bytes form a valid wasm module (`true`) or not (`false`). */
  function validate(bufferSource: domTypes.BufferSource): boolean;

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
    constructor(bufferSource: domTypes.BufferSource);

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

  interface GlobalDescriptor {
    value: string;
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

/* eslint-enable @typescript-eslint/no-unused-vars, @typescript-eslint/no-explicit-any */
