/**
 * The `node:vfs` module provides an in-memory virtual filesystem with a
 * `node:fs`-compatible API.
 *
 * ```js
 * import { create } from 'node:vfs';
 *
 * const vfs = create();
 * vfs.writeFileSync('/app/file.txt', 'hello');
 * vfs.readFileSync('/app/file.txt', 'utf8'); // 'hello'
 * ```
 *
 * This module is only available under the `node:` scheme.
 *
 * @experimental
 * @see https://github.com/nodejs/node/pull/63115
 */
declare module "node:vfs" {
  import type { Buffer } from "node:buffer";
  import type { Stats, BigIntStats, Dirent, ReadStream, WriteStream } from "node:fs";
  import type { EventEmitter } from "node:events";

  type StatOptions = { bigint?: boolean };
  type EncodingOption = string | { encoding?: string };
  type ReadResult = { bytesRead: number; buffer: Uint8Array };
  type WriteResult = { bytesWritten: number; buffer: Uint8Array };
  type WriteFileOptions = { encoding?: string; mode?: number; flag?: string };
  type ReadFileOptions = string | { encoding?: string; flag?: string };
  type MkdirOptions = { recursive?: boolean; mode?: number };
  type ReaddirOptions = { withFileTypes?: boolean; recursive?: boolean };
  type RmOptions = { recursive?: boolean; force?: boolean };

  class VirtualFileHandle {
    readonly path: string;
    readonly flags: string;
    readonly mode: number;
    position: number;
    readonly closed: boolean;
    read(
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): Promise<ReadResult>;
    readSync(
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): number;
    write(
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): Promise<WriteResult>;
    writeSync(
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): number;
    readFile(options?: ReadFileOptions): Promise<Buffer | string>;
    readFileSync(options?: ReadFileOptions): Buffer | string;
    writeFile(
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): Promise<void>;
    writeFileSync(
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): void;
    appendFile(
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): Promise<void>;
    stat(options?: StatOptions): Promise<Stats | BigIntStats>;
    statSync(options?: StatOptions): Stats | BigIntStats;
    truncate(len?: number): Promise<void>;
    truncateSync(len?: number): void;
    chmod(mode: number): Promise<void>;
    chown(uid: number, gid: number): Promise<void>;
    utimes(atime: Date | number, mtime: Date | number): Promise<void>;
    sync(): Promise<void>;
    datasync(): Promise<void>;
    readv(
      buffers: Uint8Array[],
      position?: number | null,
    ): Promise<{ bytesRead: number; buffers: Uint8Array[] }>;
    writev(
      buffers: Uint8Array[],
      position?: number | null,
    ): Promise<{ bytesWritten: number; buffers: Uint8Array[] }>;
    close(): Promise<void>;
    closeSync(): void;
  }

  class MemoryFileHandle extends VirtualFileHandle {}

  class VirtualProvider {
    readonly readonly: boolean;
    readonly supportsSymlinks: boolean;
    readonly supportsWatch: boolean;
    open(
      path: string,
      flags: string,
      mode?: number,
    ): Promise<VirtualFileHandle>;
    openSync(path: string, flags: string, mode?: number): VirtualFileHandle;
    stat(path: string, options?: StatOptions): Promise<Stats | BigIntStats>;
    statSync(path: string, options?: StatOptions): Stats | BigIntStats;
    lstat(path: string, options?: StatOptions): Promise<Stats | BigIntStats>;
    lstatSync(path: string, options?: StatOptions): Stats | BigIntStats;
    readdir(
      path: string,
      options?: ReaddirOptions,
    ): Promise<string[] | Dirent[]>;
    readdirSync(path: string, options?: ReaddirOptions): string[] | Dirent[];
    mkdir(path: string, options?: MkdirOptions): Promise<string | undefined>;
    mkdirSync(path: string, options?: MkdirOptions): string | undefined;
    rmdir(path: string): Promise<void>;
    rmdirSync(path: string): void;
    unlink(path: string): Promise<void>;
    unlinkSync(path: string): void;
    rename(oldPath: string, newPath: string): Promise<void>;
    renameSync(oldPath: string, newPath: string): void;
    readFile(
      path: string,
      options?: ReadFileOptions,
    ): Promise<Buffer | string>;
    readFileSync(path: string, options?: ReadFileOptions): Buffer | string;
    writeFile(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): Promise<void>;
    writeFileSync(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): void;
    appendFile(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): Promise<void>;
    appendFileSync(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): void;
    exists(path: string): Promise<boolean>;
    existsSync(path: string): boolean;
    copyFile(src: string, dest: string, mode?: number): Promise<void>;
    copyFileSync(src: string, dest: string, mode?: number): void;
    realpath(path: string, options?: unknown): Promise<string>;
    realpathSync(path: string, options?: unknown): string;
    access(path: string, mode?: number): Promise<void>;
    accessSync(path: string, mode?: number): void;
    link(existingPath: string, newPath: string): Promise<void>;
    linkSync(existingPath: string, newPath: string): void;
    readlink(path: string, options?: unknown): Promise<string>;
    readlinkSync(path: string, options?: unknown): string;
    symlink(target: string, path: string, type?: string): Promise<void>;
    symlinkSync(target: string, path: string, type?: string): void;
    watch(path: string, options?: unknown): EventEmitter;
    watchAsync(path: string, options?: unknown): AsyncIterable<unknown>;
    watchFile(
      path: string,
      options?: unknown,
      listener?: (curr: Stats, prev: Stats) => void,
    ): EventEmitter;
    unwatchFile(path: string, listener?: (...args: unknown[]) => void): void;
  }

