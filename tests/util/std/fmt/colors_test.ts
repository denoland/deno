// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../assert/mod.ts";
import * as c from "./colors.ts";

Deno.test("reset", function () {
  assertEquals(c.reset("foo bar"), "[0mfoo bar[0m");
});

Deno.test("single color", function () {
  assertEquals(c.red("foo bar"), "[31mfoo bar[39m");
});

Deno.test("double color", function () {
  assertEquals(c.bgBlue(c.red("foo bar")), "[44m[31mfoo bar[39m[49m");
});

Deno.test("replaces close characters", function () {
  assertEquals(c.red("Hel[39mlo"), "[31mHel[31mlo[39m");
});

Deno.test("enabling colors", function () {
  assertEquals(c.getColorEnabled(), true);
  c.setColorEnabled(false);
  assertEquals(c.bgBlue(c.red("foo bar")), "foo bar");
  c.setColorEnabled(true);
  assertEquals(c.red("foo bar"), "[31mfoo bar[39m");
});

Deno.test("test bold", function () {
  assertEquals(c.bold("foo bar"), "[1mfoo bar[22m");
});

Deno.test("test dim", function () {
  assertEquals(c.dim("foo bar"), "[2mfoo bar[22m");
});

Deno.test("test italic", function () {
  assertEquals(c.italic("foo bar"), "[3mfoo bar[23m");
});

Deno.test("test underline", function () {
  assertEquals(c.underline("foo bar"), "[4mfoo bar[24m");
});

Deno.test("test inverse", function () {
  assertEquals(c.inverse("foo bar"), "[7mfoo bar[27m");
});

Deno.test("test hidden", function () {
  assertEquals(c.hidden("foo bar"), "[8mfoo bar[28m");
});

Deno.test("test strikethrough", function () {
  assertEquals(c.strikethrough("foo bar"), "[9mfoo bar[29m");
});

Deno.test("test black", function () {
  assertEquals(c.black("foo bar"), "[30mfoo bar[39m");
});

Deno.test("test red", function () {
  assertEquals(c.red("foo bar"), "[31mfoo bar[39m");
});

Deno.test("test green", function () {
  assertEquals(c.green("foo bar"), "[32mfoo bar[39m");
});

Deno.test("test yellow", function () {
  assertEquals(c.yellow("foo bar"), "[33mfoo bar[39m");
});

Deno.test("test blue", function () {
  assertEquals(c.blue("foo bar"), "[34mfoo bar[39m");
});

Deno.test("test magenta", function () {
  assertEquals(c.magenta("foo bar"), "[35mfoo bar[39m");
});

Deno.test("test cyan", function () {
  assertEquals(c.cyan("foo bar"), "[36mfoo bar[39m");
});

Deno.test("test white", function () {
  assertEquals(c.white("foo bar"), "[37mfoo bar[39m");
});

Deno.test("test gray", function () {
  assertEquals(c.gray("foo bar"), "[90mfoo bar[39m");
});

Deno.test("test brightBlack", function () {
  assertEquals(c.brightBlack("foo bar"), "[90mfoo bar[39m");
});

Deno.test("test brightRed", function () {
  assertEquals(c.brightRed("foo bar"), "[91mfoo bar[39m");
});

Deno.test("test brightGreen", function () {
  assertEquals(c.brightGreen("foo bar"), "[92mfoo bar[39m");
});

Deno.test("test brightYellow", function () {
  assertEquals(c.brightYellow("foo bar"), "[93mfoo bar[39m");
});

Deno.test("test brightBlue", function () {
  assertEquals(c.brightBlue("foo bar"), "[94mfoo bar[39m");
});

Deno.test("test brightMagenta", function () {
  assertEquals(c.brightMagenta("foo bar"), "[95mfoo bar[39m");
});

Deno.test("test brightCyan", function () {
  assertEquals(c.brightCyan("foo bar"), "[96mfoo bar[39m");
});

Deno.test("test brightWhite", function () {
  assertEquals(c.brightWhite("foo bar"), "[97mfoo bar[39m");
});

