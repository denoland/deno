// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertMatch } from "../testing/asserts.ts";
import { BaseHandler } from "./handlers.ts";
import { LevelName, LogLevels } from "./levels.ts";
import { Logger, LogRecord } from "./logger.ts";

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

Deno.test({
  name: "Logger names can be output in logs",
  fn() {
    const handlerNoName = new TestHandler("DEBUG");
    const handlerWithLoggerName = new TestHandler("DEBUG", {
      formatter: "[{loggerName}] {levelName} {msg}",
    });

    const logger = new Logger("config", "DEBUG", {
      handlers: [handlerNoName, handlerWithLoggerName],
    });
    logger.debug("hello");
    assertEquals(handlerNoName.messages[0], "DEBUG hello");
    assertEquals(handlerWithLoggerName.messages[0], "[config] DEBUG hello");
  },
});

Deno.test("simpleLogger", function (): void {
  const handler = new TestHandler("DEBUG");
  let logger = new Logger("default", "DEBUG");

  assertEquals(logger.level, LogLevels.DEBUG);
  assertEquals(logger.levelName, "DEBUG");
  assertEquals(logger.handlers, []);

  logger = new Logger("default", "DEBUG", { handlers: [handler] });

  assertEquals(logger.handlers, [handler]);
});

Deno.test("customHandler", function (): void {
  const handler = new TestHandler("DEBUG");
  const logger = new Logger("default", "DEBUG", { handlers: [handler] });

  const inlineData: string = logger.debug("foo", 1, 2);

  const record = handler.records[0];
  assertEquals(record.msg, "foo");
  assertEquals(record.args, [1, 2]);
  assertEquals(record.level, LogLevels.DEBUG);
  assertEquals(record.levelName, "DEBUG");

  assertEquals(handler.messages, ["DEBUG foo"]);
  assertEquals(inlineData!, "foo");
});

Deno.test("logFunctions", function (): void {
  const doLog = (level: LevelName): TestHandler => {
    const handler = new TestHandler(level);
    const logger = new Logger("default", level, { handlers: [handler] });
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

Deno.test(
  "String resolver fn will not execute if msg will not be logged",
  function (): void {
    const handler = new TestHandler("ERROR");
    const logger = new Logger("default", "ERROR", { handlers: [handler] });
    let called = false;

    const expensiveFunction = (): string => {
      called = true;
      return "expensive function result";
    };

    const inlineData: string | undefined = logger.debug(
      expensiveFunction,
      1,
      2,
    );
    assert(!called);
    assertEquals(inlineData, undefined);
  },
);

Deno.test("String resolver fn resolves as expected", function (): void {
  const handler = new TestHandler("ERROR");
  const logger = new Logger("default", "ERROR", { handlers: [handler] });
  const expensiveFunction = (x: number): string => {
    return "expensive function result " + x;
  };

  const firstInlineData = logger.error(() => expensiveFunction(5));
  const secondInlineData = logger.error(() => expensiveFunction(12), 1, "abc");
  assertEquals(firstInlineData, "expensive function result 5");
  assertEquals(secondInlineData, "expensive function result 12");
});

Deno.test(
  "All types map correctly to log strings and are returned as is",
  function (): void {
    const handler = new TestHandler("DEBUG");
    const logger = new Logger("default", "DEBUG", { handlers: [handler] });
    const sym = Symbol();
    const syma = Symbol("a");
    const fn = (): string => {
      return "abc";
    };

    // string
    const data1: string = logger.debug("abc");
    assertEquals(data1, "abc");
    const data2: string = logger.debug("def", 1);
    assertEquals(data2, "def");
    assertEquals(handler.messages[0], "DEBUG abc");
    assertEquals(handler.messages[1], "DEBUG def");

    // null
    const data3: null = logger.info(null);
    assertEquals(data3, null);
    const data4: null = logger.info(null, 1);
    assertEquals(data4, null);
    assertEquals(handler.messages[2], "INFO null");
    assertEquals(handler.messages[3], "INFO null");

    // number
    const data5: number = logger.warning(3);
    assertEquals(data5, 3);
    const data6: number = logger.warning(3, 1);
    assertEquals(data6, 3);
    assertEquals(handler.messages[4], "WARNING 3");
    assertEquals(handler.messages[5], "WARNING 3");

    // bigint
    const data7: bigint = logger.error(5n);
    assertEquals(data7, 5n);
    const data8: bigint = logger.error(5n, 1);
    assertEquals(data8, 5n);
    assertEquals(handler.messages[6], "ERROR 5");
    assertEquals(handler.messages[7], "ERROR 5");

    // boolean
    const data9: boolean = logger.critical(true);
    assertEquals(data9, true);
    const data10: boolean = logger.critical(false, 1);
    assertEquals(data10, false);
    assertEquals(handler.messages[8], "CRITICAL true");
    assertEquals(handler.messages[9], "CRITICAL false");

    // undefined
    const data11: undefined = logger.debug(undefined);
    assertEquals(data11, undefined);
    const data12: undefined = logger.debug(undefined, 1);
    assertEquals(data12, undefined);
    assertEquals(handler.messages[10], "DEBUG undefined");
    assertEquals(handler.messages[11], "DEBUG undefined");

    // symbol
    const data13: symbol = logger.info(sym);
    assertEquals(data13, sym);
    const data14: symbol = logger.info(syma, 1);
    assertEquals(data14, syma);
    assertEquals(handler.messages[12], "INFO Symbol()");
    assertEquals(handler.messages[13], "INFO Symbol(a)");

    // function
    const data15: string | undefined = logger.warning(fn);
    assertEquals(data15, "abc");
    const data16: string | undefined = logger.warning(fn, 1);
    assertEquals(data16, "abc");
    assertEquals(handler.messages[14], "WARNING abc");
    assertEquals(handler.messages[15], "WARNING abc");

    // object
    const data17: { payload: string; other: number } = logger.error({
      payload: "data",
      other: 123,
    });
    assertEquals(data17, {
      payload: "data",
      other: 123,
    });
    const data18: { payload: string; other: number } = logger.error(
      { payload: "data", other: 123 },
      1,
    );
    assertEquals(data18, {
      payload: "data",
      other: 123,
    });
    assertEquals(handler.messages[16], 'ERROR {"payload":"data","other":123}');
    assertEquals(handler.messages[17], 'ERROR {"payload":"data","other":123}');

    // error
    const error = new RangeError("Uh-oh!");
    const data19: RangeError = logger.error(error);
    assertEquals(data19, error);
    const messages19 = handler.messages[18].split("\n");
    assertEquals(messages19[0], `ERROR ${error.name}: ${error.message}`);
    assertMatch(messages19[1], /^\s+at file:.*\d+:\d+$/);
  },
);
