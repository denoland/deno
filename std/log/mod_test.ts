// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../testing/asserts.ts";
import {
  critical,
  debug,
  error,
  getLogger,
  info,
  LevelName,
  Logger,
  LogLevels,
  setup,
  warning,
} from "./mod.ts";
import { BaseHandler } from "./handlers.ts";

class TestHandler extends BaseHandler {
  public messages: string[] = [];

  public log(str: string): void {
    this.messages.push(str);
  }
}

let logger: Logger | null = null;
try {
  // Need to initialize it here
  // otherwise it will be already initialized on Deno.test
  logger = getLogger();
} catch {
  // Pass
}

Deno.test("logger is initialized", function (): void {
  assert(logger instanceof Logger);
});

Deno.test("default loggers work as expected", function (): void {
  const sym = Symbol("a");
  const debugData: string = debug("foo");
  const debugResolver: string | undefined = debug(() => "foo");
  const infoData: number = info(456, 1, 2, 3);
  const infoResolver: boolean | undefined = info(() => true);
  const warningData: symbol = warning(sym);
  const warningResolver: null | undefined = warning(() => null);
  const errorData: undefined = error(undefined, 1, 2, 3);
  const errorResolver: bigint | undefined = error(() => 5n);
  const criticalData: string = critical("foo");
  const criticalResolver: string | undefined = critical(() => "bar");
  assertEquals(debugData, "foo");
  assertEquals(debugResolver, undefined);
  assertEquals(infoData, 456);
  assertEquals(infoResolver, true);
  assertEquals(warningData, sym);
  assertEquals(warningResolver, null);
  assertEquals(errorData, undefined);
  assertEquals(errorResolver, 5n);
  assertEquals(criticalData, "foo");
  assertEquals(criticalResolver, "bar");
});

Deno.test({
  name: "Logging config works as expected with logger names",
  async fn() {
    const consoleHandler = new TestHandler("DEBUG");
    const anotherConsoleHandler = new TestHandler("DEBUG", {
      formatter: "[{loggerName}] {levelName} {msg}",
    });
    await setup({
      handlers: {
        console: consoleHandler,
        anotherConsole: anotherConsoleHandler,
      },

      loggers: {
        // configure default logger available via short-hand methods above
        default: {
          level: "DEBUG",
          handlers: ["console"],
        },

        tasks: {
          level: "ERROR",
          handlers: ["anotherConsole"],
        },
      },
    });
    getLogger().debug("hello");
    getLogger("tasks").error("world");
    assertEquals(consoleHandler.messages[0], "DEBUG hello");
    assertEquals(anotherConsoleHandler.messages[0], "[tasks] ERROR world");
  },
});

Deno.test({
  name: "Loggers have level and levelName to get/set loglevels",
  async fn() {
    const testHandler = new TestHandler("DEBUG");
    await setup({
      handlers: {
        test: testHandler,
      },

      loggers: {
        // configure default logger available via short-hand methods above
        default: {
          level: "DEBUG",
          handlers: ["test"],
        },
      },
    });
    const logger: Logger = getLogger();
    assertEquals(logger.levelName, "DEBUG");
    assertEquals(logger.level, LogLevels.DEBUG);

    logger.debug("debug");
    logger.error("error");
    logger.critical("critical");
    assertEquals(testHandler.messages.length, 3);
    assertEquals(testHandler.messages[0], "DEBUG debug");
    assertEquals(testHandler.messages[1], "ERROR error");
    assertEquals(testHandler.messages[2], "CRITICAL critical");

    testHandler.messages = [];
    logger.level = LogLevels.WARNING;
    assertEquals(logger.levelName, "WARNING");
    assertEquals(logger.level, LogLevels.WARNING);

    logger.debug("debug2");
    logger.error("error2");
    logger.critical("critical2");
    assertEquals(testHandler.messages.length, 2);
    assertEquals(testHandler.messages[0], "ERROR error2");
    assertEquals(testHandler.messages[1], "CRITICAL critical2");

    testHandler.messages = [];
    const setLevelName: LevelName = "CRITICAL";
    logger.levelName = setLevelName;
    assertEquals(logger.levelName, "CRITICAL");
    assertEquals(logger.level, LogLevels.CRITICAL);

    logger.debug("debug3");
    logger.error("error3");
    logger.critical("critical3");
    assertEquals(testHandler.messages.length, 1);
    assertEquals(testHandler.messages[0], "CRITICAL critical3");
  },
});

Deno.test({
  name: "Loggers have loggerName to get logger name",
  async fn() {
    const testHandler = new TestHandler("DEBUG");
    await setup({
      handlers: {
        test: testHandler,
      },

      loggers: {
        namedA: {
          level: "DEBUG",
          handlers: ["test"],
        },
        namedB: {
          level: "DEBUG",
          handlers: ["test"],
        },
      },
    });

    assertEquals(getLogger("namedA").loggerName, "namedA");
    assertEquals(getLogger("namedB").loggerName, "namedB");
    assertEquals(getLogger().loggerName, "default");
    assertEquals(getLogger("nonsetupname").loggerName, "nonsetupname");
  },
});

Deno.test({
  name: "Logger has mutable handlers",
  async fn() {
    const testHandlerA = new TestHandler("DEBUG");
    const testHandlerB = new TestHandler("DEBUG");
    await setup({
      handlers: {
        testA: testHandlerA,
        testB: testHandlerB,
      },

      loggers: {
        default: {
          level: "DEBUG",
          handlers: ["testA"],
        },
      },
    });
    const logger: Logger = getLogger();
    logger.info("msg1");
    assertEquals(testHandlerA.messages.length, 1);
    assertEquals(testHandlerA.messages[0], "INFO msg1");
    assertEquals(testHandlerB.messages.length, 0);

    logger.handlers = [testHandlerA, testHandlerB];

    logger.info("msg2");
    assertEquals(testHandlerA.messages.length, 2);
    assertEquals(testHandlerA.messages[1], "INFO msg2");
    assertEquals(testHandlerB.messages.length, 1);
    assertEquals(testHandlerB.messages[0], "INFO msg2");

    logger.handlers = [testHandlerB];

    logger.info("msg3");
    assertEquals(testHandlerA.messages.length, 2);
    assertEquals(testHandlerB.messages.length, 2);
    assertEquals(testHandlerB.messages[1], "INFO msg3");

    logger.handlers = [];
    logger.info("msg4");
    assertEquals(testHandlerA.messages.length, 2);
    assertEquals(testHandlerB.messages.length, 2);
  },
});
