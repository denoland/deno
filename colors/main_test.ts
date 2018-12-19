import { assertEqual, test } from "https://deno.land/x/testing/testing.ts";
import { color } from "./main";

test(function singleColor() {
  assertEqual(color.red("Hello world"), "[31mHello world[39m");
});

test(function doubleColor() {
  assertEqual(color.red.bgBlue("Hello world"), "[44m[31mHello world[39m[49m");
});
