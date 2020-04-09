// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { open, openSync, close, renameSync, statSync } = Deno;
type File = Deno.File;
type Writer = Deno.Writer;
import { getLevelByName, LogLevel } from "./levels.ts";
import { LogRecord } from "./logger.ts";
import { red, yellow, blue, bold } from "../fmt/colors.ts";
import { existsSync, exists } from "../fs/exists.ts";

const DEFAULT_FORMATTER = "{levelName} {msg}";
type FormatterFunction = (logRecord: LogRecord) => string;
type LogMode = "a" | "w" | "x";

interface HandlerOptions {
  formatter?: string | FormatterFunction;
}

export class BaseHandler {
  level: number;
  levelName: string;
  formatter: string | FormatterFunction;

  constructor(levelName: string, options: HandlerOptions = {}) {
    this.level = getLevelByName(levelName);
    this.levelName = levelName;

    this.formatter = options.formatter || DEFAULT_FORMATTER;
  }

  handle(logRecord: LogRecord): void {
    if (this.level > logRecord.level) return;

    const msg = this.format(logRecord);
    return this.log(msg);
  }

  format(logRecord: LogRecord): string {
    if (this.formatter instanceof Function) {
      return this.formatter(logRecord);
    }

    return this.formatter.replace(/{(\S+)}/g, (match, p1): string => {
      const value = logRecord[p1 as keyof LogRecord];

      // do not interpolate missing values
      if (!value) {
        return match;
      }

      return String(value);
    });
  }

  log(_msg: string): void {}
  async setup(): Promise<void> {}
  async destroy(): Promise<void> {}
}

export class ConsoleHandler extends BaseHandler {
  format(logRecord: LogRecord): string {
    let msg = super.format(logRecord);

    switch (logRecord.level) {
      case LogLevel.INFO:
        msg = blue(msg);
        break;
      case LogLevel.WARNING:
        msg = yellow(msg);
        break;
      case LogLevel.ERROR:
        msg = red(msg);
        break;
      case LogLevel.CRITICAL:
        msg = bold(red(msg));
        break;
      default:
        break;
    }

    return msg;
  }

  log(msg: string): void {
    console.log(msg);
  }
}

export abstract class WriterHandler extends BaseHandler {
  protected _writer!: Writer;
  #encoder = new TextEncoder();

  abstract log(msg: string): void;
}

interface FileHandlerOptions extends HandlerOptions {
  filename: string;
  mode?: LogMode;
}

export class FileHandler extends WriterHandler {
  protected _file!: File;
  protected _filename: string;
  protected _mode: LogMode;
  #encoder = new TextEncoder();

  constructor(levelName: string, options: FileHandlerOptions) {
    super(levelName, options);
    this._filename = options.filename;
    // default to append mode, write only
    this._mode = options.mode ? options.mode : "a";
  }

  async setup(): Promise<void> {
    this._file = await open(this._filename, this._mode);
    this._writer = this._file;
  }

  log(msg: string): void {
    Deno.writeSync(this._file.rid, this.#encoder.encode(msg + "\n"));
  }

  async destroy(): Promise<void> {
    await this._file.close();
  }
}

interface RotatingFileHandlerOptions extends FileHandlerOptions {
  maxBytes: number;
  maxBackupCount: number;
}

export class RotatingFileHandler extends FileHandler {
  #maxBytes: number;
  #maxBackupCount: number;

  constructor(levelName: string, options: RotatingFileHandlerOptions) {
    super(levelName, options);
    this.#maxBytes = options.maxBytes;
    this.#maxBackupCount = options.maxBackupCount;
  }

  async setup(): Promise<void> {
    if (this.#maxBytes < 1) {
      throw new Error("maxBytes cannot be less than 1");
    }
    if (this.#maxBackupCount < 1) {
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
          Deno.close(this._file.rid);
          throw new Deno.errors.AlreadyExists(
            "Backup log file " + this._filename + "." + i + " already exists"
          );
        }
      }
    }
  }

  handle(logRecord: LogRecord): void {
    if (this.level > logRecord.level) return;

    const msg = this.format(logRecord);
    const currentFileSize = statSync(this._filename).size;
    if (currentFileSize + msg.length > this.#maxBytes) {
      this.rotateLogFiles();
    }

    return this.log(msg);
  }

  rotateLogFiles(): void {
    close(this._file.rid);

    for (let i = this.#maxBackupCount - 1; i >= 0; i--) {
      const source = this._filename + (i === 0 ? "" : "." + i);
      const dest = this._filename + "." + (i + 1);

      if (existsSync(source)) {
        renameSync(source, dest);
      }
    }

    this._file = openSync(this._filename, this._mode);
    this._writer = this._file;
  }
}
