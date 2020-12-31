// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { relativePath, resolvePath } from "../fs/mod.ts";

const CLOCKID_REALTIME = 0;
const CLOCKID_MONOTONIC = 1;
const CLOCKID_PROCESS_CPUTIME_ID = 2;
const CLOCKID_THREAD_CPUTIME_ID = 3;

const ERRNO_SUCCESS = 0;
const _ERRNO_2BIG = 1;
const ERRNO_ACCES = 2;
const ERRNO_ADDRINUSE = 3;
const ERRNO_ADDRNOTAVAIL = 4;
const _ERRNO_AFNOSUPPORT = 5;
const _ERRNO_AGAIN = 6;
const _ERRNO_ALREADY = 7;
const ERRNO_BADF = 8;
const _ERRNO_BADMSG = 9;
const ERRNO_BUSY = 10;
const _ERRNO_CANCELED = 11;
const _ERRNO_CHILD = 12;
const ERRNO_CONNABORTED = 13;
const ERRNO_CONNREFUSED = 14;
const ERRNO_CONNRESET = 15;
const _ERRNO_DEADLK = 16;
const _ERRNO_DESTADDRREQ = 17;
const _ERRNO_DOM = 18;
const _ERRNO_DQUOT = 19;
const _ERRNO_EXIST = 20;
const _ERRNO_FAULT = 21;
const _ERRNO_FBIG = 22;
const _ERRNO_HOSTUNREACH = 23;
const _ERRNO_IDRM = 24;
const _ERRNO_ILSEQ = 25;
const _ERRNO_INPROGRESS = 26;
const ERRNO_INTR = 27;
const ERRNO_INVAL = 28;
const _ERRNO_IO = 29;
const _ERRNO_ISCONN = 30;
const _ERRNO_ISDIR = 31;
const _ERRNO_LOOP = 32;
const _ERRNO_MFILE = 33;
const _ERRNO_MLINK = 34;
const _ERRNO_MSGSIZE = 35;
const _ERRNO_MULTIHOP = 36;
const _ERRNO_NAMETOOLONG = 37;
const _ERRNO_NETDOWN = 38;
const _ERRNO_NETRESET = 39;
const _ERRNO_NETUNREACH = 40;
const _ERRNO_NFILE = 41;
const _ERRNO_NOBUFS = 42;
const _ERRNO_NODEV = 43;
const ERRNO_NOENT = 44;
const _ERRNO_NOEXEC = 45;
const _ERRNO_NOLCK = 46;
const _ERRNO_NOLINK = 47;
const _ERRNO_NOMEM = 48;
const _ERRNO_NOMSG = 49;
const _ERRNO_NOPROTOOPT = 50;
const _ERRNO_NOSPC = 51;
const ERRNO_NOSYS = 52;
const ERRNO_NOTCONN = 53;
const ERRNO_NOTDIR = 54;
const _ERRNO_NOTEMPTY = 55;
const _ERRNO_NOTRECOVERABLE = 56;
const _ERRNO_NOTSOCK = 57;
const _ERRNO_NOTSUP = 58;
const _ERRNO_NOTTY = 59;
const _ERRNO_NXIO = 60;
const _ERRNO_OVERFLOW = 61;
const _ERRNO_OWNERDEAD = 62;
const _ERRNO_PERM = 63;
const ERRNO_PIPE = 64;
const _ERRNO_PROTO = 65;
const _ERRNO_PROTONOSUPPORT = 66;
const _ERRNO_PROTOTYPE = 67;
const _ERRNO_RANGE = 68;
const _ERRNO_ROFS = 69;
const _ERRNO_SPIPE = 70;
const _ERRNO_SRCH = 71;
const _ERRNO_STALE = 72;
const ERRNO_TIMEDOUT = 73;
const _ERRNO_TXTBSY = 74;
const _ERRNO_XDEV = 75;
const ERRNO_NOTCAPABLE = 76;

const RIGHTS_FD_DATASYNC = 0x0000000000000001n;
const RIGHTS_FD_READ = 0x0000000000000002n;
const _RIGHTS_FD_SEEK = 0x0000000000000004n;
const _RIGHTS_FD_FDSTAT_SET_FLAGS = 0x0000000000000008n;
const _RIGHTS_FD_SYNC = 0x0000000000000010n;
const _RIGHTS_FD_TELL = 0x0000000000000020n;
const RIGHTS_FD_WRITE = 0x0000000000000040n;
const _RIGHTS_FD_ADVISE = 0x0000000000000080n;
const RIGHTS_FD_ALLOCATE = 0x0000000000000100n;
const _RIGHTS_PATH_CREATE_DIRECTORY = 0x0000000000000200n;
const _RIGHTS_PATH_CREATE_FILE = 0x0000000000000400n;
const _RIGHTS_PATH_LINK_SOURCE = 0x0000000000000800n;
const _RIGHTS_PATH_LINK_TARGET = 0x0000000000001000n;
const _RIGHTS_PATH_OPEN = 0x0000000000002000n;
const RIGHTS_FD_READDIR = 0x0000000000004000n;
const _RIGHTS_PATH_READLINK = 0x0000000000008000n;
const _RIGHTS_PATH_RENAME_SOURCE = 0x0000000000010000n;
const _RIGHTS_PATH_RENAME_TARGET = 0x0000000000020000n;
const _RIGHTS_PATH_FILESTAT_GET = 0x0000000000040000n;
const _RIGHTS_PATH_FILESTAT_SET_SIZE = 0x0000000000080000n;
const _RIGHTS_PATH_FILESTAT_SET_TIMES = 0x0000000000100000n;
const _RIGHTS_FD_FILESTAT_GET = 0x0000000000200000n;
const RIGHTS_FD_FILESTAT_SET_SIZE = 0x0000000000400000n;
const _RIGHTS_FD_FILESTAT_SET_TIMES = 0x0000000000800000n;
const _RIGHTS_PATH_SYMLINK = 0x0000000001000000n;
const _RIGHTS_PATH_REMOVE_DIRECTORY = 0x0000000002000000n;
const _RIGHTS_PATH_UNLINK_FILE = 0x0000000004000000n;
const _RIGHTS_POLL_FD_READWRITE = 0x0000000008000000n;
const _RIGHTS_SOCK_SHUTDOWN = 0x0000000010000000n;

