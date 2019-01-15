import { assertEqual, test } from "../testing/mod.ts";
import { color } from "./mod.ts";
import "./example.ts";

test(function singleColor() {
  assertEqual(color.red("Hello world"), "[31mHello world[39m");
});

test(function doubleColor() {
  assertEqual(color.red.bgBlue("Hello world"),
    "[44m[31mHello world[39m[49m");
});

test(function newLinesContinueColors() {
  assertEqual(color.red("Hello\nworld"),
    "[31mHello[39m\n[31mworld[39m");
  assertEqual(color.red("Hello\r\nworld"),
    "[31mHello[39m\r\n[31mworld[39m");
  assertEqual(color.red("Hello\n\nworld"),
    "[31mHello[39m\n[31m[39m\n[31mworld[39m");
});

test(function replacesCloseCharacters() {
  assertEqual(color.red("Hel[39mlo"), "[31mHel[31mlo[39m");
});
