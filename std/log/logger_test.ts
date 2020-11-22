// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "../testing/asserts.ts";
import { Logger, LogRecord } from "./logger.ts";
import { ConsoleHandler, Handler } from "./handlers.ts";
import { logLevels } from "./levels.ts";

class TestHandler extends Handler {
  public messages: string[] = [];
  public records: LogRecord[] = [];

  handlerFunctions = {
    [logLevels.trace.code]: (message: string) => this.messages.push(message),
    [logLevels.debug.code]: (message: string) => this.messages.push(message),
    [logLevels.info.code]: (message: string) => this.messages.push(message),
    [logLevels.warn.code]: (message: string) => this.messages.push(message),
    [logLevels.error.code]: (message: string) => this.messages.push(message),
  };

  handle(record: LogRecord): void {
    this.records.push(record);
    super.handle(record);
  }
}

Deno.test({
  name: "default instance",
  fn() {
    const logger = new Logger(logLevels.debug, {
      handlers: [new ConsoleHandler(logLevels.debug)],
    });
    assertEquals(logger.logLevel, logLevels.debug);
    assert(logger.handlers[0] instanceof ConsoleHandler);
  },
});

Deno.test({
  name: "default name",
  fn() {
    const logLevel = logLevels.debug;
    const handlerNoName = new TestHandler(logLevel);
    const handlerWithLoggerName = new TestHandler(logLevel, {
      formatter: ({ loggerName, logLevel, message }) =>
        `[${loggerName}] ${logLevel.name} ${message}`,
    });

    const logger = new Logger(logLevel, {
      handlers: [handlerNoName, handlerWithLoggerName],
    });
    logger.debug("hello");
    assertEquals(handlerNoName.messages[0], "Debug hello");
    assertEquals(handlerWithLoggerName.messages[0], "[logger] Debug hello");
  },
});

Deno.test({
  name: "custom name",
  fn() {
    const logLevel = logLevels.debug;
    const handlerNoName = new TestHandler(logLevel);
    const handlerWithLoggerName = new TestHandler(logLevel, {
      formatter: ({ loggerName, logLevel, message }) =>
        `[${loggerName}] ${logLevel.name} ${message}`,
    });

    const logger = new Logger(logLevel, {
      name: "config",
      handlers: [handlerNoName, handlerWithLoggerName],
    });
    logger.debug("hello");
    assertEquals(handlerNoName.messages[0], "Debug hello");
    assertEquals(handlerWithLoggerName.messages[0], "[config] Debug hello");
  },
});

Deno.test({
  name: "get set logLevel",
  fn() {
    const logger = new Logger(logLevels.error);

    logger.logLevel = logLevels.error;

    assertEquals(logger.logLevel, logLevels.error);
  },
});

Deno.test({
  name: "get set level",
  fn() {
    const logger = new Logger(logLevels.error);

    logger.logLevel = logLevels.error;

    assertEquals(logger.logLevel, logLevels.error);
  },
});

Deno.test("default methods", function (): void {
  const logger = new Logger(logLevels.error);
  const sym = Symbol("a");
  logger.debug("foo");
  logger.debug(() => "foo");
  logger.info(456, 1, 2, 3);
  logger.info(() => true);
  logger.warn(sym);
  logger.warn(() => null);
  logger.error(undefined, 1, 2, 3);
  logger.error(() => 5n);
});

Deno.test("custom handler", function (): void {
  const logLevel = logLevels.debug;
  const handler = new TestHandler(logLevel);
  const logger = new Logger(logLevel, {
    handlers: [handler],
  });

  logger.debug("foo", 1, 2);
  const record = handler.records[0];
  assertEquals(record.message, "foo");
  assertEquals(record.args, [1, 2]);
  assertEquals(record.logLevel, logLevels.debug);
  assertEquals(record.logLevel, logLevel);

  assertEquals(handler.messages, ["Debug foo 1 2"]);
});

Deno.test("lazy log evaluation", function (): void {
  const logLevel = logLevels.error;
  const handler = new TestHandler(logLevel);
  const logger = new Logger(logLevel, {
    handlers: [handler],
  });
  let called = false;

  const expensiveFunction = (): string => {
    called = true;
    return "expensive function result";
  };
  logger.debug(expensiveFunction);
  assert(!called);
  assertEquals(handler.messages[0], undefined);
  logger.error(expensiveFunction);
  assert(called);
  assertEquals(handler.messages[0], "Error expensive function result");
});

