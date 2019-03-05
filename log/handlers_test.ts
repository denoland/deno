// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { assertEqual, test } from "../testing/mod.ts";
import { LogLevel, getLevelName, getLevelByName } from "./levels.ts";
import { BaseHandler } from "./handlers.ts";

class TestHandler extends BaseHandler {
  public messages: string[] = [];

  public log(str: string): void {
    this.messages.push(str);
  }
}

test(function simpleHandler() {
  const cases = new Map<number, string[]>([
    [
      LogLevel.DEBUG,
      [
        "DEBUG debug-test",
        "INFO info-test",
        "WARNING warning-test",
        "ERROR error-test",
        "CRITICAL critical-test"
      ]
    ],
    [
      LogLevel.INFO,
      [
        "INFO info-test",
        "WARNING warning-test",
        "ERROR error-test",
        "CRITICAL critical-test"
      ]
    ],
    [
      LogLevel.WARNING,
      ["WARNING warning-test", "ERROR error-test", "CRITICAL critical-test"]
    ],
    [LogLevel.ERROR, ["ERROR error-test", "CRITICAL critical-test"]],
    [LogLevel.CRITICAL, ["CRITICAL critical-test"]]
  ]);

  for (const [testCase, messages] of cases.entries()) {
    const testLevel = getLevelName(testCase);
    const handler = new TestHandler(testLevel);

    for (const levelName in LogLevel) {
      const level = getLevelByName(levelName);
      handler.handle({
        msg: `${levelName.toLowerCase()}-test`,
        args: [],
        datetime: new Date(),
        level: level,
        levelName: levelName
      });
    }

    assertEqual(handler.level, testCase);
    assertEqual(handler.levelName, testLevel);
    assertEqual(handler.messages, messages);
  }
});

test(function testFormatterAsString() {
  const handler = new TestHandler("DEBUG", {
    formatter: "test {levelName} {msg}"
  });

  handler.handle({
    msg: "Hello, world!",
    args: [],
    datetime: new Date(),
    level: LogLevel.DEBUG,
    levelName: "DEBUG"
  });

  assertEqual(handler.messages, ["test DEBUG Hello, world!"]);
});

test(function testFormatterAsFunction() {
  const handler = new TestHandler("DEBUG", {
    formatter: logRecord =>
      `fn formmatter ${logRecord.levelName} ${logRecord.msg}`
  });

  handler.handle({
    msg: "Hello, world!",
    args: [],
    datetime: new Date(),
    level: LogLevel.ERROR,
    levelName: "ERROR"
  });

  assertEqual(handler.messages, ["fn formmatter ERROR Hello, world!"]);
});