  class MemoryProvider extends VirtualProvider {
    constructor();
    setReadOnly(): void;
  }

  class RealFSProvider extends VirtualProvider {
    constructor(rootPath: string);
    readonly rootPath: string;
  }

  class VirtualDir {
    readonly path: string;
    readSync(): Dirent | null;
    read(): Promise<Dirent | null>;
    read(callback: (err: Error | null, dirent: Dirent | null) => void): void;
    closeSync(): void;
    close(): Promise<void>;
    close(callback: (err: Error | null) => void): void;
    entries(): AsyncIterableIterator<Dirent>;
    [Symbol.asyncIterator](): AsyncIterableIterator<Dirent>;
    [Symbol.asyncDispose](): Promise<void>;
  }

  interface VirtualFileSystemOptions {
    emitExperimentalWarning?: boolean;
  }

  interface VirtualFileSystemPromises {
    readFile(path: string, options?: ReadFileOptions): Promise<Buffer | string>;
    writeFile(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): Promise<void>;
    appendFile(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): Promise<void>;
    stat(path: string, options?: StatOptions): Promise<Stats | BigIntStats>;
    lstat(path: string, options?: StatOptions): Promise<Stats | BigIntStats>;
    readdir(
      path: string,
      options?: ReaddirOptions,
    ): Promise<string[] | Dirent[]>;
    mkdir(path: string, options?: MkdirOptions): Promise<string | undefined>;
    rmdir(path: string): Promise<void>;
    unlink(path: string): Promise<void>;
    rename(oldPath: string, newPath: string): Promise<void>;
    copyFile(src: string, dest: string, mode?: number): Promise<void>;
    realpath(path: string, options?: unknown): Promise<string>;
    readlink(path: string, options?: unknown): Promise<string>;
    symlink(target: string, path: string, type?: string): Promise<void>;
    access(path: string, mode?: number): Promise<void>;
    rm(path: string, options?: RmOptions): Promise<void>;
    truncate(path: string, len?: number): Promise<void>;
    link(existingPath: string, newPath: string): Promise<void>;
    mkdtemp(prefix: string): Promise<string>;
    chmod(path: string, mode: number): Promise<void>;
    lchmod(path: string, mode: number): Promise<void>;
    chown(path: string, uid: number, gid: number): Promise<void>;
    lchown(path: string, uid: number, gid: number): Promise<void>;
    utimes(
      path: string,
      atime: Date | number,
      mtime: Date | number,
    ): Promise<void>;
    lutimes(
      path: string,
      atime: Date | number,
      mtime: Date | number,
    ): Promise<void>;
    open(path: string, flags?: string, mode?: number): Promise<number>;
    watch(path: string, options?: unknown): AsyncIterable<unknown>;
  }

  class VirtualFileSystem {
    constructor(
      providerOrOptions?: VirtualProvider | VirtualFileSystemOptions,
      options?: VirtualFileSystemOptions,
    );
    readonly provider: VirtualProvider;
    readonly readonly: boolean;

    existsSync(path: string): boolean;
    statSync(path: string, options?: StatOptions): Stats | BigIntStats;
    lstatSync(path: string, options?: StatOptions): Stats | BigIntStats;
    readFileSync(path: string, options?: ReadFileOptions): Buffer | string;
    writeFileSync(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): void;
    appendFileSync(
      path: string,
      data: string | Uint8Array,
      options?: WriteFileOptions,
    ): void;
    readdirSync(path: string, options?: ReaddirOptions): string[] | Dirent[];
    mkdirSync(path: string, options?: MkdirOptions): string | undefined;
    rmdirSync(path: string): void;
    unlinkSync(path: string): void;
    renameSync(oldPath: string, newPath: string): void;
    copyFileSync(src: string, dest: string, mode?: number): void;
    realpathSync(path: string, options?: unknown): string;
    readlinkSync(path: string, options?: unknown): string;
    symlinkSync(target: string, path: string, type?: string): void;
    accessSync(path: string, mode?: number): void;
    rmSync(path: string, options?: RmOptions): void;
    truncateSync(path: string, len?: number): void;
    ftruncateSync(fd: number, len?: number): void;
    linkSync(existingPath: string, newPath: string): void;
    chmodSync(path: string, mode: number): void;
    chownSync(path: string, uid: number, gid: number): void;
    utimesSync(
      path: string,
      atime: Date | number,
      mtime: Date | number,
    ): void;
    lutimesSync(
      path: string,
      atime: Date | number,
      mtime: Date | number,
    ): void;
    mkdtempSync(prefix: string): string;
    opendirSync(path: string, options?: { recursive?: boolean }): VirtualDir;
    openAsBlob(path: string, options?: { type?: string }): Blob;

