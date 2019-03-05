// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assertEqual, test } from "../testing/mod.ts";
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

  assertEqual(logger.level, LogLevel.DEBUG);
  assertEqual(logger.levelName, "DEBUG");
  assertEqual(logger.handlers, []);

  logger = new Logger("DEBUG", [handler]);

  assertEqual(logger.handlers, [handler]);
});

test(function customHandler() {
  const handler = new TestHandler("DEBUG");
  const logger = new Logger("DEBUG", [handler]);

  logger.debug("foo", 1, 2);

  assertEqual(handler.records, [
    {
      msg: "foo",
      args: [1, 2],
      datetime: null,
      level: LogLevel.DEBUG,
      levelName: "DEBUG"
    }
  ]);

  assertEqual(handler.messages, ["DEBUG foo"]);
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

  assertEqual(handler.messages, [
    "DEBUG foo",
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo"
  ]);

  doLog("INFO");

  assertEqual(handler.messages, [
    "INFO bar",
    "WARNING baz",
    "ERROR boo",
    "CRITICAL doo"
  ]);

  doLog("WARNING");

  assertEqual(handler.messages, ["WARNING baz", "ERROR boo", "CRITICAL doo"]);

  doLog("ERROR");

  assertEqual(handler.messages, ["ERROR boo", "CRITICAL doo"]);

  doLog("CRITICAL");

  assertEqual(handler.messages, ["CRITICAL doo"]);
});