Deno.test("test bgBlack", function () {
  assertEquals(c.bgBlack("foo bar"), "[40mfoo bar[49m");
});

Deno.test("test bgRed", function () {
  assertEquals(c.bgRed("foo bar"), "[41mfoo bar[49m");
});

Deno.test("test bgGreen", function () {
  assertEquals(c.bgGreen("foo bar"), "[42mfoo bar[49m");
});

Deno.test("test bgYellow", function () {
  assertEquals(c.bgYellow("foo bar"), "[43mfoo bar[49m");
});

Deno.test("test bgBlue", function () {
  assertEquals(c.bgBlue("foo bar"), "[44mfoo bar[49m");
});

Deno.test("test bgMagenta", function () {
  assertEquals(c.bgMagenta("foo bar"), "[45mfoo bar[49m");
});

Deno.test("test bgCyan", function () {
  assertEquals(c.bgCyan("foo bar"), "[46mfoo bar[49m");
});

Deno.test("test bgWhite", function () {
  assertEquals(c.bgWhite("foo bar"), "[47mfoo bar[49m");
});

Deno.test("test bgBrightBlack", function () {
  assertEquals(c.bgBrightBlack("foo bar"), "[100mfoo bar[49m");
});

Deno.test("test bgBrightRed", function () {
  assertEquals(c.bgBrightRed("foo bar"), "[101mfoo bar[49m");
});

Deno.test("test bgBrightGreen", function () {
  assertEquals(c.bgBrightGreen("foo bar"), "[102mfoo bar[49m");
});

Deno.test("test bgBrightYellow", function () {
  assertEquals(c.bgBrightYellow("foo bar"), "[103mfoo bar[49m");
});

Deno.test("test bgBrightBlue", function () {
  assertEquals(c.bgBrightBlue("foo bar"), "[104mfoo bar[49m");
});

Deno.test("test bgBrightMagenta", function () {
  assertEquals(c.bgBrightMagenta("foo bar"), "[105mfoo bar[49m");
});

Deno.test("test bgBrightCyan", function () {
  assertEquals(c.bgBrightCyan("foo bar"), "[106mfoo bar[49m");
});

Deno.test("test bgBrightWhite", function () {
  assertEquals(c.bgBrightWhite("foo bar"), "[107mfoo bar[49m");
});

Deno.test("test clamp using rgb8", function () {
  assertEquals(c.rgb8("foo bar", -10), "[38;5;0mfoo bar[39m");
});

Deno.test("test truncate using rgb8", function () {
  assertEquals(c.rgb8("foo bar", 42.5), "[38;5;42mfoo bar[39m");
});

Deno.test("test rgb8", function () {
  assertEquals(c.rgb8("foo bar", 42), "[38;5;42mfoo bar[39m");
});

Deno.test("test bgRgb8", function () {
  assertEquals(c.bgRgb8("foo bar", 42), "[48;5;42mfoo bar[49m");
});

Deno.test("test rgb24", function () {
  assertEquals(
    c.rgb24("foo bar", {
      r: 41,
      g: 42,
      b: 43,
    }),
    "[38;2;41;42;43mfoo bar[39m",
  );
});

Deno.test("test rgb24 number", function () {
  assertEquals(c.rgb24("foo bar", 0x070809), "[38;2;7;8;9mfoo bar[39m");
});

Deno.test("test bgRgb24", function () {
  assertEquals(
    c.bgRgb24("foo bar", {
      r: 41,
      g: 42,
      b: 43,
    }),
    "[48;2;41;42;43mfoo bar[49m",
  );
});

Deno.test("test bgRgb24 number", function () {
  assertEquals(c.bgRgb24("foo bar", 0x070809), "[48;2;7;8;9mfoo bar[49m");
});

// https://github.com/chalk/strip-ansi/blob/2b8c961e75760059699373f9a69101065c3ded3a/test.js#L4-L6
Deno.test("test stripColor", function () {
  assertEquals(
    c.stripColor(
      "\u001B[0m\u001B[4m\u001B[42m\u001B[31mfoo\u001B[39m\u001B[49m\u001B[24mfoo\u001B[0m",
    ),
    "foofoo",
  );
});
