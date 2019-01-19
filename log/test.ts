import { remove, open, readAll } from "deno";
import { assertEqual, test } from "../testing/mod.ts";
import * as log from "./mod.ts";
import { FileHandler } from "./handlers.ts";

// constructor(levelName: string, options: HandlerOptions = {}) {
//   this.level = getLevelByName(levelName);
//   this.levelName = levelName;

//   this.formatter = options.formatter || DEFAULT_FORMATTER;
// }

class TestHandler extends log.handlers.BaseHandler {
  testOutput = "";

  log(msg: string) {
    this.testOutput += `${msg}\n`;
  }
}

test(function testDefaultlogMethods() {
  log.debug("Foobar");
  log.info("Foobar");
  log.warning("Foobar");
  log.error("Foobar");
  log.critical("Foobar");

  const logger = log.getLogger("");
  console.log(logger);
});

test(async function testDefaultFormatter() {
  await log.setup({
    handlers: {
      test: new TestHandler("DEBUG"),
    },

    loggers: {
      test: {
        level: "DEBUG",
        handlers: ["test"],
      },
    },
  });

  const logger = log.getLogger("test");
  const handler = log.getHandler("test");
  logger.debug("Hello, world!");
  assertEqual(handler.testOutput, "DEBUG Hello, world!\n");
});

test(async function testFormatterAsString() {
  await log.setup({
    handlers: {
      test: new TestHandler("DEBUG", {
        formatter: "test {levelName} {msg}",
      }),
    },

    loggers: {
      test: {
        level: "DEBUG",
        handlers: ["test"],
      },
    },
  });

  const logger = log.getLogger("test");
  const handler = log.getHandler("test");
  logger.debug("Hello, world!");
  assertEqual(handler.testOutput, "test DEBUG Hello, world!\n");
});

test(async function testFormatterAsFunction() {
  await log.setup({
    handlers: {
      test: new TestHandler("DEBUG", {
        formatter: logRecord => `fn formmatter ${logRecord.levelName} ${logRecord.msg}`,
      }),
    },

    loggers: {
      test: {
        level: "DEBUG",
        handlers: ["test"],
      },
    },
  });

  const logger = log.getLogger("test");
  const handler = log.getHandler("test");
  logger.error("Hello, world!");
  assertEqual(handler.testOutput, "fn formmatter ERROR Hello, world!\n");
});