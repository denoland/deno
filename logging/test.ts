import { assertEqual, test } from "https://deno.land/x/testing/testing.ts";

import * as logging from "index.ts";

// TODO: establish something more sophisticated

let testOutput = "";

class TestHandler extends logging.handlers.BaseHandler {
  _log(level, ...args) {
    testOutput += `${level} ${args[0]}\n`;
  }
}

logging.setup({
  handlers: {
    debug: {
      level: "DEBUG",
      class: TestHandler
    },

    info: {
      level: "INFO",
      class: TestHandler
    }
  },

  loggers: {
    default: {
      level: "DEBUG",
      handlers: ["debug"]
    },

    info: {
      level: "INFO",
      handlers: ["info"]
    }
  }
});

const logger = logging.getLogger("default");
const unknownLogger = logging.getLogger("info");

test(function basicTest() {
  logger.debug("I should be printed.");
  unknownLogger.debug("I should not be printed.");
  unknownLogger.info("And I should be printed as well.");

  const expectedOutput =
    "10 I should be printed.\n20 And I should be printed as well.\n";

  assertEqual(testOutput, expectedOutput);
});
