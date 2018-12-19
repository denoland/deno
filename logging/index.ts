import { Logger } from "./logger.ts";
import { BaseHandler } from "./handler.ts";
import { ConsoleHandler } from "./handlers/console.ts";

export interface HandlerConfig {
  // TODO: replace with type describing class derived from BaseHandler
  class: typeof BaseHandler;
  level?: string;
}

export class LoggerConfig {
  level?: string;
  handlers?: string[];
}

export interface LoggingConfig {
  handlers?: {
    [name: string]: HandlerConfig;
  };
  loggers?: {
    [name: string]: LoggerConfig;
  };
}

const DEFAULT_LEVEL = "INFO";
const DEFAULT_NAME = "";
const DEFAULT_CONFIG: LoggingConfig = {
  handlers: {
    [DEFAULT_NAME]: {
      level: DEFAULT_LEVEL,
      class: ConsoleHandler
    }
  },

  loggers: {
    [DEFAULT_NAME]: {
      level: DEFAULT_LEVEL,
      handlers: [DEFAULT_NAME]
    }
  }
};

const state = {
  loggers: new Map(),
  config: DEFAULT_CONFIG
};

function createNewHandler(name: string) {
  let handlerConfig = state.config.handlers[name];

  if (!handlerConfig) {
    handlerConfig = state.config.handlers[DEFAULT_NAME];
  }

  const constructor = handlerConfig.class;
  console.log(constructor);
  const handler = new constructor(handlerConfig.level);
  return handler;
}

function createNewLogger(name: string) {
  let loggerConfig = state.config.loggers[name];

  if (!loggerConfig) {
    loggerConfig = state.config.loggers[DEFAULT_NAME];
  }

  const handlers = (loggerConfig.handlers || []).map(createNewHandler);
  const levelName = loggerConfig.level || DEFAULT_LEVEL;
  return new Logger(levelName, handlers);
}

export const handlers = {
  BaseHandler: BaseHandler,
  ConsoleHandler: ConsoleHandler
};

export function getLogger(name?: string) {
  if (!name) {
    name = DEFAULT_NAME;
  }

  if (!state.loggers.has(name)) {
    return createNewLogger(name);
  }

  return state.loggers.get(name);
}

export function setup(config: LoggingConfig) {
  state.config = {
    handlers: {
      ...DEFAULT_CONFIG.handlers,
      ...config.handlers!
    },
    loggers: {
      ...DEFAULT_CONFIG.loggers,
      ...config.loggers!
    }
  };
}
