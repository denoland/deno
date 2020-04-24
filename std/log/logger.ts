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

  _log(level: number, msg: string, ...args: unknown[]): void {
    if (this.level > level) return;

    const record: LogRecord = new LogRecord(msg, args, level);

    this.handlers.forEach((handler): void => {
      handler.handle(record);
    });
  }

  debug(msg: string, ...args: unknown[]): void {
    this._log(LogLevels.DEBUG, msg, ...args);
  }

  info(msg: string, ...args: unknown[]): void {
    this._log(LogLevels.INFO, msg, ...args);
  }

  warning(msg: string, ...args: unknown[]): void {
    this._log(LogLevels.WARNING, msg, ...args);
  }

  error(msg: string, ...args: unknown[]): void {
    this._log(LogLevels.ERROR, msg, ...args);
  }

  critical(msg: string, ...args: unknown[]): void {
    this._log(LogLevels.CRITICAL, msg, ...args);
  }
}
