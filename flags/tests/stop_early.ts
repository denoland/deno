import { test, assertEqual } from "https://deno.land/x/testing/testing.ts";
import { parse } from "../index.ts";

// stops parsing on the first non-option when stopEarly is set
test(function stopParsing() {
  const argv = parse(["--aaa", "bbb", "ccc", "--ddd"], {
    stopEarly: true
  });

  assertEqual(argv, {
    aaa: "bbb",
    _: ["ccc", "--ddd"]
  });
});