const _WHENCE_SET = 0;
const _WHENCE_CUR = 1;
const _WHENCE_END = 2;

const FILETYPE_UNKNOWN = 0;
const _FILETYPE_BLOCK_DEVICE = 1;
const FILETYPE_CHARACTER_DEVICE = 2;
const FILETYPE_DIRECTORY = 3;
const FILETYPE_REGULAR_FILE = 4;
const _FILETYPE_SOCKET_DGRAM = 5;
const _FILETYPE_SOCKET_STREAM = 6;
const FILETYPE_SYMBOLIC_LINK = 7;

const _ADVICE_NORMAL = 0;
const _ADVICE_SEQUENTIAL = 1;
const _ADVICE_RANDOM = 2;
const _ADVICE_WILLNEED = 3;
const _ADVICE_DONTNEED = 4;
const _ADVICE_NOREUSE = 5;

const FDFLAGS_APPEND = 0x0001;
const FDFLAGS_DSYNC = 0x0002;
const FDFLAGS_NONBLOCK = 0x0004;
const FDFLAGS_RSYNC = 0x0008;
const FDFLAGS_SYNC = 0x0010;

const _FSTFLAGS_ATIM = 0x0001;
const FSTFLAGS_ATIM_NOW = 0x0002;
const _FSTFLAGS_MTIM = 0x0004;
const FSTFLAGS_MTIM_NOW = 0x0008;

const LOOKUPFLAGS_SYMLINK_FOLLOW = 0x0001;

const OFLAGS_CREAT = 0x0001;
const OFLAGS_DIRECTORY = 0x0002;
const OFLAGS_EXCL = 0x0004;
const OFLAGS_TRUNC = 0x0008;

const _EVENTTYPE_CLOCK = 0;
const _EVENTTYPE_FD_READ = 1;
const _EVENTTYPE_FD_WRITE = 2;

const _EVENTRWFLAGS_FD_READWRITE_HANGUP = 1;
const _SUBCLOCKFLAGS_SUBSCRIPTION_CLOCK_ABSTIME = 1;

const _SIGNAL_NONE = 0;
const _SIGNAL_HUP = 1;
const _SIGNAL_INT = 2;
const _SIGNAL_QUIT = 3;
const _SIGNAL_ILL = 4;
const _SIGNAL_TRAP = 5;
const _SIGNAL_ABRT = 6;
const _SIGNAL_BUS = 7;
const _SIGNAL_FPE = 8;
const _SIGNAL_KILL = 9;
const _SIGNAL_USR1 = 10;
const _SIGNAL_SEGV = 11;
const _SIGNAL_USR2 = 12;
const _SIGNAL_PIPE = 13;
const _SIGNAL_ALRM = 14;
const _SIGNAL_TERM = 15;
const _SIGNAL_CHLD = 16;
const _SIGNAL_CONT = 17;
const _SIGNAL_STOP = 18;
const _SIGNAL_TSTP = 19;
const _SIGNAL_TTIN = 20;
const _SIGNAL_TTOU = 21;
const _SIGNAL_URG = 22;
const _SIGNAL_XCPU = 23;
const _SIGNAL_XFSZ = 24;
const _SIGNAL_VTALRM = 25;
const _SIGNAL_PROF = 26;
const _SIGNAL_WINCH = 27;
const _SIGNAL_POLL = 28;
const _SIGNAL_PWR = 29;
const _SIGNAL_SYS = 30;

const _RIFLAGS_RECV_PEEK = 0x0001;
const _RIFLAGS_RECV_WAITALL = 0x0002;

const _ROFLAGS_RECV_DATA_TRUNCATED = 0x0001;

const _SDFLAGS_RD = 0x0001;
const _SDFLAGS_WR = 0x0002;

const PREOPENTYPE_DIR = 0;

function syscall<T extends CallableFunction>(target: T) {
  return function (...args: unknown[]) {
    try {
      return target(...args);
    } catch (err) {
      if (err instanceof ExitStatus) {
        throw err;
      }

      switch (err.name) {
        case "NotFound":
          return ERRNO_NOENT;

        case "PermissionDenied":
          return ERRNO_ACCES;

        case "ConnectionRefused":
          return ERRNO_CONNREFUSED;

        case "ConnectionReset":
          return ERRNO_CONNRESET;

        case "ConnectionAborted":
          return ERRNO_CONNABORTED;

        case "NotConnected":
          return ERRNO_NOTCONN;

        case "AddrInUse":
          return ERRNO_ADDRINUSE;

        case "AddrNotAvailable":
          return ERRNO_ADDRNOTAVAIL;

        case "BrokenPipe":
          return ERRNO_PIPE;

        case "InvalidData":
          return ERRNO_INVAL;

        case "TimedOut":
          return ERRNO_TIMEDOUT;

        case "Interrupted":
          return ERRNO_INTR;

        case "BadResource":
          return ERRNO_BADF;

        case "Busy":
          return ERRNO_BUSY;

        default:
          return ERRNO_INVAL;
      }
    }
  };
}

