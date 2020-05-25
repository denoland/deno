// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { Logger } from "./logger.ts";
import {
  BaseHandler,
  ConsoleHandler,
  WriterHandler,
  FileHandler,
  RotatingFileHandler,
} from "./handlers.ts";
import { assert } from "../_util/assert.ts";
import { LevelName } from "./levels.ts";

export { LogLevels } from "./levels.ts";

export class LoggerConfig {
  level?: LevelName;
  handlers?: string[];
}

export interface LogConfig {
  handlers?: {
    [name: string]: BaseHandler;
  };
  loggers?: {
    [name: string]: LoggerConfig;
  };
}

const DEFAULT_LEVEL = "INFO";
const DEFAULT_CONFIG: LogConfig = {
  handlers: {
    default: new ConsoleHandler(DEFAULT_LEVEL),
  },

  loggers: {
    default: {
      level: DEFAULT_LEVEL,
      handlers: ["default"],
    },
  },
};

const state = {
  handlers: new Map<string, BaseHandler>(),
  loggers: new Map<string, Logger>(),
  config: DEFAULT_CONFIG,
};

export const handlers = {
  BaseHandler,
  ConsoleHandler,
  WriterHandler,
  FileHandler,
  RotatingFileHandler,
};

export function getLogger(name?: string): Logger {
  if (!name) {
    const d = state.loggers.get("default");
    assert(
      d != null,
      `"default" logger must be set for getting logger without name`
    );
    return d;
  }
  const result = state.loggers.get(name);
  if (!result) {
    const logger = new Logger("NOTSET", []);
    state.loggers.set(name, logger);
    return logger;
  }
  return result;
}

export const debug = (msg: string, ...args: unknown[]): void =>
  getLogger("default").debug(msg, ...args);
export const info = (msg: string, ...args: unknown[]): void =>
  getLogger("default").info(msg, ...args);
export const warning = (msg: string, ...args: unknown[]): void =>
  getLogger("default").warning(msg, ...args);
export const error = (msg: string, ...args: unknown[]): void =>
  getLogger("default").error(msg, ...args);
export const critical = (msg: string, ...args: unknown[]): void =>
  getLogger("default").critical(msg, ...args);

export async function setup(config: LogConfig): Promise<void> {
  state.config = {
    handlers: { ...DEFAULT_CONFIG.handlers, ...config.handlers },
    loggers: { ...DEFAULT_CONFIG.loggers, ...config.loggers },
  };

  // tear down existing handlers
  state.handlers.forEach((handler): void => {
    handler.destroy();
  });
  state.handlers.clear();

  // setup handlers
  const handlers = state.config.handlers || {};

  for (const handlerName in handlers) {
    const handler = handlers[handlerName];
    await handler.setup();
    state.handlers.set(handlerName, handler);
  }

  // remove existing loggers
  state.loggers.clear();

  // setup loggers
  const loggers = state.config.loggers || {};
  for (const loggerName in loggers) {
    const loggerConfig = loggers[loggerName];
    const handlerNames = loggerConfig.handlers || [];
    const handlers: BaseHandler[] = [];

    handlerNames.forEach((handlerName): void => {
      const handler = state.handlers.get(handlerName);
      if (handler) {
        handlers.push(handler);
      }
    });

    const levelName = loggerConfig.level || DEFAULT_LEVEL;
    const logger = new Logger(levelName, handlers);
    state.loggers.set(loggerName, logger);
  }
}

await setup(DEFAULT_CONFIG);
