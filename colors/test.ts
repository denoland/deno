import { assert, test } from "../testing/mod.ts";
import { red, bgBlue, setEnabled, getEnabled } from "./mod.ts";
import "./example.ts";

test(function singleColor() {
  assert.equal(red("Hello world"), "[31mHello world[39m");
});

test(function doubleColor() {
  assert.equal(bgBlue(red("Hello world")), "[44m[31mHello world[39m[49m");
});

test(function replacesCloseCharacters() {
  assert.equal(red("Hel[39mlo"), "[31mHel[31mlo[39m");
});

test(function enablingColors() {
  assert.equal(getEnabled(), true);
  setEnabled(false);
  assert.equal(bgBlue(red("Hello world")), "Hello world");
});
