import { assertEqual, test } from "../testing/mod.ts";
import { color } from "./mod.ts";
import "./example.ts";

test(function singleColor() {
  assertEqual(color.red("Hello world"), "[31mHello world[39m");
});

test(function doubleColor() {
  assertEqual(color.red.bgBlue("Hello world"), "[44m[31mHello world[39m[49m");
});
