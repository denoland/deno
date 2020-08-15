// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { getLevelByName, LevelName, LogLevels } from "./levels.ts";
import type { LogRecord } from "./logger.ts";
import { red, yellow, blue, bold } from "../fmt/colors.ts";
import { existsSync, exists } from "../fs/exists.ts";
import { BufWriterSync } from "../io/bufio.ts";

const DEFAULT_FORMATTER = "{levelName} {message}";
type FormatterFunction = (logRecord: LogRecord) => string;
type LogMode = "a" | "w" | "x";

interface HandlerOptions {
  formatter?: string | FormatterFunction;
}

export class BaseHandler {
  level: number;
  levelName: LevelName;
  formatter: string | FormatterFunction;

  constructor(levelName: LevelName, options: HandlerOptions = {}) {
    this.level = getLevelByName(levelName);
    this.levelName = levelName;

    this.formatter = options.formatter || DEFAULT_FORMATTER;
  }

  handle(logRecord: LogRecord): void {
    if (this.level > logRecord.level) return;

    const message = this.format(logRecord);
    return this.log(message);
  }

  format(logRecord: LogRecord): string {
    if (this.formatter instanceof Function) {
      return this.formatter(logRecord);
    }

    return this.formatter.replace(/{(\S+)}/g, (match, p1): string => {
      const value = logRecord[p1 as keyof LogRecord];

      // do not interpolate missing values
      if (value == null) {
        return match;
      }

      return String(value);
    });
  }

  log(_message: string): void {}
  async setup(): Promise<void> {}
  async destroy(): Promise<void> {}
}

export class ConsoleHandler extends BaseHandler {
  format(logRecord: LogRecord): string {
    let message = super.format(logRecord);

    switch (logRecord.level) {
      case LogLevels.INFO:
        message = blue(message);
        break;
      case LogLevels.WARNING:
        message = yellow(message);
        break;
      case LogLevels.ERROR:
        message = red(message);
        break;
      case LogLevels.CRITICAL:
        message = bold(red(message));
        break;
      default:
        break;
    }

    return message;
  }

  log(message: string): void {
    console.log(message);
  }
}

export abstract class WriterHandler extends BaseHandler {
  protected _writer!: Deno.Writer;
  #encoder = new TextEncoder();

  abstract log(message: string): void;
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
  #unloadCallback = (): Promise<void> => this.destroy();

  constructor(levelName: LevelName, options: FileHandlerOptions) {
    super(levelName, options);
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

  async setup(): Promise<void> {
    this._file = await Deno.open(this._filename, this._openOptions);
    this._writer = this._file;
    this._buf = new BufWriterSync(this._file);

    addEventListener("unload", this.#unloadCallback);
  }

  handle(logRecord: LogRecord): void {
    super.handle(logRecord);

    // Immediately flush if log level is higher than ERROR
    if (logRecord.level > LogLevels.ERROR) {
      this.flush();
    }
  }

  log(message: string): void {
    this._buf.writeSync(this._encoder.encode(message + "\n"));
  }

  flush(): void {
    if (this._buf?.buffered() > 0) {
      this._buf.flush();
    }
  }

  destroy(): Promise<void> {
    this.flush();
    this._file?.close();
    this._file = undefined;
    removeEventListener("unload", this.#unloadCallback);
    return Promise.resolve();
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

  constructor(levelName: LevelName, options: RotatingFileHandlerOptions) {
    super(levelName, options);
    this.#maxBytes = options.maxBytes;
    this.#maxBackupCount = options.maxBackupCount;
  }

  async setup(): Promise<void> {
    if (this.#maxBytes < 1) {
      this.destroy();
      throw new Error("maxBytes cannot be less than 1");
    }
    if (this.#maxBackupCount < 1) {
      this.destroy();
      throw new Error("maxBackupCount cannot be less than 1");
    }
    await super.setup();

    if (this._mode === "w") {
      // Remove old backups too as it doesn't make sense to start with a clean
      // log file, but old backups
      for (let i = 1; i <= this.#maxBackupCount; i++) {
        if (await exists(this._filename + "." + i)) {
          await Deno.remove(this._filename + "." + i);
        }
      }
    } else if (this._mode === "x") {
      // Throw if any backups also exist
      for (let i = 1; i <= this.#maxBackupCount; i++) {
        if (await exists(this._filename + "." + i)) {
          this.destroy();
          throw new Deno.errors.AlreadyExists(
            "Backup log file " + this._filename + "." + i + " already exists",
          );
        }
      }
    } else {
      this.#currentFileSize = (await Deno.stat(this._filename)).size;
    }
  }

  log(message: string): void {
    const messageByteLength = this._encoder.encode(message).byteLength + 1;

    if (this.#currentFileSize + messageByteLength > this.#maxBytes) {
      this.rotateLogFiles();
      this.#currentFileSize = 0;
    }

    this._buf.writeSync(this._encoder.encode(message + "\n"));
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
