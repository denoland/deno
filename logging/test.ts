import { remove, open, readAll } from "deno";
import { assertEqual, test } from "../testing/mod.ts";
import * as log from "index.ts";
import { FileHandler } from "./handlers.ts";

// TODO: establish something more sophisticated
let testOutput = "";

class TestHandler extends log.handlers.BaseHandler {
  constructor(levelName: string) {
    super(levelName);
  }

  log(msg: string) {
    testOutput += `${msg}\n`;
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
