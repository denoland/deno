// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Handler } from "./handlers.ts";
import { LogLevel, logLevels } from "./levels.ts";

export function asString(data: unknown): string {
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

// deno-lint-ignore no-explicit-any
export type GenericFunction = (...args: any[]) => any;

export interface LogRecordOptions {
  message: unknown;
  args: unknown[];
  logLevel: LogLevel;
  loggerName: string;
}

export class LogRecord {
  readonly message: unknown;
  #args: unknown[];
  #datetime: Date;
  readonly logLevel: LogLevel;
  readonly loggerName: string;

  constructor(options: LogRecordOptions) {
    this.#args = [...options.args];
    this.#datetime = new Date();
    this.message = options.message;
    this.logLevel = options.logLevel;
    this.loggerName = options.loggerName;
  }
  get args(): unknown[] {
    return [...this.#args];
  }
  get datetime(): Date {
    return new Date(this.#datetime.getTime());
  }
}

const DEFAULT_LOGGER_NAME = "logger";

export class BaseLogger {
  name: string;
  logLevel: LogLevel;
  handlers: Handler[];
  constructor(logLevel: LogLevel, {
    name = DEFAULT_LOGGER_NAME,
    handlers = [],
  }: {
    name?: string;
    handlers?: Handler[];
  } = {}) {
    this.name = name;
    this.logLevel = logLevel;
    this.handlers = handlers;
  }

  protected dispatch(logLevel: LogLevel, message: unknown, ...args: unknown[]) {
    if (this.logLevel.code > logLevel.code) return;

    if (message instanceof Function) {
      message = message(logLevel);
    }

    message = asString(message);

    const record = new LogRecord({
      loggerName: this.name,
      message,
      args,
      logLevel,
    });

    this.handlers.forEach((handler) => handler.handle(record));
  }
}

export class Logger extends BaseLogger {
  trace(message: unknown, ...args: unknown[]) {
    return this.dispatch(logLevels.trace, message, ...args);
  }
  debug(message: unknown, ...args: unknown[]) {
    return this.dispatch(logLevels.debug, message, ...args);
  }
  info(message: unknown, ...args: unknown[]) {
    return this.dispatch(logLevels.info, message, ...args);
  }
  warn(message: unknown, ...args: unknown[]) {
    return this.dispatch(logLevels.warn, message, ...args);
  }
  error(message: unknown, ...args: unknown[]) {
    return this.dispatch(logLevels.error, message, ...args);
  }
}
