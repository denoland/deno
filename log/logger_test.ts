// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { LogRecord, Logger } from "./logger.ts";
import { LogLevel } from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

class TestHandler extends BaseHandler {
  public messages: string[] = [];
  public records: LogRecord[] = [];

  handle(record: LogRecord): void {
    this.records.push({ ...record, datetime: null });
    super.handle(record);
  }

  public log(str: string): void {
    this.messages.push(str);
  }
}

test(function simpleLogger(): void {
  const handler = new TestHandler("DEBUG");
  let logger = new Logger("DEBUG");

  assertEquals(logger.level, LogLevel.DEBUG);
  assertEquals(logger.levelName, "DEBUG");
  assertEquals(logger.handlers, []);

  logger = new Logger("DEBUG", [handler]);

  assertEquals(logger.handlers, [handler]);
});

test(function customHandler(): void {
  const handler = new TestHandler("DEBUG");
  const logger = new Logger("DEBUG", [handler]);

  logger.debug("foo", 1, 2);

  assertEquals(handler.records, [
    {
      msg: "foo",
      args: [1, 2],
      datetime: null,
      level: LogLevel.DEBUG,
      levelName: "DEBUG"
    }
  ]);

  assertEquals(handler.messages, ["DEBUG foo"]);
});

test(function logFunctions(): void {
  let handler: TestHandler;

  const doLog = (level: string): void => {
    handler = new TestHandler(level);
    let logger = new Logger(level, [handler]);
    logger.debug("foo");
    logger.info("bar");
    logger.warning("baz");
    logger.error("boo");
    logger.critical("doo");
  };

  doLog("DEBUG");

  assertEquals(handler.messages, [
    "DEBUG foo",
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo"
  ]);

  doLog("INFO");

  assertEquals(handler.messages, [
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo"
  ]);

  doLog("WARNING");

  assertEquals(handler.messages, ["WARNING baz", "ERROR boo", "CRITICAL doo"]);

  doLog("ERROR");

  assertEquals(handler.messages, ["ERROR boo", "CRITICAL doo"]);

  doLog("CRITICAL");

  assertEquals(handler.messages, ["CRITICAL doo"]);
});