interface FileDescriptor {
  rid?: number;
  type?: number;
  flags?: number;
  path?: string;
  vpath?: string;
  entries?: Deno.DirEntry[];
}

export class ExitStatus {
  code: number;

  constructor(code: number) {
    this.code = code;
  }
}

export interface ContextOptions {
  /**
   * An array of strings that the WebAssembly instance will see as command-line
   * arguments.
   *
   * The first argument is the virtual path to the command itself.
   */
  args?: string[];

  /**
   * An object of string keys mapped to string values that the WebAssembly module will see as its environment.
   */
  env?: { [key: string]: string | undefined };

  /**
   * An object of string keys mapped to string values that the WebAssembly module will see as it's filesystem.
   *
   * The string keys of are treated as directories within the sandboxed
   * filesystem, the values are the real paths to those directories on the host
   * machine.
   *
   */
  preopens?: { [key: string]: string };

  /**
   * Determines if calls to exit from within the WebAssembly module will terminate the proess or return.
   */
  exitOnReturn?: boolean;
}

/**
 * The Context class provides the environment required to run WebAssembly
 * modules compiled to run with the WebAssembly System Interface.
 *
 * Each context represents a distinct sandboxed environment and must have its
 * command-line arguments, environment variables, and pre-opened directory
 * structure configured explicitly.
 */
export default class Context {
  args: string[];
  env: { [key: string]: string | undefined };
  exitOnReturn: boolean;
  memory: WebAssembly.Memory;

  fds: FileDescriptor[];

  exports: Record<string, WebAssembly.ImportValue>;
  #started: boolean;

