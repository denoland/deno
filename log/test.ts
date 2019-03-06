// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import * as log from "./mod.ts";
import { LogLevel } from "./levels.ts";

import "./handlers_test.ts";
import "./logger_test.ts";

// constructor(levelName: string, options: HandlerOptions = {}) {
//   this.level = getLevelByName(levelName);
//   this.levelName = levelName;

//   this.formatter = options.formatter || DEFAULT_FORMATTER;
// }

class TestHandler extends log.handlers.BaseHandler {
  public messages: string[] = [];

  log(msg: string): void {
    this.messages.push(msg);
  }
}

test(async function defaultHandlers() {
  const loggers = {
    DEBUG: log.debug,
    INFO: log.info,
    WARNING: log.warning,
    ERROR: log.error,
    CRITICAL: log.critical
  };

  for (const levelName in LogLevel) {
    if (levelName === "NOTSET") {
      continue;
    }

    const level = LogLevel[levelName];
    const logger = loggers[levelName];
    const handler = new TestHandler(level);

    await log.setup({
      handlers: {
        default: handler
      },
      loggers: {
        default: {
          level: levelName,
          handlers: ["default"]
        }
      }
    });

    logger("foo");
    logger("bar", 1, 2);

    assertEq(handler.messages, [`${levelName} foo`, `${levelName} bar`]);
  }
});

test(async function getLogger() {
  const handler = new TestHandler("DEBUG");

  await log.setup({
    handlers: {
      default: handler
    },
    loggers: {
      default: {
        level: "DEBUG",
        handlers: ["default"]
      }
    }
  });

  const logger = log.getLogger();

  assertEq(logger.levelName, "DEBUG");
  assertEq(logger.handlers, [handler]);
});

test(async function getLoggerWithName() {
  const fooHandler = new TestHandler("DEBUG");

  await log.setup({
    handlers: {
      foo: fooHandler
    },
    loggers: {
      bar: {
        level: "INFO",
        handlers: ["foo"]
      }
    }
  });

  const logger = log.getLogger("bar");

  assertEq(logger.levelName, "INFO");
  assertEq(logger.handlers, [fooHandler]);
});

test(async function getLoggerUnknown() {
  await log.setup({
    handlers: {},
    loggers: {}
  });

  const logger = log.getLogger("nonexistent");

  assertEq(logger.levelName, "NOTSET");
  assertEq(logger.handlers, []);
});
