#!/usr/bin/env deno --allow-run
import { args } from "deno";
import { format, testFormat } from "./format.ts";
import { lint } from "./lint.ts";

async function main() {
  const command = args[1];
  if (!command) {
    const message = [
      "usage: ./denotools.ts <command>",
      "commands:",
      "  - format",
      "  - test_format",
      "  - lint"
    ].join("\n");
    return console.log(message);
  }
  switch (command) {
    case "format":
      return format();
    case "test_format":
      return testFormat();
    case "lint":
      return lint();
    default:
      throw new Error(`Unknown command ${command}`);
  }
}

main();
