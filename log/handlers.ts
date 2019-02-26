// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
const { open } = Deno;
import { File, Writer } from "deno";
import { getLevelByName, LogLevel } from "./levels.ts";
import { LogRecord } from "./logger.ts";
import { red, yellow, blue, bold } from "../colors/mod.ts";

const DEFAULT_FORMATTER = "{levelName} {msg}";
type FormatterFunction = (logRecord: LogRecord) => string;

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

  handle(logRecord: LogRecord) {
    if (this.level > logRecord.level) return;

    const msg = this.format(logRecord);
    return this.log(msg);
  }

  format(logRecord: LogRecord): string {
    if (this.formatter instanceof Function) {
      return this.formatter(logRecord);
    }

    return this.formatter.replace(/{(\S+)}/g, (match, p1) => {
      const value = logRecord[p1];

      // do not interpolate missing values
      if (!value) {
        return match;
      }

      return value;
    });
  }

  log(msg: string) {}
  async setup() {}
  async destroy() {}
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

  log(msg: string) {
    console.log(msg);
  }
}

export abstract class WriterHandler extends BaseHandler {
  protected _writer: Writer;
  private _encoder = new TextEncoder();

  log(msg: string) {
    this._writer.write(this._encoder.encode(msg + "\n"));
  }
}

interface FileHandlerOptions extends HandlerOptions {
  filename: string;
}

export class FileHandler extends WriterHandler {
  private _file: File;
  private _filename: string;

  constructor(levelName: string, options: FileHandlerOptions) {
    super(levelName, options);
    this._filename = options.filename;
  }

  async setup() {
    // open file in append mode - write only
    this._file = await open(this._filename, "a");
    this._writer = this._file;
  }

  async destroy() {
    await this._file.close();
  }
}
