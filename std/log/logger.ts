// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  LogLevels,
  getLevelByName,
  getLevelName,
} from "./levels.ts";
import type { LevelName } from "./levels.ts";
import type { BaseHandler } from "./handlers.ts";

export interface LogRecordOptions {
  message: string;
  args: unknown[];
  level: number;
  loggerName: string;
}

export class LogRecord {
  readonly message: string;
  #args: unknown[];
  #datetime: Date;
  readonly level: number;
  readonly levelName: string;
  readonly loggerName: string;

  constructor(options: LogRecordOptions) {
    this.message = options.message;
    this.#args = [...options.args];
    this.level = options.level;
    this.loggerName = options.loggerName;
    this.#datetime = new Date();
    this.levelName = getLevelName(options.level);
  }
  get args(): unknown[] {
    return [...this.#args];
  }
  get datetime(): Date {
    return new Date(this.#datetime.getTime());
  }
}

export interface LoggerOptions {
  handlers?: BaseHandler[];
}

export class Logger {
  #level: LogLevels;
  #handlers: BaseHandler[];
  readonly #loggerName: string;

  constructor(
    loggerName: string,
    levelName: LevelName,
    options: LoggerOptions = {},
  ) {
    this.#loggerName = loggerName;
    this.#level = getLevelByName(levelName);
    this.#handlers = options.handlers || [];
  }

  get level(): LogLevels {
    return this.#level;
  }
  set level(level: LogLevels) {
    this.#level = level;
  }

  get levelName(): LevelName {
    return getLevelName(this.#level);
  }
  set levelName(levelName: LevelName) {
    this.#level = getLevelByName(levelName);
  }

  get loggerName(): string {
    return this.#loggerName;
  }

  set handlers(hndls: BaseHandler[]) {
    this.#handlers = hndls;
  }
  get handlers(): BaseHandler[] {
    return this.#handlers;
  }

  /** If the level of the logger is greater than the level to log, then nothing
   * is logged, otherwise a log record is passed to each log handler.  `message` data
   * passed in is returned.  If a function is passed in, it is only evaluated
   * if the message will be logged and the return value will be the result of the
   * function, not the function itself, unless the function isn't called, in which
   * case undefined is returned.  All types are coerced to strings for logging.
   */
  private _log<T>(
    level: number,
    message: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    if (this.level > level) {
      return message instanceof Function ? undefined : message;
    }

    let fnResult: T | undefined;
    let logMessage: string;
    if (message instanceof Function) {
      fnResult = message();
      logMessage = this.asString(fnResult);
    } else {
      logMessage = this.asString(message);
    }
    const record: LogRecord = new LogRecord({
      message: logMessage,
      args: args,
      level: level,
      loggerName: this.loggerName,
    });

    this.#handlers.forEach((handler): void => {
      handler.handle(record);
    });

    return message instanceof Function ? fnResult : message;
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

  debug<T>(message: () => T, ...args: unknown[]): T | undefined;
  debug<T>(message: T extends Function ? never : T, ...args: unknown[]): T;
  debug<T>(
    message: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.DEBUG, message, ...args);
  }

  info<T>(message: () => T, ...args: unknown[]): T | undefined;
  info<T>(message: T extends Function ? never : T, ...args: unknown[]): T;
  info<T>(
    message: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.INFO, message, ...args);
  }

  warning<T>(message: () => T, ...args: unknown[]): T | undefined;
  warning<T>(message: T extends Function ? never : T, ...args: unknown[]): T;
  warning<T>(
    message: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.WARNING, message, ...args);
  }

  error<T>(message: () => T, ...args: unknown[]): T | undefined;
  error<T>(message: T extends Function ? never : T, ...args: unknown[]): T;
  error<T>(
    message: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.ERROR, message, ...args);
  }

  critical<T>(message: () => T, ...args: unknown[]): T | undefined;
  critical<T>(message: T extends Function ? never : T, ...args: unknown[]): T;
  critical<T>(
    message: (T extends Function ? never : T) | (() => T),
    ...args: unknown[]
  ): T | undefined {
    return this._log(LogLevels.CRITICAL, message, ...args);
  }
}
