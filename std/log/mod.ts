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
import type { LevelName } from "./levels.ts";

export { LogLevels } from "./levels.ts";
export type { LevelName } from "./levels.ts";
export { Logger } from "./logger.ts";

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
      `"default" logger must be set for getting logger without name`,
    );
    return d;
  }
  const result = state.loggers.get(name);
  if (!result) {
    const logger = new Logger(name, "NOTSET", { handlers: [] });
    state.loggers.set(name, logger);
    return logger;
  }
  return result;
}

export function debug<T>(msg: () => T, ...args: unknown[]): T | undefined;
export function debug<T>(
  msg: T extends Function ? never : T,
  ...args: unknown[]
): T;
export function debug<T>(
  msg: (T extends Function ? never : T) | (() => T),
  ...args: unknown[]
): T | undefined {
  // Assist TS compiler with pass-through generic type
  if (msg instanceof Function) {
    return getLogger("default").debug(msg, ...args);
  }
  return getLogger("default").debug(msg, ...args);
}

export function info<T>(msg: () => T, ...args: unknown[]): T | undefined;
export function info<T>(
  msg: T extends Function ? never : T,
  ...args: unknown[]
): T;
export function info<T>(
  msg: (T extends Function ? never : T) | (() => T),
  ...args: unknown[]
): T | undefined {
  // Assist TS compiler with pass-through generic type
  if (msg instanceof Function) {
    return getLogger("default").info(msg, ...args);
  }
  return getLogger("default").info(msg, ...args);
}

export function warning<T>(msg: () => T, ...args: unknown[]): T | undefined;
export function warning<T>(
  msg: T extends Function ? never : T,
  ...args: unknown[]
): T;
export function warning<T>(
  msg: (T extends Function ? never : T) | (() => T),
  ...args: unknown[]
): T | undefined {
  // Assist TS compiler with pass-through generic type
  if (msg instanceof Function) {
    return getLogger("default").warning(msg, ...args);
  }
  return getLogger("default").warning(msg, ...args);
}

export function error<T>(msg: () => T, ...args: unknown[]): T | undefined;
export function error<T>(
  msg: T extends Function ? never : T,
  ...args: unknown[]
): T;
export function error<T>(
  msg: (T extends Function ? never : T) | (() => T),
  ...args: unknown[]
): T | undefined {
  // Assist TS compiler with pass-through generic type
  if (msg instanceof Function) {
    return getLogger("default").error(msg, ...args);
  }
  return getLogger("default").error(msg, ...args);
}

export function critical<T>(msg: () => T, ...args: unknown[]): T | undefined;
export function critical<T>(
  msg: T extends Function ? never : T,
  ...args: unknown[]
): T;
export function critical<T>(
  msg: (T extends Function ? never : T) | (() => T),
  ...args: unknown[]
): T | undefined {
  // Assist TS compiler with pass-through generic type
  if (msg instanceof Function) {
    return getLogger("default").critical(msg, ...args);
  }
  return getLogger("default").critical(msg, ...args);
}

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
    const logger = new Logger(loggerName, levelName, { handlers: handlers });
    state.loggers.set(loggerName, logger);
  }
}

await setup(DEFAULT_CONFIG);
