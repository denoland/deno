// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { LogLevel, getLevelByName, getLevelName } from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

export interface LogRecord {
  msg: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  args: any[];
  datetime: Date;
  level: number;
  levelName: string;
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

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  _log(level: number, msg: string, ...args: any[]): void {
    if (this.level > level) return;

    // TODO: it'd be a good idea to make it immutable, so
    // no handler mangles it by mistake
    // TODO: iterpolate msg with values
    const record: LogRecord = {
      msg: msg,
      args: args,
      datetime: new Date(),
      level: level,
      levelName: getLevelName(level)
    };

    this.handlers.forEach(
      (handler): void => {
        handler.handle(record);
      }
    );
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  debug(msg: string, ...args: any[]): void {
    this._log(LogLevel.DEBUG, msg, ...args);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  info(msg: string, ...args: any[]): void {
    this._log(LogLevel.INFO, msg, ...args);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  warning(msg: string, ...args: any[]): void {
    this._log(LogLevel.WARNING, msg, ...args);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  error(msg: string, ...args: any[]): void {
    this._log(LogLevel.ERROR, msg, ...args);
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  critical(msg: string, ...args: any[]): void {
    this._log(LogLevel.CRITICAL, msg, ...args);
  }
}
