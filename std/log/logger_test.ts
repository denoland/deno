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
  assertEquals(inlineData.msg, "foo");
  assertEquals(inlineData.args, [1, 2]);
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
  const expensiveFunction = (): string => {
    return "expensive function result";
  };

  const firstInlineData = logger.error(expensiveFunction);
  const secondInlineData = logger.error(expensiveFunction, 1, "abc");
  assertEquals(firstInlineData, "expensive function result");
  assertEquals(secondInlineData, {
    msg: "expensive function result",
    args: [1, "abc"],
  });
});
