import { assertEquals, loadTestLibrary } from "./common.js";

const coerce = loadTestLibrary();

Deno.test("napi coerce bool", function () {
  assertEquals(coerce.test_coerce_bool(true), true);
  assertEquals(coerce.test_coerce_bool(false), false);
  assertEquals(coerce.test_coerce_bool(0), false);
  assertEquals(coerce.test_coerce_bool(69), true);
  assertEquals(coerce.test_coerce_bool(Number.MAX_SAFE_INTEGER), true);
  assertEquals(coerce.test_coerce_bool(new Array(10)), true);
  assertEquals(coerce.test_coerce_bool("Hello, Deno!"), true);
  assertEquals(coerce.test_coerce_bool(Symbol("[[test]]")), true);
  assertEquals(coerce.test_coerce_bool({}), true);
  assertEquals(coerce.test_coerce_bool(() => false), true);
  assertEquals(coerce.test_coerce_bool(undefined), false);
  assertEquals(coerce.test_coerce_bool(null), false);
});

Deno.test("napi coerce number", function () {
  assertEquals(coerce.test_coerce_number(true), 1);
  assertEquals(coerce.test_coerce_number(false), 0);
  assertEquals(coerce.test_coerce_number(0), 0);
  assertEquals(coerce.test_coerce_number(69), 69);
  assertEquals(coerce.test_coerce_number(""), 0);
  assertEquals(
    coerce.test_coerce_number(Number.MAX_SAFE_INTEGER),
    Number.MAX_SAFE_INTEGER,
  );
  assertEquals(coerce.test_coerce_number(new Array(10)), NaN);
  assertEquals(coerce.test_coerce_number("Hello, Deno!"), NaN);
  assertEquals(coerce.test_coerce_number({}), NaN);
  assertEquals(coerce.test_coerce_number(() => false), NaN);
  assertEquals(coerce.test_coerce_number(undefined), NaN);
  assertEquals(coerce.test_coerce_number(null), 0);
});

Deno.test("napi coerce string", function () {
  assertEquals(coerce.test_coerce_string(true), "true");
  assertEquals(coerce.test_coerce_string(false), "false");
  assertEquals(coerce.test_coerce_string(0), "0");
  assertEquals(coerce.test_coerce_string(69), "69");
  assertEquals(coerce.test_coerce_string(""), "");
  assertEquals(
    coerce.test_coerce_string(Number.MAX_SAFE_INTEGER),
    "9007199254740991",
  );
  assertEquals(coerce.test_coerce_string(new Array(10)), ",,,,,,,,,");
  assertEquals(coerce.test_coerce_string("Hello, Deno!"), "Hello, Deno!");
  assertEquals(coerce.test_coerce_string({}), "[object Object]");
  assertEquals(coerce.test_coerce_string(() => false), "() => false");
  assertEquals(coerce.test_coerce_string(undefined), "undefined");
  assertEquals(coerce.test_coerce_string(null), "null");
});
