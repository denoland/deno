// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { xrun } from "../prettier/util.ts";
import { red, green } from "../fmt/colors.ts";

/**
 * Checks whether any source file is changed since the given start time.
 * If some files are changed, this function exits with 1.
 */
async function main(startTime: number): Promise<void> {
  console.log("test checkSourceFileChanges ...");
  const changed = new TextDecoder()
    .decode(await xrun({ args: ["git", "ls-files"], stdout: "piped" }).output())
    .trim()
    .split("\n")
    .filter(file => {
      const stat = Deno.lstatSync(file);
      if (stat != null) {
        return (stat as any).modified * 1000 > startTime;
      }
    });
  if (changed.length > 0) {
    console.log(red("FAILED"));
    console.log(
      `Error: Some source files are modified during test: ${changed.join(", ")}`
    );
    Deno.exit(1);
  } else {
    console.log(green("ok"));
  }
}

main(parseInt(Deno.args[1]));
