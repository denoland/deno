// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
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

test(function simpleLogger() {
  const handler = new TestHandler("DEBUG");
  let logger = new Logger("DEBUG");

  assertEq(logger.level, LogLevel.DEBUG);
  assertEq(logger.levelName, "DEBUG");
  assertEq(logger.handlers, []);

  logger = new Logger("DEBUG", [handler]);

  assertEq(logger.handlers, [handler]);
});

test(function customHandler() {
  const handler = new TestHandler("DEBUG");
  const logger = new Logger("DEBUG", [handler]);

  logger.debug("foo", 1, 2);

  assertEq(handler.records, [
    {
      msg: "foo",
      args: [1, 2],
      datetime: null,
      level: LogLevel.DEBUG,
      levelName: "DEBUG"
    }
  ]);

  assertEq(handler.messages, ["DEBUG foo"]);
});

test(function logFunctions() {
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

  assertEq(handler.messages, [
    "DEBUG foo",
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo"
  ]);

  doLog("INFO");

  assertEq(handler.messages, [
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo"
  ]);

  doLog("WARNING");

  assertEq(handler.messages, ["WARNING baz", "ERROR boo", "CRITICAL doo"]);

  doLog("ERROR");

  assertEq(handler.messages, ["ERROR boo", "CRITICAL doo"]);

  doLog("CRITICAL");

  assertEq(handler.messages, ["CRITICAL doo"]);
});
