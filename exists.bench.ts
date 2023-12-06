import { existsSync } from "https://deno.land/std/fs/mod.ts";
import fs from "node:fs";

Deno.bench("std/existsSync", () => {
  runWithLargeStack(() => {
    existsSync("./not_exists");
  });
});

Deno.bench("node:fs existsSync", () => {
  runWithLargeStack(() => {
    fs.existsSync("./not_exists");
  });
});

function runWithLargeStack(fn: () => void, count = 100) {
  if (count == 0) {
    fn();
  } else {
    runWithLargeStack(fn, count - 1);
  }
}
