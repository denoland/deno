/**
 * The `node:vfs` module provides an in-memory virtual filesystem with an
 * `fs`-compatible API, mount points, and a provider-based storage layer.
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
 * @see https://github.com/nodejs/node/pull/61478
 */
declare module "node:vfs" {
  import type { Buffer } from "node:buffer";
  import type { Readable } from "node:stream";

  interface VirtualStats {
    readonly dev: number;
    readonly mode: number;
    readonly nlink: number;
    readonly uid: number;
    readonly gid: number;
    readonly rdev: number;
    readonly blksize: number;
    readonly ino: number;
    readonly size: number;
    readonly blocks: number;
    readonly atimeMs: number;
    readonly mtimeMs: number;
    readonly ctimeMs: number;
    readonly birthtimeMs: number;
    readonly atime: Date;
    readonly mtime: Date;
    readonly ctime: Date;
    readonly birthtime: Date;
    isFile(): boolean;
    isDirectory(): boolean;
    isSymbolicLink(): boolean;
    isBlockDevice(): boolean;
    isCharacterDevice(): boolean;
    isFIFO(): boolean;
    isSocket(): boolean;
  }

  interface VirtualDirent {
    readonly name: string;
    readonly parentPath: string;
    readonly path: string;
    isFile(): boolean;
    isDirectory(): boolean;
    isSymbolicLink(): boolean;
    isBlockDevice(): boolean;
    isCharacterDevice(): boolean;
    isFIFO(): boolean;
    isSocket(): boolean;
  }

