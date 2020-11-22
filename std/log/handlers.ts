// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { LogLevel, logLevels } from "./levels.ts";
import type { LogRecord } from "./logger.ts";
import { blue, red, yellow } from "../fmt/colors.ts";
import { existsSync } from "../fs/exists.ts";
import { BufWriterSync } from "../io/bufio.ts";

function defaultFormatter({ logLevel, message, args }: LogRecord) {
  return `${logLevel.name} ${[message, ...args].join(" ")}`;
}

type FormatterFunction = (logRecord: LogRecord) => string;
type LogMode = "a" | "w" | "x";

interface HandlerOptions {
  formatter?: FormatterFunction;
}

type HandlerFunctions = {
  [code: number]: (message: string) => void;
};

export class Handler {
  readonly logLevel: LogLevel;
  readonly formatter: FormatterFunction;
  readonly handlerFunctions: HandlerFunctions = {};

  constructor(
    logLevel: LogLevel,
    { formatter = defaultFormatter }: HandlerOptions = {},
  ) {
    this.logLevel = logLevel;
    this.formatter = formatter;
  }

  addLogLevel(logLevel: LogLevel, fn: (message: string) => void) {
    this.handlerFunctions[logLevel.code] = fn;
  }
  deleteLogLevel(logLevel: LogLevel) {
    delete this.handlerFunctions[logLevel.code];
  }

  handle(logRecord: LogRecord): void {
    if (this.logLevel.code > logRecord.logLevel.code) return;
    const fn = this.handlerFunctions[logRecord.logLevel.code];
    if (!fn) {
      throw Error(
        `logLevel ${logRecord.logLevel.code} ${logRecord.logLevel.name} is not supported.`,
      );
    }
    const message = this.formatter(logRecord);
    fn(message);
  }
}

export class ConsoleHandler extends Handler {
  handlerFunctions = {
    [logLevels.trace.code]: (message: string) => console.log(message),
    [logLevels.debug.code]: (message: string) => console.log(message),
    [logLevels.info.code]: (message: string) => console.log(blue(message)),
    [logLevels.warn.code]: (message: string) => console.log(yellow(message)),
    [logLevels.error.code]: (message: string) => console.log(red(message)),
  };
}

export abstract class WriterHandler extends Handler {
  protected _writer!: Deno.Writer;
  abstract open(): void;
  abstract write(message: string): void;
  abstract close(): void;
}

interface FileHandlerOptions extends HandlerOptions {
  filename: string;
  mode?: LogMode;
}

export class FileHandler extends WriterHandler {
  protected _file: Deno.File | undefined;
  protected _buf!: BufWriterSync;
  protected _filename: string;
  protected _mode: LogMode;
  protected _openOptions: Deno.OpenOptions;
  protected _encoder = new TextEncoder();

  #unloadCallback = (): void => this.close();

  handlerFunctions = {
    [logLevels.trace.code]: (message: string) => this.write(message),
    [logLevels.debug.code]: (message: string) => this.write(message),
    [logLevels.info.code]: (message: string) => this.write(message),
    [logLevels.warn.code]: (message: string) => this.write(message),
    [logLevels.error.code]: (message: string) => this.write(message),
  };

  constructor(logLevel: LogLevel, options: FileHandlerOptions) {
    super(logLevel, options);
    this._filename = options.filename;
    // default to append mode, write only
    this._mode = options.mode ? options.mode : "a";
    this._openOptions = {
      createNew: this._mode === "x",
      create: this._mode !== "x",
      append: this._mode === "a",
      truncate: this._mode !== "a",
      write: true,
    };
  }

  open(): void {
    this._file = Deno.openSync(this._filename, this._openOptions);
    this._writer = this._file;
    this._buf = new BufWriterSync(this._file);

    addEventListener("unload", this.#unloadCallback);
  }
  write(message: string): void {
    if (!this._buf) this.open();
    this._buf.writeSync(this._encoder.encode(message + "\n"));
  }
  close(): void {
    this.flush();
    this._file?.close();
    this._file = undefined;
    removeEventListener("unload", this.#unloadCallback);
  }
  flush(): void {
    if (this._buf?.buffered() > 0) {
      this._buf.flush();
    }
  }
}

interface RotatingFileHandlerOptions extends FileHandlerOptions {
  maxBytes: number;
  maxBackupCount: number;
}

export class RotatingFileHandler extends FileHandler {
  #maxBytes: number;
  #maxBackupCount: number;
  #currentFileSize = 0;

  constructor(logLevel: LogLevel, options: RotatingFileHandlerOptions) {
    super(logLevel, options);
    this.#maxBytes = options.maxBytes;
    this.#maxBackupCount = options.maxBackupCount;
  }

  open(): void {
    if (this.#maxBytes < 1) {
      this.close();
      throw new Error("maxBytes cannot be less than 1");
    }
    if (this.#maxBackupCount < 1) {
      this.close();
      throw new Error("maxBackupCount cannot be less than 1");
    }
    super.open();

    switch (this._mode) {
      case "w":
        for (let i = 1; i <= this.#maxBackupCount; i++) {
          if (existsSync(this._filename + "." + i)) {
            Deno.removeSync(this._filename + "." + i);
          }
        }
        break;
      case "x":
        for (let i = 1; i <= this.#maxBackupCount; i++) {
          if (existsSync(this._filename + "." + i)) {
            this.close();
            throw new Deno.errors.AlreadyExists(
              "Backup log file " + this._filename + "." + i + " already exists",
            );
          }
        }
        break;
      default:
        this.#currentFileSize = (Deno.statSync(this._filename)).size;
        break;
    }
  }
  write(message: string): void {
    const messageByteLength = this._encoder.encode(message).byteLength + 1;

    if (this.#currentFileSize + messageByteLength > this.#maxBytes) {
      this.rotateLogFiles();
      this.#currentFileSize = 0;
    }

    super.write(message);
    this.#currentFileSize += messageByteLength;
  }

  rotateLogFiles(): void {
    this._buf.flush();
    Deno.close(this._file!.rid);

    for (let i = this.#maxBackupCount - 1; i >= 0; i--) {
      const source = this._filename + (i === 0 ? "" : "." + i);
      const dest = this._filename + "." + (i + 1);

      if (existsSync(source)) {
        Deno.renameSync(source, dest);
      }
    }

    this._file = Deno.openSync(this._filename, this._openOptions);
    this._writer = this._file;
    this._buf = new BufWriterSync(this._file);
  }
}
