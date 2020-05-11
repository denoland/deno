// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { test } = Deno;
import { assertEquals, assert } from "../testing/asserts.ts";
import { LogRecord, Logger } from "./logger.ts";
import { LogLevels, LevelName } from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

class TestHandler extends BaseHandler {
  public messages: string[] = [];
  public records: LogRecord[] = [];

  handle(record: LogRecord): void {
    this.records.push(record);
    super.handle(record);
  }

  public log(str: string): void {
    this.messages.push(str);
  }
}

test("simpleLogger", function (): void {
  const handler = new TestHandler("DEBUG");
  let logger = new Logger("DEBUG");

  assertEquals(logger.level, LogLevels.DEBUG);
  assertEquals(logger.levelName, "DEBUG");
  assertEquals(logger.handlers, []);

  logger = new Logger("DEBUG", [handler]);

  assertEquals(logger.handlers, [handler]);
});

test("customHandler", function (): void {
  const handler = new TestHandler("DEBUG");
  const logger = new Logger("DEBUG", [handler]);

  const inlineData = logger.debug("foo", 1, 2);

  const record = handler.records[0];
  assertEquals(record.msg, "foo");
  assertEquals(record.args, [1, 2]);
  assertEquals(record.level, LogLevels.DEBUG);
  assertEquals(record.levelName, "DEBUG");

  assertEquals(handler.messages, ["DEBUG foo"]);
  assertEquals(inlineData!.msg, "foo");
  assertEquals(inlineData!.args, [1, 2]);
});

test("logFunctions", function (): void {
  const doLog = (level: LevelName): TestHandler => {
    const handler = new TestHandler(level);
    const logger = new Logger(level, [handler]);
    const debugData = logger.debug("foo");
    const infoData = logger.info("bar");
    const warningData = logger.warning("baz");
    const errorData = logger.error("boo");
    const criticalData = logger.critical("doo");
    assertEquals(debugData, "foo");
    assertEquals(infoData, "bar");
    assertEquals(warningData, "baz");
    assertEquals(errorData, "boo");
    assertEquals(criticalData, "doo");
    return handler;
  };

  let handler: TestHandler;
  handler = doLog("DEBUG");

  assertEquals(handler.messages, [
    "DEBUG foo",
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo",
  ]);

  handler = doLog("INFO");

  assertEquals(handler.messages, [
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo",
  ]);

  handler = doLog("WARNING");

  assertEquals(handler.messages, ["WARNING baz", "ERROR boo", "CRITICAL doo"]);

  handler = doLog("ERROR");

  assertEquals(handler.messages, ["ERROR boo", "CRITICAL doo"]);

  handler = doLog("CRITICAL");

  assertEquals(handler.messages, ["CRITICAL doo"]);
});

test("String resolver fn will not execute if msg will not be logged", function (): void {
  const handler = new TestHandler("ERROR");
  const logger = new Logger("ERROR", [handler]);
  let called = false;

  const expensiveFunction = (): string => {
    called = true;
    return "expensive function result";
  };

  const inlineData = logger.debug(expensiveFunction, 1, 2);
  assert(!called);
  assert(inlineData === undefined);
});

test("String resolver fn resolves as expected", function (): void {
  const handler = new TestHandler("ERROR");
  const logger = new Logger("ERROR", [handler]);
  const expensiveFunction = (x: number): string => {
    return "expensive function result " + x;
  };

  const firstInlineData = logger.error(() => expensiveFunction(5));
  const secondInlineData = logger.error(() => expensiveFunction(5), 1, "abc");
  assertEquals(firstInlineData, "expensive function result 5");
  assertEquals(secondInlineData, {
    msg: "expensive function result 5",
    args: [1, "abc"],
  });
});

test("All types map correctly to log strings and are returned as is", function (): void {
  const handler = new TestHandler("DEBUG");
  const logger = new Logger("DEBUG", [handler]);
  const sym: Symbol = Symbol();
  const syma: Symbol = Symbol("a");
  const fn = (): string => {
    return "abc";
  };

  // string
  assertEquals(logger.debug("abc"), "abc");
  assertEquals(logger.debug("def", 1), { msg: "def", args: [1] });
  // null
  assertEquals(logger.info(null), null);
  assertEquals(logger.info(null, 1), { msg: null, args: [1] });
  // number
  assertEquals(logger.warning(3), 3);
  assertEquals(logger.warning(3, 1), { msg: 3, args: [1] });
  // bigint
  assertEquals(logger.error(5n), 5n);
  assertEquals(logger.error(5n, 1), { msg: 5n, args: [1] });
  // boolean
  assertEquals(logger.critical(true), true);
  assertEquals(logger.critical(true, 1), { msg: true, args: [1] });
  // undefined
  assertEquals(logger.debug(undefined), undefined);
  assertEquals(logger.debug(undefined, 1), { msg: undefined, args: [1] });
  // symbol
  assertEquals(logger.info(sym), sym);
  assertEquals(logger.info(syma, 1), { msg: syma, args: [1] });
  // function
  assertEquals(logger.warning(fn), "abc");
  assertEquals(logger.warning(fn, 1), { msg: "abc", args: [1] });
  // object
  assertEquals(logger.error({ payload: "data", other: 123 }), {
    payload: "data",
    other: 123,
  });
  assertEquals(logger.error({ payload: "data", other: 123 }, 1), {
    msg: { payload: "data", other: 123 },
    args: [1],
  });

  assertEquals(handler.messages[0], "DEBUG abc");
  assertEquals(handler.messages[1], "DEBUG def");
  assertEquals(handler.messages[2], "INFO null");
  assertEquals(handler.messages[3], "INFO null");
  assertEquals(handler.messages[4], "WARNING 3");
  assertEquals(handler.messages[5], "WARNING 3");
  assertEquals(handler.messages[6], "ERROR 5");
  assertEquals(handler.messages[7], "ERROR 5");
  assertEquals(handler.messages[8], "CRITICAL true");
  assertEquals(handler.messages[9], "CRITICAL true");
  assertEquals(handler.messages[10], "DEBUG undefined");
  assertEquals(handler.messages[11], "DEBUG undefined");
  assertEquals(handler.messages[12], "INFO Symbol()");
  assertEquals(handler.messages[13], "INFO Symbol(a)");
  assertEquals(handler.messages[14], "WARNING abc");
  assertEquals(handler.messages[15], "WARNING abc");
  assertEquals(handler.messages[16], 'ERROR {"payload":"data","other":123}');
  assertEquals(handler.messages[17], 'ERROR {"payload":"data","other":123}');
});