  interface VirtualFileHandle {
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
    ): Promise<{ bytesRead: number; buffer: Uint8Array }>;
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
    ): Promise<{ bytesWritten: number; buffer: Uint8Array }>;
    writeSync(
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): number;
    readFile(
      options?: string | { encoding?: string },
    ): Promise<Buffer | string>;
    readFileSync(
      options?: string | { encoding?: string },
    ): Buffer | string;
    writeFile(
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): Promise<void>;
    writeFileSync(
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): void;
    stat(options?: unknown): Promise<VirtualStats>;
    statSync(options?: unknown): VirtualStats;
    truncate(len?: number): Promise<void>;
    truncateSync(len?: number): void;
    close(): Promise<void>;
    closeSync(): void;
  }

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
    stat(path: string, options?: unknown): Promise<VirtualStats>;
    statSync(path: string, options?: unknown): VirtualStats;
    lstat(path: string, options?: unknown): Promise<VirtualStats>;
    lstatSync(path: string, options?: unknown): VirtualStats;
    readdir(
      path: string,
      options?: { withFileTypes?: boolean },
    ): Promise<string[] | VirtualDirent[]>;
    readdirSync(
      path: string,
      options?: { withFileTypes?: boolean },
    ): string[] | VirtualDirent[];
    mkdir(
      path: string,
      options?: { recursive?: boolean; mode?: number },
    ): Promise<string | undefined>;
    mkdirSync(
      path: string,
      options?: { recursive?: boolean; mode?: number },
    ): string | undefined;
    rmdir(path: string): Promise<void>;
    rmdirSync(path: string): void;
    unlink(path: string): Promise<void>;
    unlinkSync(path: string): void;
    rename(oldPath: string, newPath: string): Promise<void>;
    renameSync(oldPath: string, newPath: string): void;
    readFile(
      path: string,
      options?: string | { encoding?: string },
    ): Promise<Buffer | string>;
    readFileSync(
      path: string,
      options?: string | { encoding?: string },
    ): Buffer | string;
    writeFile(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): Promise<void>;
    writeFileSync(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): void;
    appendFile(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): Promise<void>;
    appendFileSync(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): void;
    exists(path: string): Promise<boolean>;
    existsSync(path: string): boolean;
    copyFile(src: string, dest: string, mode?: number): Promise<void>;
    copyFileSync(src: string, dest: string, mode?: number): void;
    internalModuleStat(path: string): number;
    realpath(path: string, options?: unknown): Promise<string>;
    realpathSync(path: string, options?: unknown): string;
    access(path: string, mode?: number): Promise<void>;
    accessSync(path: string, mode?: number): void;
    readlink(path: string, options?: unknown): Promise<string>;
    readlinkSync(path: string, options?: unknown): string;
    symlink(target: string, path: string, type?: string): Promise<void>;
    symlinkSync(target: string, path: string, type?: string): void;
  }

  class MemoryProvider extends VirtualProvider {
    constructor();
    setReadOnly(): void;
  }

  interface VirtualFileSystemOptions {
    moduleHooks?: boolean;
    virtualCwd?: boolean;
    overlay?: boolean;
  }

  interface VirtualFileSystemPromises {
    readFile(
      path: string,
      options?: string | { encoding?: string },
    ): Promise<Buffer | string>;
    writeFile(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): Promise<void>;
    appendFile(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): Promise<void>;
    stat(path: string, options?: unknown): Promise<VirtualStats>;
    lstat(path: string, options?: unknown): Promise<VirtualStats>;
    readdir(
      path: string,
      options?: { withFileTypes?: boolean },
    ): Promise<string[] | VirtualDirent[]>;
    mkdir(
      path: string,
      options?: { recursive?: boolean; mode?: number },
    ): Promise<string | undefined>;
    rmdir(path: string): Promise<void>;
    unlink(path: string): Promise<void>;
    rename(oldPath: string, newPath: string): Promise<void>;
    copyFile(src: string, dest: string, mode?: number): Promise<void>;
    realpath(path: string, options?: unknown): Promise<string>;
    readlink(path: string, options?: unknown): Promise<string>;
    symlink(target: string, path: string, type?: string): Promise<void>;
    access(path: string, mode?: number): Promise<void>;
  }

  class VirtualFileSystem {
    constructor(
      providerOrOptions?: VirtualProvider | VirtualFileSystemOptions,
      options?: VirtualFileSystemOptions,
    );
    readonly provider: VirtualProvider;
    readonly mountPoint: string | null;
    readonly mounted: boolean;
    readonly readonly: boolean;
    readonly overlay: boolean;
    readonly virtualCwdEnabled: boolean;
    cwd(): string;
    chdir(dirPath: string): void;
    resolvePath(path: string): string;
    mount(prefix: string): this;
    unmount(): void;
    shouldHandle(path: string): boolean;
    existsSync(path: string): boolean;
    statSync(path: string, options?: unknown): VirtualStats;
    lstatSync(path: string, options?: unknown): VirtualStats;
    readFileSync(
      path: string,
      options?: string | { encoding?: string },
    ): Buffer | string;
    writeFileSync(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): void;
    appendFileSync(
      path: string,
      data: string | Uint8Array,
      options?: { encoding?: string; mode?: number },
    ): void;
    readdirSync(
      path: string,
      options?: { withFileTypes?: boolean },
    ): string[] | VirtualDirent[];
    mkdirSync(
      path: string,
      options?: { recursive?: boolean; mode?: number },
    ): string | undefined;
    rmdirSync(path: string): void;
    unlinkSync(path: string): void;
    renameSync(oldPath: string, newPath: string): void;
    copyFileSync(src: string, dest: string, mode?: number): void;
    realpathSync(path: string, options?: unknown): string;
    readlinkSync(path: string, options?: unknown): string;
    symlinkSync(target: string, path: string, type?: string): void;
    accessSync(path: string, mode?: number): void;
    internalModuleStat(path: string): number;
    openSync(path: string, flags?: string, mode?: number): number;
    closeSync(fd: number): void;
    readSync(
      fd: number,
      buffer: Uint8Array,
      offset: number,
      length: number,
      position: number | null,
    ): number;
    fstatSync(fd: number, options?: unknown): VirtualStats;
    readFile(
      path: string,
      options:
        | string
        | { encoding?: string }
        | ((err: Error | null, data: Buffer | string) => void),
      callback?: (err: Error | null, data: Buffer | string) => void,
    ): void;
    writeFile(
      path: string,
      data: string | Uint8Array,
      options:
        | { encoding?: string; mode?: number }
        | ((err: Error | null) => void),
      callback?: (err: Error | null) => void,
    ): void;
    stat(
      path: string,
      options: unknown,
      callback?: (err: Error | null, stats: VirtualStats) => void,
    ): void;
    lstat(
      path: string,
      options: unknown,
      callback?: (err: Error | null, stats: VirtualStats) => void,
    ): void;
    readdir(
      path: string,
      options:
        | { withFileTypes?: boolean }
        | ((err: Error | null, entries: string[] | VirtualDirent[]) => void),
      callback?: (
        err: Error | null,
        entries: string[] | VirtualDirent[],
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
    fstat(
      fd: number,
      options: unknown,
      callback?: (err: Error | null, stats: VirtualStats) => void,
    ): void;
    createReadStream(
      path: string,
      options?: {
        start?: number;
        end?: number;
        highWaterMark?: number;
        encoding?: string;
        autoClose?: boolean;
      },
    ): Readable;
    readonly promises: VirtualFileSystemPromises;
  }

  function create(
    provider?: VirtualProvider | VirtualFileSystemOptions,
    options?: VirtualFileSystemOptions,
  ): VirtualFileSystem;

  export {
    create,
    MemoryProvider,
    VirtualDirent,
    VirtualFileHandle,
    VirtualFileSystem,
    VirtualProvider,
    VirtualStats,
  };
}