    openSync(path: string, flags?: string, mode?: number): number;
    closeSync(fd: number): void;
    readSync(
      fd: number,
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): number;
    writeSync(
      fd: number,
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): number;
    fstatSync(fd: number, options?: StatOptions): Stats | BigIntStats;

    readFile(
      path: string,
      options:
        | ReadFileOptions
        | ((err: Error | null, data: Buffer | string) => void),
      callback?: (err: Error | null, data: Buffer | string) => void,
    ): void;
    writeFile(
      path: string,
      data: string | Uint8Array,
      options:
        | WriteFileOptions
        | ((err: Error | null) => void),
      callback?: (err: Error | null) => void,
    ): void;
    stat(
      path: string,
      options:
        | StatOptions
        | ((err: Error | null, stats: Stats | BigIntStats) => void),
      callback?: (err: Error | null, stats: Stats | BigIntStats) => void,
    ): void;
    lstat(
      path: string,
      options:
        | StatOptions
        | ((err: Error | null, stats: Stats | BigIntStats) => void),
      callback?: (err: Error | null, stats: Stats | BigIntStats) => void,
    ): void;
    readdir(
      path: string,
      options:
        | ReaddirOptions
        | ((err: Error | null, entries: string[] | Dirent[]) => void),
      callback?: (
        err: Error | null,
        entries: string[] | Dirent[],
      ) => void,
    ): void;
    realpath(
      path: string,
      options: unknown,
      callback?: (err: Error | null, resolved: string) => void,
    ): void;
    readlink(
      path: string,
      options: unknown,
      callback?: (err: Error | null, target: string) => void,
    ): void;
    access(
      path: string,
      mode: number | ((err: Error | null) => void),
      callback?: (err: Error | null) => void,
    ): void;
    open(
      path: string,
      flags: string | ((err: Error | null, fd: number) => void),
      mode?: number | ((err: Error | null, fd: number) => void),
      callback?: (err: Error | null, fd: number) => void,
    ): void;
    close(fd: number, callback: (err: Error | null) => void): void;
    read(
      fd: number,
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
      callback: (
        err: Error | null,
        bytesRead: number,
        buffer: Uint8Array,
      ) => void,
    ): void;
    write(
      fd: number,
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
      callback: (
        err: Error | null,
        bytesWritten: number,
        buffer: Uint8Array,
      ) => void,
    ): void;
    fstat(
      fd: number,
      options: StatOptions | ((err: Error | null, stats: Stats | BigIntStats) => void),
      callback?: (err: Error | null, stats: Stats | BigIntStats) => void,
    ): void;
    rm(
      path: string,
      options: RmOptions | ((err: Error | null) => void),
      callback?: (err: Error | null) => void,
    ): void;
    truncate(
      path: string,
      len: number | ((err: Error | null) => void),
      callback?: (err: Error | null) => void,
    ): void;
    ftruncate(
      fd: number,
      len: number | ((err: Error | null) => void),
      callback?: (err: Error | null) => void,
    ): void;
    link(
      existingPath: string,
      newPath: string,
      callback: (err: Error | null) => void,
    ): void;
    mkdtemp(
      prefix: string,
      options:
        | EncodingOption
        | ((err: Error | null, dirPath: string) => void),
      callback?: (err: Error | null, dirPath: string) => void,
    ): void;
    opendir(
      path: string,
      options:
        | { recursive?: boolean }
        | ((err: Error | null, dir: VirtualDir) => void),
      callback?: (err: Error | null, dir: VirtualDir) => void,
    ): void;

    createReadStream(
      path: string,
      options?: {
        start?: number;
        end?: number;
        highWaterMark?: number;
        encoding?: string;
        autoClose?: boolean;
        fd?: number;
      },
    ): ReadStream;
    createWriteStream(
      path: string,
      options?: {
        start?: number;
        highWaterMark?: number;
        encoding?: string;
        autoClose?: boolean;
        fd?: number;
        flags?: string;
      },
    ): WriteStream;

    watch(
      path: string,
      options?: unknown,
      listener?: (eventType: string, filename: string) => void,
    ): EventEmitter;
    watchFile(
      path: string,
      options?: unknown,
      listener?: (curr: Stats, prev: Stats) => void,
    ): EventEmitter;
    unwatchFile(path: string, listener?: (...args: unknown[]) => void): void;

    readonly promises: VirtualFileSystemPromises;
  }

  function create(
    provider?: VirtualProvider | VirtualFileSystemOptions,
    options?: VirtualFileSystemOptions,
  ): VirtualFileSystem;

  export {
    create,
    MemoryFileHandle,
    MemoryProvider,
    RealFSProvider,
    VirtualDir,
    VirtualFileHandle,
    VirtualFileSystem,
    VirtualProvider,
  };
}
