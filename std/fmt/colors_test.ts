// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import * as c from "./colors.ts";
import "../examples/colors.ts";

Deno.test(function singleColor(): void {
  assertEquals(c.red("foo bar"), "[31mfoo bar[39m");
});

Deno.test(function doubleColor(): void {
  assertEquals(c.bgBlue(c.red("foo bar")), "[44m[31mfoo bar[39m[49m");
});

Deno.test(function replacesCloseCharacters(): void {
  assertEquals(c.red("Hel[39mlo"), "[31mHel[31mlo[39m");
});

Deno.test(function enablingColors(): void {
  assertEquals(c.getColorEnabled(), true);
  c.setColorEnabled(false);
  assertEquals(c.bgBlue(c.red("foo bar")), "foo bar");
  c.setColorEnabled(true);
  assertEquals(c.red("foo bar"), "[31mfoo bar[39m");
});

Deno.test(function testBold(): void {
  assertEquals(c.bold("foo bar"), "[1mfoo bar[22m");
});

Deno.test(function testDim(): void {
  assertEquals(c.dim("foo bar"), "[2mfoo bar[22m");
});

Deno.test(function testItalic(): void {
  assertEquals(c.italic("foo bar"), "[3mfoo bar[23m");
});

Deno.test(function testUnderline(): void {
  assertEquals(c.underline("foo bar"), "[4mfoo bar[24m");
});

Deno.test(function testInverse(): void {
  assertEquals(c.inverse("foo bar"), "[7mfoo bar[27m");
});

Deno.test(function testHidden(): void {
  assertEquals(c.hidden("foo bar"), "[8mfoo bar[28m");
});

Deno.test(function testStrikethrough(): void {
  assertEquals(c.strikethrough("foo bar"), "[9mfoo bar[29m");
});

Deno.test(function testBlack(): void {
  assertEquals(c.black("foo bar"), "[30mfoo bar[39m");
});

Deno.test(function testRed(): void {
  assertEquals(c.red("foo bar"), "[31mfoo bar[39m");
});

Deno.test(function testGreen(): void {
  assertEquals(c.green("foo bar"), "[32mfoo bar[39m");
});

Deno.test(function testYellow(): void {
  assertEquals(c.yellow("foo bar"), "[33mfoo bar[39m");
});

Deno.test(function testBlue(): void {
  assertEquals(c.blue("foo bar"), "[34mfoo bar[39m");
});

Deno.test(function testMagenta(): void {
  assertEquals(c.magenta("foo bar"), "[35mfoo bar[39m");
});

Deno.test(function testCyan(): void {
  assertEquals(c.cyan("foo bar"), "[36mfoo bar[39m");
});

Deno.test(function testWhite(): void {
  assertEquals(c.white("foo bar"), "[37mfoo bar[39m");
});

Deno.test(function testGray(): void {
  assertEquals(c.gray("foo bar"), "[90mfoo bar[39m");
});

Deno.test(function testBgBlack(): void {
  assertEquals(c.bgBlack("foo bar"), "[40mfoo bar[49m");
});

Deno.test(function testBgRed(): void {
  assertEquals(c.bgRed("foo bar"), "[41mfoo bar[49m");
});

Deno.test(function testBgGreen(): void {
  assertEquals(c.bgGreen("foo bar"), "[42mfoo bar[49m");
});

Deno.test(function testBgYellow(): void {
  assertEquals(c.bgYellow("foo bar"), "[43mfoo bar[49m");
});

Deno.test(function testBgBlue(): void {
  assertEquals(c.bgBlue("foo bar"), "[44mfoo bar[49m");
});

Deno.test(function testBgMagenta(): void {
  assertEquals(c.bgMagenta("foo bar"), "[45mfoo bar[49m");
});

Deno.test(function testBgCyan(): void {
  assertEquals(c.bgCyan("foo bar"), "[46mfoo bar[49m");
});

Deno.test(function testBgWhite(): void {
  assertEquals(c.bgWhite("foo bar"), "[47mfoo bar[49m");
});