Deno.test("argument types", function (): void {
  const handler = new TestHandler(logLevels.debug);
  const logger = new Logger(logLevels.debug, {
    handlers: [handler],
  });
  const sym = Symbol();
  const syma = Symbol("a");
  const fn = (): string => {
    return "abc";
  };

  // string
  logger.debug("abc");
  logger.debug("def", 1);
  assertEquals(handler.messages[0], "Debug abc");
  assertEquals(handler.messages[1], "Debug def 1");

  // null
  logger.info(null);
  logger.info(null, 1);
  assertEquals(handler.messages[2], "Info null");
  assertEquals(handler.messages[3], "Info null 1");

  // number
  logger.warn(3);
  logger.warn(3, 1);
  assertEquals(handler.messages[4], "Warn 3");
  assertEquals(handler.messages[5], "Warn 3 1");

  // bigint
  logger.error(5n);
  logger.error(5n, 1);
  assertEquals(handler.messages[6], "Error 5");
  assertEquals(handler.messages[7], "Error 5 1");

  // boolean
  logger.error(true);
  logger.error(false, 1);
  assertEquals(handler.messages[8], "Error true");
  assertEquals(handler.messages[9], "Error false 1");

  // undefined
  logger.debug(undefined);
  logger.debug(undefined, 1);
  assertEquals(handler.messages[10], "Debug undefined");
  assertEquals(handler.messages[11], "Debug undefined 1");

  // symbol
  logger.info(sym);
  logger.info(syma, 1);
  assertEquals(handler.messages[12], "Info Symbol()");
  assertEquals(handler.messages[13], "Info Symbol(a) 1");

  // function
  logger.warn(fn);
  logger.warn(fn, 1);
  assertEquals(handler.messages[14], "Warn abc");
  assertEquals(handler.messages[15], "Warn abc 1");

  // object
  logger.error({
    payload: "data",
    other: 123,
  });
  logger.error(
    { payload: "data", other: 123 },
    1,
  );
  assertEquals(handler.messages[16], 'Error {"payload":"data","other":123}');
  assertEquals(handler.messages[17], 'Error {"payload":"data","other":123} 1');
});

Deno.test({
  name: "mutable handlers",
  fn() {
    const testHandlerA = new TestHandler(logLevels.debug);
    const testHandlerB = new TestHandler(logLevels.debug);
    const logger = new Logger(logLevels.debug, {
      handlers: [testHandlerA],
    });
    logger.info("message1");
    assertEquals(testHandlerA.messages.length, 1);
    assertEquals(testHandlerA.messages[0], "Info message1");
    assertEquals(testHandlerB.messages.length, 0);

    logger.handlers = [testHandlerA, testHandlerB];

    logger.info("message2");
    assertEquals(testHandlerA.messages.length, 2);
    assertEquals(testHandlerA.messages[1], "Info message2");
    assertEquals(testHandlerB.messages.length, 1);
    assertEquals(testHandlerB.messages[0], "Info message2");

    logger.handlers = [testHandlerB];

    logger.info("message3");
    assertEquals(testHandlerA.messages.length, 2);
    assertEquals(testHandlerB.messages.length, 2);
    assertEquals(testHandlerB.messages[1], "Info message3");

    logger.handlers = [];
    logger.info("message4");
    assertEquals(testHandlerA.messages.length, 2);
    assertEquals(testHandlerB.messages.length, 2);
  },
});

Deno.test("trace", function (): void {
  const logLevel = logLevels.trace;
  const handler = new TestHandler(logLevel);

  const logger = new Logger(logLevel, {
    handlers: [handler],
  });

  logger.trace("foo");
  logger.trace("bar", 1, 2);

  assertEquals(
    handler.messages,
    [`${logLevel.name} foo`, `${logLevel.name} bar 1 2`],
  );
});

Deno.test("debug", function (): void {
  const logLevel = logLevels.debug;
  const handler = new TestHandler(logLevel);

  const logger = new Logger(logLevel, {
    handlers: [handler],
  });

  logger.debug("foo");
  logger.debug("bar", 1, 2);

  assertEquals(
    handler.messages,
    [`${logLevel.name} foo`, `${logLevel.name} bar 1 2`],
  );
});

Deno.test("info", function (): void {
  const logLevel = logLevels.info;
  const handler = new TestHandler(logLevel);

  const logger = new Logger(logLevel, {
    handlers: [handler],
  });

  logger.info("foo");
  logger.info("bar", 1, 2);

  assertEquals(
    handler.messages,
    [`${logLevel.name} foo`, `${logLevel.name} bar 1 2`],
  );
});

Deno.test("warn", function (): void {
  const logLevel = logLevels.warn;
  const handler = new TestHandler(logLevel);

  const logger = new Logger(logLevel, {
    handlers: [handler],
  });

  logger.warn("foo");
  logger.warn("bar", 1, 2);

  assertEquals(
    handler.messages,
    [`${logLevel.name} foo`, `${logLevel.name} bar 1 2`],
  );
});

Deno.test("error", function (): void {
  const logLevel = logLevels.error;
  const handler = new TestHandler(logLevel);

  const logger = new Logger(logLevel, {
    handlers: [handler],
  });

  logger.error("foo");
  logger.error("bar", 1, 2);

  assertEquals(
    handler.messages,
    [`${logLevel.name} foo`, `${logLevel.name} bar 1 2`],
  );
});
