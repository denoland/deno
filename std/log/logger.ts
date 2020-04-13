// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { LogLevel, getLevelByName, getLevelName } from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

export class LogRecord {
  readonly msg: string;
  #args: unknown[];
  #datetime: Date;
  readonly level: number;
  readonly levelName: string;

  constructor(msg: string, args: unknown[], level: number) {
    this.msg = msg;
    this.#args = [...args];
    this.level = level;
    this.#datetime = new Date();
    this.levelName = getLevelName(level);
  }
  get args(): unknown[] {
    return [...this.#args];
  }
  get datetime(): Date {
    return new Date(this.#datetime.getTime());
  }
}

export class Logger {
  level: number;
  levelName: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  handlers: any[];

  constructor(levelName: string, handlers?: BaseHandler[]) {
    this.level = getLevelByName(levelName);
    this.levelName = levelName;

    this.handlers = handlers || [];
  }

  _log(level: number, msg: string, ...args: unknown[]): void {
    if (this.level > level) return;

    const record: LogRecord = new LogRecord(msg, args, level);

    this.handlers.forEach((handler): void => {
      handler.handle(record);
    });
  }

  debug(msg: string, ...args: unknown[]): void {
    this._log(LogLevel.DEBUG, msg, ...args);
  }

  info(msg: string, ...args: unknown[]): void {
    this._log(LogLevel.INFO, msg, ...args);
  }

  warning(msg: string, ...args: unknown[]): void {
    this._log(LogLevel.WARNING, msg, ...args);
  }

  error(msg: string, ...args: unknown[]): void {
    this._log(LogLevel.ERROR, msg, ...args);
  }

  critical(msg: string, ...args: unknown[]): void {
    this._log(LogLevel.CRITICAL, msg, ...args);
  }
}