  constructor(options: ContextOptions) {
    this.args = options.args ?? [];
    this.env = options.env ?? {};
    this.exitOnReturn = options.exitOnReturn ?? true;
    this.memory = null!;

    this.fds = [
      {
        rid: Deno.stdin.rid,
        type: FILETYPE_CHARACTER_DEVICE,
        flags: FDFLAGS_APPEND,
      },
      {
        rid: Deno.stdout.rid,
        type: FILETYPE_CHARACTER_DEVICE,
        flags: FDFLAGS_APPEND,
      },
      {
        rid: Deno.stderr.rid,
        type: FILETYPE_CHARACTER_DEVICE,
        flags: FDFLAGS_APPEND,
      },
    ];

    if (options.preopens) {
      for (const [vpath, path] of Object.entries(options.preopens)) {
        const type = FILETYPE_DIRECTORY;
        const entries = Array.from(Deno.readDirSync(path));

        const entry = {
          type,
          entries,
          path,
          vpath,
        };

        this.fds.push(entry);
      }
    }

    this.exports = {
      "args_get": syscall((
        argvOffset: number,
        argvBufferOffset: number,
      ): number => {
        const args = this.args;
        const textEncoder = new TextEncoder();
        const memoryData = new Uint8Array(this.memory.buffer);
        const memoryView = new DataView(this.memory.buffer);

        for (const arg of args) {
          memoryView.setUint32(argvOffset, argvBufferOffset, true);
          argvOffset += 4;

          const data = textEncoder.encode(`${arg}\0`);
          memoryData.set(data, argvBufferOffset);
          argvBufferOffset += data.length;
        }

        return ERRNO_SUCCESS;
      }),

      "args_sizes_get": syscall((
        argcOffset: number,
        argvBufferSizeOffset: number,
      ): number => {
        const args = this.args;
        const textEncoder = new TextEncoder();
        const memoryView = new DataView(this.memory.buffer);

        memoryView.setUint32(argcOffset, args.length, true);
        memoryView.setUint32(
          argvBufferSizeOffset,
          args.reduce(function (acc, arg) {
            return acc + textEncoder.encode(`${arg}\0`).length;
          }, 0),
          true,
        );

        return ERRNO_SUCCESS;
      }),

      "environ_get": syscall((
        environOffset: number,
        environBufferOffset: number,
      ): number => {
        const entries = Object.entries(this.env);
        const textEncoder = new TextEncoder();
        const memoryData = new Uint8Array(this.memory.buffer);
        const memoryView = new DataView(this.memory.buffer);

        for (const [key, value] of entries) {
          memoryView.setUint32(environOffset, environBufferOffset, true);
          environOffset += 4;

          const data = textEncoder.encode(`${key}=${value}\0`);
          memoryData.set(data, environBufferOffset);
          environBufferOffset += data.length;
        }

        return ERRNO_SUCCESS;
      }),

      "environ_sizes_get": syscall((
        environcOffset: number,
        environBufferSizeOffset: number,
      ): number => {
        const entries = Object.entries(this.env);
        const textEncoder = new TextEncoder();
        const memoryView = new DataView(this.memory.buffer);

        memoryView.setUint32(environcOffset, entries.length, true);
        memoryView.setUint32(
          environBufferSizeOffset,
          entries.reduce(function (acc, [key, value]) {
            return acc + textEncoder.encode(`${key}=${value}\0`).length;
          }, 0),
          true,
        );

        return ERRNO_SUCCESS;
      }),

      "clock_res_get": syscall((
        id: number,
        resolutionOffset: number,
      ): number => {
        const memoryView = new DataView(this.memory.buffer);

        switch (id) {
          case CLOCKID_REALTIME: {
            const resolution = BigInt(1e6);

            memoryView.setBigUint64(
              resolutionOffset,
              resolution,
              true,
            );
            break;
          }

          case CLOCKID_MONOTONIC:
          case CLOCKID_PROCESS_CPUTIME_ID:
          case CLOCKID_THREAD_CPUTIME_ID: {
            const resolution = BigInt(1e3);
            memoryView.setBigUint64(resolutionOffset, resolution, true);
            break;
          }

          default:
            return ERRNO_INVAL;
        }

        return ERRNO_SUCCESS;
      }),

      "clock_time_get": syscall((
        id: number,
        precision: bigint,
        timeOffset: number,
      ): number => {
        const memoryView = new DataView(this.memory.buffer);

        switch (id) {
          case CLOCKID_REALTIME: {
            const time = BigInt(Date.now()) * BigInt(1e6);
            memoryView.setBigUint64(timeOffset, time, true);
            break;
          }

          case CLOCKID_MONOTONIC:
          case CLOCKID_PROCESS_CPUTIME_ID:
          case CLOCKID_THREAD_CPUTIME_ID: {
            const t = performance.now();
            const s = Math.trunc(t);
            const ms = Math.floor((t - s) * 1e3);

            const time = BigInt(s) * BigInt(1e9) + BigInt(ms) * BigInt(1e6);

            memoryView.setBigUint64(timeOffset, time, true);
            break;
          }

          default:
            return ERRNO_INVAL;
        }

        return ERRNO_SUCCESS;
      }),

      "fd_advise": syscall((
        _fd: number,
        _offset: bigint,
        _length: bigint,
        _advice: number,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "fd_allocate": syscall((
        _fd: number,
        _offset: bigint,
        _length: bigint,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "fd_close": syscall((
        fd: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (entry.rid) {
          Deno.close(entry.rid);
        }

        delete this.fds[fd];

        return ERRNO_SUCCESS;
      }),

      "fd_datasync": syscall((
        fd: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        Deno.fdatasyncSync(entry.rid!);

        return ERRNO_SUCCESS;
      }),

      "fd_fdstat_get": syscall((
        fd: number,
        offset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);
        memoryView.setUint8(offset, entry.type!);
        memoryView.setUint16(offset + 2, entry.flags!, true);
        memoryView.setBigUint64(offset + 8, 0n, true); // TODO
        memoryView.setBigUint64(offset + 16, 0n, true); // TODO

        return ERRNO_SUCCESS;
      }),

      "fd_fdstat_set_flags": syscall((
        _fd: number,
        _flags: number,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "fd_fdstat_set_rights": syscall((
        _fd: number,
        _rightsBase: bigint,
        _rightsInheriting: bigint,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "fd_filestat_get": syscall((
        fd: number,
        offset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);

        const info = Deno.fstatSync(entry.rid!);

        if (entry.type === undefined) {
          switch (true) {
            case info.isFile:
              entry.type = FILETYPE_REGULAR_FILE;
              break;

            case info.isDirectory:
              entry.type = FILETYPE_DIRECTORY;
              break;

            case info.isSymlink:
              entry.type = FILETYPE_SYMBOLIC_LINK;
              break;

            default:
              entry.type = FILETYPE_UNKNOWN;
              break;
          }
        }

        memoryView.setBigUint64(offset, BigInt(info.dev ? info.dev : 0), true);
        offset += 8;

        memoryView.setBigUint64(offset, BigInt(info.ino ? info.ino : 0), true);
        offset += 8;

        memoryView.setUint8(offset, entry.type);
        offset += 8;

        memoryView.setUint32(offset, Number(info.nlink), true);
        offset += 8;

        memoryView.setBigUint64(offset, BigInt(info.size), true);
        offset += 8;

        memoryView.setBigUint64(
          offset,
          BigInt(info.atime ? info.atime.getTime() * 1e6 : 0),
          true,
        );
        offset += 8;

        memoryView.setBigUint64(
          offset,
          BigInt(info.mtime ? info.mtime.getTime() * 1e6 : 0),
          true,
        );
        offset += 8;

        memoryView.setBigUint64(
          offset,
          BigInt(info.birthtime ? info.birthtime.getTime() * 1e6 : 0),
          true,
        );
        offset += 8;

        return ERRNO_SUCCESS;
      }),

      "fd_filestat_set_size": syscall((
        fd: number,
        size: bigint,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        Deno.ftruncateSync(entry.rid!, Number(size));

        return ERRNO_SUCCESS;
      }),

      "fd_filestat_set_times": syscall((
        fd: number,
        atim: bigint,
        mtim: bigint,
        flags: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        if ((flags & FSTFLAGS_ATIM_NOW) == FSTFLAGS_ATIM_NOW) {
          atim = BigInt(Date.now() * 1e6);
        }

        if ((flags & FSTFLAGS_MTIM_NOW) == FSTFLAGS_MTIM_NOW) {
          mtim = BigInt(Date.now() * 1e6);
        }

        Deno.utimeSync(entry.path!, Number(atim), Number(mtim));

        return ERRNO_SUCCESS;
      }),

      "fd_pread": syscall((
        fd: number,
        iovsOffset: number,
        iovsLength: number,
        offset: bigint,
        nreadOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (entry == null) {
          return ERRNO_BADF;
        }

        const seek = Deno.seekSync(entry.rid!, 0, Deno.SeekMode.Current);
        const memoryView = new DataView(this.memory.buffer);

        let nread = 0;
        for (let i = 0; i < iovsLength; i++) {
          const dataOffset = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const dataLength = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const data = new Uint8Array(
            this.memory.buffer,
            dataOffset,
            dataLength,
          );
          nread += Deno.readSync(entry.rid!, data) as number;
        }

        Deno.seekSync(entry.rid!, seek, Deno.SeekMode.Start);
        memoryView.setUint32(nreadOffset, nread, true);

        return ERRNO_SUCCESS;
      }),

      "fd_prestat_get": syscall((
        fd: number,
        prestatOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.vpath) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);
        memoryView.setUint8(prestatOffset, PREOPENTYPE_DIR);
        memoryView.setUint32(
          prestatOffset + 4,
          new TextEncoder().encode(entry.vpath).byteLength,
          true,
        );

        return ERRNO_SUCCESS;
      }),

      "fd_prestat_dir_name": syscall((
        fd: number,
        pathOffset: number,
        pathLength: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.vpath) {
          return ERRNO_BADF;
        }

        const data = new Uint8Array(this.memory.buffer, pathOffset, pathLength);
        data.set(new TextEncoder().encode(entry.vpath));

        return ERRNO_SUCCESS;
      }),

      "fd_pwrite": syscall((
        fd: number,
        iovsOffset: number,
        iovsLength: number,
        offset: bigint,
        nwrittenOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const seek = Deno.seekSync(entry.rid!, 0, Deno.SeekMode.Current);
        const memoryView = new DataView(this.memory.buffer);

        let nwritten = 0;
        for (let i = 0; i < iovsLength; i++) {
          const dataOffset = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const dataLength = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const data = new Uint8Array(
            this.memory.buffer,
            dataOffset,
            dataLength,
          );
          nwritten += Deno.writeSync(entry.rid!, data) as number;
        }

        Deno.seekSync(entry.rid!, seek, Deno.SeekMode.Start);
        memoryView.setUint32(nwrittenOffset, nwritten, true);

        return ERRNO_SUCCESS;
      }),

      "fd_read": syscall((
        fd: number,
        iovsOffset: number,
        iovsLength: number,
        nreadOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);

        let nread = 0;
        for (let i = 0; i < iovsLength; i++) {
          const dataOffset = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const dataLength = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const data = new Uint8Array(
            this.memory.buffer,
            dataOffset,
            dataLength,
          );
          nread += Deno.readSync(entry.rid!, data) as number;
        }

        memoryView.setUint32(nreadOffset, nread, true);

        return ERRNO_SUCCESS;
      }),

      "fd_readdir": syscall((
        fd: number,
        bufferOffset: number,
        bufferLength: number,
        cookie: bigint,
        bufferUsedOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryData = new Uint8Array(this.memory.buffer);
        const memoryView = new DataView(this.memory.buffer);

        let bufferUsed = 0;

        const entries = Array.from(Deno.readDirSync(entry.path!));
        for (let i = Number(cookie); i < entries.length; i++) {
          const nameData = new TextEncoder().encode(entries[i].name);

          const entryInfo = Deno.statSync(
            resolvePath(entry.path!, entries[i].name),
          );
          const entryData = new Uint8Array(24 + nameData.byteLength);
          const entryView = new DataView(entryData.buffer);

          entryView.setBigUint64(0, BigInt(i + 1), true);
          entryView.setBigUint64(
            8,
            BigInt(entryInfo.ino ? entryInfo.ino : 0),
            true,
          );
          entryView.setUint32(16, nameData.byteLength, true);

          let type: number;
          switch (true) {
            case entries[i].isFile:
              type = FILETYPE_REGULAR_FILE;
              break;

            case entries[i].isDirectory:
              type = FILETYPE_REGULAR_FILE;
              break;

            case entries[i].isSymlink:
              type = FILETYPE_SYMBOLIC_LINK;
              break;

            default:
              type = FILETYPE_REGULAR_FILE;
              break;
          }

          entryView.setUint8(20, type);
          entryData.set(nameData, 24);

          const data = entryData.slice(
            0,
            Math.min(entryData.length, bufferLength - bufferUsed),
          );
          memoryData.set(data, bufferOffset + bufferUsed);
          bufferUsed += data.byteLength;
        }

        memoryView.setUint32(bufferUsedOffset, bufferUsed, true);

        return ERRNO_SUCCESS;
      }),

      "fd_renumber": syscall((
        fd: number,
        to: number,
      ): number => {
        if (!this.fds[fd]) {
          return ERRNO_BADF;
        }

        if (!this.fds[to]) {
          return ERRNO_BADF;
        }

        if (this.fds[to].rid) {
          Deno.close(this.fds[to].rid!);
        }

        this.fds[to] = this.fds[fd];
        delete this.fds[fd];

        return ERRNO_SUCCESS;
      }),

      "fd_seek": syscall((
        fd: number,
        offset: bigint,
        whence: number,
        newOffsetOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);

        // FIXME Deno does not support seeking with big integers
        const newOffset = Deno.seekSync(entry.rid!, Number(offset), whence);
        memoryView.setBigUint64(newOffsetOffset, BigInt(newOffset), true);

        return ERRNO_SUCCESS;
      }),

      "fd_sync": syscall((
        fd: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        Deno.fsyncSync(entry.rid!);

        return ERRNO_SUCCESS;
      }),

      "fd_tell": syscall((
        fd: number,
        offsetOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);

        const offset = Deno.seekSync(entry.rid!, 0, Deno.SeekMode.Current);
        memoryView.setBigUint64(offsetOffset, BigInt(offset), true);

        return ERRNO_SUCCESS;
      }),

      "fd_write": syscall((
        fd: number,
        iovsOffset: number,
        iovsLength: number,
        nwrittenOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        const memoryView = new DataView(this.memory.buffer);

        let nwritten = 0;
        for (let i = 0; i < iovsLength; i++) {
          const dataOffset = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const dataLength = memoryView.getUint32(iovsOffset, true);
          iovsOffset += 4;

          const data = new Uint8Array(
            this.memory.buffer,
            dataOffset,
            dataLength,
          );
          nwritten += Deno.writeSync(entry.rid!, data) as number;
        }

        memoryView.setUint32(nwrittenOffset, nwritten, true);

        return ERRNO_SUCCESS;
      }),

      "path_create_directory": syscall((
        fd: number,
        pathOffset: number,
        pathLength: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const data = new Uint8Array(this.memory.buffer, pathOffset, pathLength);
        const path = resolvePath(entry.path!, textDecoder.decode(data));

        Deno.mkdirSync(path);

        return ERRNO_SUCCESS;
      }),

      "path_filestat_get": syscall((
        fd: number,
        flags: number,
        pathOffset: number,
        pathLength: number,
        bufferOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const data = new Uint8Array(this.memory.buffer, pathOffset, pathLength);
        const path = resolvePath(entry.path!, textDecoder.decode(data));

        const memoryView = new DataView(this.memory.buffer);

        const info = (flags & LOOKUPFLAGS_SYMLINK_FOLLOW) != 0
          ? Deno.statSync(path)
          : Deno.lstatSync(path);

        memoryView.setBigUint64(
          bufferOffset,
          BigInt(info.dev ? info.dev : 0),
          true,
        );
        bufferOffset += 8;

        memoryView.setBigUint64(
          bufferOffset,
          BigInt(info.ino ? info.ino : 0),
          true,
        );
        bufferOffset += 8;

        switch (true) {
          case info.isFile:
            memoryView.setUint8(bufferOffset, FILETYPE_REGULAR_FILE);
            bufferOffset += 8;
            break;

          case info.isDirectory:
            memoryView.setUint8(bufferOffset, FILETYPE_DIRECTORY);
            bufferOffset += 8;
            break;

          case info.isSymlink:
            memoryView.setUint8(bufferOffset, FILETYPE_SYMBOLIC_LINK);
            bufferOffset += 8;
            break;

          default:
            memoryView.setUint8(bufferOffset, FILETYPE_UNKNOWN);
            bufferOffset += 8;
            break;
        }

        memoryView.setUint32(bufferOffset, Number(info.nlink), true);
        bufferOffset += 8;

        memoryView.setBigUint64(bufferOffset, BigInt(info.size), true);
        bufferOffset += 8;

        memoryView.setBigUint64(
          bufferOffset,
          BigInt(info.atime ? info.atime.getTime() * 1e6 : 0),
          true,
        );
        bufferOffset += 8;

        memoryView.setBigUint64(
          bufferOffset,
          BigInt(info.mtime ? info.mtime.getTime() * 1e6 : 0),
          true,
        );
        bufferOffset += 8;

        memoryView.setBigUint64(
          bufferOffset,
          BigInt(info.birthtime ? info.birthtime.getTime() * 1e6 : 0),
          true,
        );
        bufferOffset += 8;

        return ERRNO_SUCCESS;
      }),

      "path_filestat_set_times": syscall((
        fd: number,
        flags: number,
        pathOffset: number,
        pathLength: number,
        atim: bigint,
        mtim: bigint,
        fstflags: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const data = new Uint8Array(this.memory.buffer, pathOffset, pathLength);
        const path = resolvePath(entry.path!, textDecoder.decode(data));

        if ((fstflags & FSTFLAGS_ATIM_NOW) == FSTFLAGS_ATIM_NOW) {
          atim = BigInt(Date.now()) * BigInt(1e6);
        }

        if ((fstflags & FSTFLAGS_MTIM_NOW) == FSTFLAGS_MTIM_NOW) {
          mtim = BigInt(Date.now()) * BigInt(1e6);
        }

        Deno.utimeSync(path, Number(atim), Number(mtim));

        return ERRNO_SUCCESS;
      }),

      "path_link": syscall((
        oldFd: number,
        oldFlags: number,
        oldPathOffset: number,
        oldPathLength: number,
        newFd: number,
        newPathOffset: number,
        newPathLength: number,
      ): number => {
        const oldEntry = this.fds[oldFd];
        const newEntry = this.fds[newFd];
        if (!oldEntry || !newEntry) {
          return ERRNO_BADF;
        }

        if (!oldEntry.path || !newEntry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const oldData = new Uint8Array(
          this.memory.buffer,
          oldPathOffset,
          oldPathLength,
        );
        const oldPath = resolvePath(
          oldEntry.path!,
          textDecoder.decode(oldData),
        );
        const newData = new Uint8Array(
          this.memory.buffer,
          newPathOffset,
          newPathLength,
        );
        const newPath = resolvePath(
          newEntry.path!,
          textDecoder.decode(newData),
        );

        Deno.linkSync(oldPath, newPath);

        return ERRNO_SUCCESS;
      }),

      "path_open": syscall((
        fd: number,
        dirflags: number,
        pathOffset: number,
        pathLength: number,
        oflags: number,
        rightsBase: bigint,
        rightsInheriting: bigint,
        fdflags: number,
        openedFdOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const pathData = new Uint8Array(
          this.memory.buffer,
          pathOffset,
          pathLength,
        );
        const resolvedPath = resolvePath(
          entry.path!,
          textDecoder.decode(pathData),
        );

        if (relativePath(entry.path, resolvedPath).startsWith("..")) {
          return ERRNO_NOTCAPABLE;
        }

        let path;
        if (
          (dirflags & LOOKUPFLAGS_SYMLINK_FOLLOW) == LOOKUPFLAGS_SYMLINK_FOLLOW
        ) {
          try {
            path = Deno.realPathSync(resolvedPath);
            if (relativePath(entry.path, path).startsWith("..")) {
              return ERRNO_NOTCAPABLE;
            }
          } catch (_err) {
            path = resolvedPath;
          }
        } else {
          path = resolvedPath;
        }

        if ((oflags & OFLAGS_DIRECTORY) !== 0) {
          // XXX (caspervonb) this isn't ideal as we can't get a rid for the
          // directory this way so there's no native fstat but Deno.open
          // doesn't work with directories on windows so we'll have to work
          // around it for now.
          const entries = Array.from(Deno.readDirSync(path));
          const openedFd = this.fds.push({
            flags: fdflags,
            path,
            entries,
          }) - 1;

          const memoryView = new DataView(this.memory.buffer);
          memoryView.setUint32(openedFdOffset, openedFd, true);

          return ERRNO_SUCCESS;
        }

        const options = {
          read: false,
          write: false,
          append: false,
          truncate: false,
          create: false,
          createNew: false,
        };

        if ((oflags & OFLAGS_CREAT) !== 0) {
          options.create = true;
          options.write = true;
        }

        if ((oflags & OFLAGS_EXCL) !== 0) {
          options.createNew = true;
        }

        if ((oflags & OFLAGS_TRUNC) !== 0) {
          options.truncate = true;
          options.write = true;
        }

        const read = (
          RIGHTS_FD_READ |
          RIGHTS_FD_READDIR
        );

        if ((rightsBase & read) != 0n) {
          options.read = true;
        }

        const write = (
          RIGHTS_FD_DATASYNC |
          RIGHTS_FD_WRITE |
          RIGHTS_FD_ALLOCATE |
          RIGHTS_FD_FILESTAT_SET_SIZE
        );

        if ((rightsBase & write) != 0n) {
          options.write = true;
        }

        if ((fdflags & FDFLAGS_APPEND) != 0) {
          options.append = true;
        }

        if ((fdflags & FDFLAGS_DSYNC) != 0) {
          // TODO (caspervonb) review if we can emulate this.
        }

        if ((fdflags & FDFLAGS_NONBLOCK) != 0) {
          // TODO (caspervonb) review if we can emulate this.
        }

        if ((fdflags & FDFLAGS_RSYNC) != 0) {
          // TODO (caspervonb) review if we can emulate this.
        }

        if ((fdflags & FDFLAGS_SYNC) != 0) {
          // TODO (caspervonb) review if we can emulate this.
        }

        if (!options.read && !options.write && !options.truncate) {
          options.read = true;
        }

        const { rid } = Deno.openSync(path, options);
        const openedFd = this.fds.push({
          rid,
          flags: fdflags,
          path,
        }) - 1;

        const memoryView = new DataView(this.memory.buffer);
        memoryView.setUint32(openedFdOffset, openedFd, true);

        return ERRNO_SUCCESS;
      }),

      "path_readlink": syscall((
        fd: number,
        pathOffset: number,
        pathLength: number,
        bufferOffset: number,
        bufferLength: number,
        bufferUsedOffset: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const memoryData = new Uint8Array(this.memory.buffer);
        const memoryView = new DataView(this.memory.buffer);

        const pathData = new Uint8Array(
          this.memory.buffer,
          pathOffset,
          pathLength,
        );
        const path = resolvePath(
          entry.path!,
          new TextDecoder().decode(pathData),
        );

        const link = Deno.readLinkSync(path);
        const linkData = new TextEncoder().encode(link);
        memoryData.set(new Uint8Array(linkData, 0, bufferLength), bufferOffset);

        const bufferUsed = Math.min(linkData.byteLength, bufferLength);
        memoryView.setUint32(bufferUsedOffset, bufferUsed, true);

        return ERRNO_SUCCESS;
      }),

      "path_remove_directory": syscall((
        fd: number,
        pathOffset: number,
        pathLength: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const data = new Uint8Array(this.memory.buffer, pathOffset, pathLength);
        const path = resolvePath(entry.path!, textDecoder.decode(data));

        if (!Deno.statSync(path).isDirectory) {
          return ERRNO_NOTDIR;
        }

        Deno.removeSync(path);

        return ERRNO_SUCCESS;
      }),

      "path_rename": syscall((
        fd: number,
        oldPathOffset: number,
        oldPathLength: number,
        newFd: number,
        newPathOffset: number,
        newPathLength: number,
      ): number => {
        const oldEntry = this.fds[fd];
        const newEntry = this.fds[newFd];
        if (!oldEntry || !newEntry) {
          return ERRNO_BADF;
        }

        if (!oldEntry.path || !newEntry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const oldData = new Uint8Array(
          this.memory.buffer,
          oldPathOffset,
          oldPathLength,
        );
        const oldPath = resolvePath(
          oldEntry.path!,
          textDecoder.decode(oldData),
        );
        const newData = new Uint8Array(
          this.memory.buffer,
          newPathOffset,
          newPathLength,
        );
        const newPath = resolvePath(
          newEntry.path!,
          textDecoder.decode(newData),
        );

        Deno.renameSync(oldPath, newPath);

        return ERRNO_SUCCESS;
      }),

      "path_symlink": syscall((
        oldPathOffset: number,
        oldPathLength: number,
        fd: number,
        newPathOffset: number,
        newPathLength: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const oldData = new Uint8Array(
          this.memory.buffer,
          oldPathOffset,
          oldPathLength,
        );
        const oldPath = textDecoder.decode(oldData);
        const newData = new Uint8Array(
          this.memory.buffer,
          newPathOffset,
          newPathLength,
        );
        const newPath = resolvePath(entry.path!, textDecoder.decode(newData));

        Deno.symlinkSync(oldPath, newPath);

        return ERRNO_SUCCESS;
      }),

      "path_unlink_file": syscall((
        fd: number,
        pathOffset: number,
        pathLength: number,
      ): number => {
        const entry = this.fds[fd];
        if (!entry) {
          return ERRNO_BADF;
        }

        if (!entry.path) {
          return ERRNO_INVAL;
        }

        const textDecoder = new TextDecoder();
        const data = new Uint8Array(this.memory.buffer, pathOffset, pathLength);
        const path = resolvePath(entry.path!, textDecoder.decode(data));

        Deno.removeSync(path);

        return ERRNO_SUCCESS;
      }),

      "poll_oneoff": syscall((
        _inOffset: number,
        _outOffset: number,
        _nsubscriptions: number,
        _neventsOffset: number,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "proc_exit": syscall((
        rval: number,
      ): never => {
        if (this.exitOnReturn) {
          Deno.exit(rval);
        }

        throw new ExitStatus(rval);
      }),

      "proc_raise": syscall((
        _sig: number,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "sched_yield": syscall((): number => {
        return ERRNO_SUCCESS;
      }),

      "random_get": syscall((
        bufferOffset: number,
        bufferLength: number,
      ): number => {
        const buffer = new Uint8Array(
          this.memory.buffer,
          bufferOffset,
          bufferLength,
        );
        crypto.getRandomValues(buffer);

        return ERRNO_SUCCESS;
      }),

      "sock_recv": syscall((
        _fd: number,
        _riDataOffset: number,
        _riDataLength: number,
        _riFlags: number,
        _roDataLengthOffset: number,
        _roFlagsOffset: number,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "sock_send": syscall((
        _fd: number,
        _siDataOffset: number,
        _siDataLength: number,
        _siFlags: number,
        _soDataLengthOffset: number,
      ): number => {
        return ERRNO_NOSYS;
      }),

      "sock_shutdown": syscall((
        _fd: number,
        _how: number,
      ): number => {
        return ERRNO_NOSYS;
      }),
    };

    this.#started = false;
  }

  /**
   * Attempt to begin execution of instance as a command by invoking its
   * _start() export.
   *
   * If the instance does not contain a _start() export, or if the instance
   * contains an _initialize export an error will be thrown.
   *
   * The instance must also have a WebAssembly.Memory export named "memory"
   * which will be used as the address space, if it does not an error will be
   * thrown.
   */
  start(instance: WebAssembly.Instance) {
    if (this.#started) {
      throw new Error("WebAssembly.Instance has already started");
    }

    this.#started = true;

    const { _start, _initialize, memory } = instance.exports;

    if (!(memory instanceof WebAssembly.Memory)) {
      throw new TypeError("WebAsembly.instance must provide a memory export");
    }

    this.memory = memory;

    if (typeof _initialize == "function") {
      throw new TypeError(
        "WebAsembly.instance export _initialize must not be a function",
      );
    }

    if (typeof _start != "function") {
      throw new TypeError(
        "WebAssembly.Instance export _start must be a function",
      );
    }

    _start();
  }

  /**
   * Attempt to initialize instance as a reactor by invoking its _initialize() export.
   *
   * If instance contains a _start() export, then an exception is thrown.
   *
   * The instance must also have a WebAssembly.Memory export named "memory"
   * which will be used as the address space, if it does not an error will be
   * thrown.
   */
  initialize(instance: WebAssembly.Instance) {
    if (this.#started) {
      throw new Error("WebAssembly.Instance has already started");
    }

    this.#started = true;

    const { _start, _initialize, memory } = instance.exports;

    if (!(memory instanceof WebAssembly.Memory)) {
      throw new TypeError("WebAsembly.instance must provide a memory export");
    }

    this.memory = memory;

    if (typeof _start == "function") {
      throw new TypeError(
        "WebAssembly.Instance export _start must not be a function",
      );
    }

    if (typeof _initialize != "function") {
      throw new TypeError(
        "WebAsembly.instance export _initialize must be a function",
      );
    }

    _initialize();
  }
}
