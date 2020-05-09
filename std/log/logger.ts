// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  LogLevels,
  getLevelByName,
  getLevelName,
  LevelName,
} from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

type StringResolverFn = () => string;

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

  _log(
    level: number,
    msg: string | StringResolverFn,
    ...args: unknown[]
  ): string | { msg: string; args: unknown[] } | undefined {
    if (this.level > level) {
      if (msg instanceof Function) {
        return undefined;
      } else {
        return args!.length > 0 ? { msg: msg, args: args } : msg;
      }
    }

    const logMessage = msg instanceof Function ? msg() : msg;
    const record: LogRecord = new LogRecord(logMessage, args, level);

    this.handlers.forEach((handler): void => {
      handler.handle(record);
    });

    return args!.length > 0 ? { msg: logMessage, args: args } : logMessage;
  }

  debug(msg: string): string;
  debug(msg: StringResolverFn): string;
  debug(msg: string, ...args: unknown[]): { msg: string; args: unknown[] };
  debug(
    msg: StringResolverFn,
    ...args: unknown[]
  ): { msg: string; args: unknown[] };
  debug(
    msg: string | StringResolverFn,
    ...args: unknown[]
  ): string | { msg: string; args: unknown[] } | undefined {
    return this._log(LogLevels.DEBUG, msg, ...args);
  }

  info(msg: string): string;
  info(msg: StringResolverFn): string;
  info(msg: string, ...args: unknown[]): { msg: string; args: unknown[] };
  info(
    msg: StringResolverFn,
    ...args: unknown[]
  ): { msg: string; args: unknown[] };
  info(
    msg: string | StringResolverFn,
    ...args: unknown[]
  ): string | { msg: string; args: unknown[] } | undefined {
    return this._log(LogLevels.INFO, msg, ...args);
  }

  warning(msg: string): string;
  warning(msg: StringResolverFn): string;
  warning(msg: string, ...args: unknown[]): { msg: string; args: unknown[] };
  warning(
    msg: StringResolverFn,
    ...args: unknown[]
  ): { msg: string; args: unknown[] };
  warning(
    msg: string | StringResolverFn,
    ...args: unknown[]
  ): string | { msg: string; args: unknown[] } | undefined {
    return this._log(LogLevels.WARNING, msg, ...args);
  }

  error(msg: string): string;
  error(msg: StringResolverFn): string;
  error(msg: string, ...args: unknown[]): { msg: string; args: unknown[] };
  error(
    msg: StringResolverFn,
    ...args: unknown[]
  ): { msg: string; args: unknown[] };
  error(
    msg: string | StringResolverFn,
    ...args: unknown[]
  ): string | { msg: string; args: unknown[] } | undefined {
    return this._log(LogLevels.ERROR, msg, ...args);
  }

  critical(msg: string): string;
  critical(msg: StringResolverFn): string;
  critical(msg: string, ...args: unknown[]): { msg: string; args: unknown[] };
  critical(
    msg: StringResolverFn,
    ...args: unknown[]
  ): { msg: string; args: unknown[] };
  critical(
    msg: string | StringResolverFn,
    ...args: unknown[]
  ): string | { msg: string; args: unknown[] } | undefined {
    return this._log(LogLevels.CRITICAL, msg, ...args);
  }
}
