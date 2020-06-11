// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  LogLevels,
  getLevelByName,
  getLevelName,
  LevelName,
} from "./levels.ts";
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
  levelName: LevelName;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  handlers: any[];

  constructor(levelName: LevelName, handlers?: BaseHandler[]) {
    this.level = getLevelByName(levelName);
    this.levelName = levelName;

    this.handlers = handlers || [];
  }

  /** If the level of the logger is greater than the level to log, then nothing
   * is logged, otherwise a log record is passed to each log handler.  `msg` data
   * passed in is returned.  If a function is passed in, it is only evaluated
   * if the msg will be logged and the return value will be the result of the
   * function, not the function itself, unless the function isn't called, in which
   * case undefined is returned.  All types are coerced to strings for logging.
   */
  _log<T>(
    level: number,
    msg: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    if (this.level > level) {
      return msg instanceof Function ? undefined : msg;
    }

    let fnResult: T | undefined;
    let logMessage: string;
    if (msg instanceof Function) {
      fnResult = msg();
      logMessage = this.asString(fnResult);
    } else {
      logMessage = this.asString(msg);
    }
    const record: LogRecord = new LogRecord(logMessage, args, level);

    this.handlers.forEach((handler): void => {
      handler.handle(record);
    });

    return msg instanceof Function ? fnResult : msg;
  }

  asString(data: unknown): string {
    if (typeof data === "string") {
      return data;
    } else if (
      data === null ||
      typeof data === "number" ||
      typeof data === "bigint" ||
      typeof data === "boolean" ||
      typeof data === "undefined" ||
      typeof data === "symbol"
    ) {
      return String(data);
    } else if (typeof data === "object") {
      return JSON.stringify(data);
    }
    return "undefined";
  }

  debug<T>(msg: () => T, ...args: unknown[]): T | undefined;
  debug<T>(msg: T extends Function ? never : T, ...args: unknown[]): T;
  debug<T>(
    msg: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.DEBUG, msg, ...args);
  }

  info<T>(msg: () => T, ...args: unknown[]): T | undefined;
  info<T>(msg: T extends Function ? never : T, ...args: unknown[]): T;
  info<T>(
    msg: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.INFO, msg, ...args);
  }

  warning<T>(msg: () => T, ...args: unknown[]): T | undefined;
  warning<T>(msg: T extends Function ? never : T, ...args: unknown[]): T;
  warning<T>(
    msg: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.WARNING, msg, ...args);
  }

  error<T>(msg: () => T, ...args: unknown[]): T | undefined;
  error<T>(msg: T extends Function ? never : T, ...args: unknown[]): T;
  error<T>(
    msg: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.ERROR, msg, ...args);
  }

  critical<T>(msg: () => T, ...args: unknown[]): T | undefined;
  critical<T>(msg: T extends Function ? never : T, ...args: unknown[]): T;
  critical<T>(
    msg: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.CRITICAL, msg, ...args);
  }
}
