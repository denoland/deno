// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { LogLevel, getLevelByName, getLevelName } from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

export interface LogRecord {
  msg: string;
  args: any[];
  datetime: Date;
  level: number;
  levelName: string;
}

export class Logger {
  level: number;
  levelName: string;
  handlers: any[];

  constructor(levelName: string, handlers?: BaseHandler[]) {
    this.level = getLevelByName(levelName);
    this.levelName = levelName;

    this.handlers = handlers || [];
  }

  _log(level: number, msg: string, ...args: any[]) {
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

    this.handlers.forEach(handler => {
      handler.handle(record);
    });
  }

  debug(msg: string, ...args: any[]) {
    return this._log(LogLevel.DEBUG, msg, ...args);
  }

  info(msg: string, ...args: any[]) {
    return this._log(LogLevel.INFO, msg, ...args);
  }

  warning(msg: string, ...args: any[]) {
    return this._log(LogLevel.WARNING, msg, ...args);
  }

  error(msg: string, ...args: any[]) {
    return this._log(LogLevel.ERROR, msg, ...args);
  }

  critical(msg: string, ...args: any[]) {
    return this._log(LogLevel.CRITICAL, msg, ...args);
  }
}
