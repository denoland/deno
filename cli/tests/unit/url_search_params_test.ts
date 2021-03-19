// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals } from "./test_util.ts";

Deno.test("urlSearchParamsWithMultipleSpaces", function (): void {
  const init = { str: "this string has spaces in it" };
  const searchParams = new URLSearchParams(init).toString();
  assertEquals(searchParams, "str=this+string+has+spaces+in+it");
});

Deno.test("urlSearchParamsWithExclamation", function (): void {
  const init = [
    ["str", "hello, world!"],
  ];
  const searchParams = new URLSearchParams(init).toString();
  assertEquals(searchParams, "str=hello%2C+world%21");
});

Deno.test("urlSearchParamsWithQuotes", function (): void {
  const init = [
    ["str", "'hello world'"],
  ];
  const searchParams = new URLSearchParams(init).toString();
  assertEquals(searchParams, "str=%27hello+world%27");
});

Deno.test("urlSearchParamsWithBraket", function (): void {
  const init = [
    ["str", "(hello world)"],
  ];
  const searchParams = new URLSearchParams(init).toString();
  assertEquals(searchParams, "str=%28hello+world%29");
});

Deno.test("urlSearchParamsWithTilde", function (): void {
  const init = [
    ["str", "hello~world"],
  ];
  const searchParams = new URLSearchParams(init).toString();
  assertEquals(searchParams, "str=hello%7Eworld");
});

Deno.test("urlSearchParamsInitString", function (): void {
  const init = "c=4&a=2&b=3&%C3%A1=1";
  const searchParams = new URLSearchParams(init);
  assert(
    init === searchParams.toString(),
    "The init query string does not match",
  );
});

Deno.test("urlSearchParamsInitStringWithPlusCharacter", function (): void {
  let params = new URLSearchParams("q=a+b");
  assertEquals(params.toString(), "q=a+b");
  assertEquals(params.get("q"), "a b");

  params = new URLSearchParams("q=a+b+c");
  assertEquals(params.toString(), "q=a+b+c");
  assertEquals(params.get("q"), "a b c");
});

Deno.test("urlSearchParamsInitStringWithMalformedParams", function (): void {
  // These test cases are copied from Web Platform Tests
  // https://github.com/web-platform-tests/wpt/blob/54c6d64/url/urlsearchparams-constructor.any.js#L60-L80
  let params = new URLSearchParams("id=0&value=%");
  assert(params != null, "constructor returned non-null value.");
  assert(params.has("id"), 'Search params object has name "id"');
  assert(params.has("value"), 'Search params object has name "value"');
  assertEquals(params.get("id"), "0");
  assertEquals(params.get("value"), "%");

  params = new URLSearchParams("b=%2sf%2a");
  assert(params != null, "constructor returned non-null value.");
  assert(params.has("b"), 'Search params object has name "b"');
  assertEquals(params.get("b"), "%2sf*");

  params = new URLSearchParams("b=%2%2af%2a");
  assert(params != null, "constructor returned non-null value.");
  assert(params.has("b"), 'Search params object has name "b"');
  assertEquals(params.get("b"), "%2*f*");

  params = new URLSearchParams("b=%%2a");
  assert(params != null, "constructor returned non-null value.");
  assert(params.has("b"), 'Search params object has name "b"');
  assertEquals(params.get("b"), "%*");
});

Deno.test("urlSearchParamsInitIterable", function (): void {
  const init = [
    ["a", "54"],
    ["b", "true"],
  ];
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.toString(), "a=54&b=true");
});

Deno.test("urlSearchParamsInitRecord", function (): void {
  const init = { a: "54", b: "true" };
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.toString(), "a=54&b=true");
});

Deno.test("urlSearchParamsInit", function (): void {
  const params1 = new URLSearchParams("a=b");
  assertEquals(params1.toString(), "a=b");
  // deno-lint-ignore no-explicit-any
  const params2 = new URLSearchParams(params1 as any);
  assertEquals(params2.toString(), "a=b");
});

Deno.test("urlSearchParamsAppendSuccess", function (): void {
  const searchParams = new URLSearchParams();
  searchParams.append("a", "true");
  assertEquals(searchParams.toString(), "a=true");
});

Deno.test("urlSearchParamsDeleteSuccess", function (): void {
  const init = "a=54&b=true";
  const searchParams = new URLSearchParams(init);
  searchParams.delete("b");
  assertEquals(searchParams.toString(), "a=54");
});

Deno.test("urlSearchParamsGetAllSuccess", function (): void {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.getAll("a"), ["54", "true"]);
  assertEquals(searchParams.getAll("b"), ["true"]);
  assertEquals(searchParams.getAll("c"), []);
});

Deno.test("urlSearchParamsGetSuccess", function (): void {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.get("a"), "54");
  assertEquals(searchParams.get("b"), "true");
  assertEquals(searchParams.get("c"), null);
});

Deno.test("urlSearchParamsHasSuccess", function (): void {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  assert(searchParams.has("a"));
  assert(searchParams.has("b"));
  assert(!searchParams.has("c"));
});

Deno.test("urlSearchParamsSetReplaceFirstAndRemoveOthers", function (): void {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  searchParams.set("a", "false");
  assertEquals(searchParams.toString(), "a=false&b=true");
});

Deno.test("urlSearchParamsSetAppendNew", function (): void {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  searchParams.set("c", "foo");
  assertEquals(searchParams.toString(), "a=54&b=true&a=true&c=foo");
});

Deno.test("urlSearchParamsSortSuccess", function (): void {
  const init = "c=4&a=2&b=3&a=1";
  const searchParams = new URLSearchParams(init);
  searchParams.sort();
  assertEquals(searchParams.toString(), "a=2&a=1&b=3&c=4");
});

Deno.test("urlSearchParamsForEachSuccess", function (): void {
  const init = [
    ["a", "54"],
    ["b", "true"],
  ];
  const searchParams = new URLSearchParams(init);
  let callNum = 0;
  searchParams.forEach((value, key, parent): void => {
    assertEquals(searchParams, parent);
    assertEquals(value, init[callNum][1]);
    assertEquals(key, init[callNum][0]);
    callNum++;
  });
  assertEquals(callNum, init.length);
});

Deno.test("urlSearchParamsMissingName", function (): void {
  const init = "=4";
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.get(""), "4");
  assertEquals(searchParams.toString(), "=4");
});

Deno.test("urlSearchParamsMissingValue", function (): void {
  const init = "4=";
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.get("4"), "");
  assertEquals(searchParams.toString(), "4=");
});

Deno.test("urlSearchParamsMissingEqualSign", function (): void {
  const init = "4";
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.get("4"), "");
  assertEquals(searchParams.toString(), "4=");
});

Deno.test("urlSearchParamsMissingPair", function (): void {
  const init = "c=4&&a=54&";
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.toString(), "c=4&a=54");
});

Deno.test("urlSearchParamsForShortEncodedChar", function (): void {
  const init = { linefeed: "\n", tab: "\t" };
  const searchParams = new URLSearchParams(init);
  assertEquals(searchParams.toString(), "linefeed=%0A&tab=%09");
});

// If pair does not contain exactly two items, then throw a TypeError.
// ref https://url.spec.whatwg.org/#interface-urlsearchparams
Deno.test("urlSearchParamsShouldThrowTypeError", function (): void {
  let hasThrown = 0;

  try {
    new URLSearchParams([["1"]]);
    hasThrown = 1;
  } catch (err) {
    if (err instanceof TypeError) {
      hasThrown = 2;
    } else {
      hasThrown = 3;
    }
  }

  assertEquals(hasThrown, 2);

  try {
    new URLSearchParams([["1", "2", "3"]]);
    hasThrown = 1;
  } catch (err) {
    if (err instanceof TypeError) {
      hasThrown = 2;
    } else {
      hasThrown = 3;
    }
  }

  assertEquals(hasThrown, 2);
});

Deno.test("urlSearchParamsAppendArgumentsCheck", function (): void {
  const methodRequireOneParam = ["delete", "getAll", "get", "has", "forEach"];

  const methodRequireTwoParams = ["append", "set"];

  methodRequireOneParam
    .concat(methodRequireTwoParams)
    .forEach((method: string): void => {
      const searchParams = new URLSearchParams();
      let hasThrown = 0;
      try {
        // deno-lint-ignore no-explicit-any
        (searchParams as any)[method]();
        hasThrown = 1;
      } catch (err) {
        if (err instanceof TypeError) {
          hasThrown = 2;
        } else {
          hasThrown = 3;
        }
      }
      assertEquals(hasThrown, 2);
    });

  methodRequireTwoParams.forEach((method: string): void => {
    const searchParams = new URLSearchParams();
    let hasThrown = 0;
    try {
      // deno-lint-ignore no-explicit-any
      (searchParams as any)[method]("foo");
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEquals(hasThrown, 2);
  });
});

// ref: https://github.com/web-platform-tests/wpt/blob/master/url/urlsearchparams-delete.any.js
Deno.test("urlSearchParamsDeletingAppendedMultiple", function (): void {
  const params = new URLSearchParams();
  params.append("first", (1 as unknown) as string);
  assert(params.has("first"));
  assertEquals(params.get("first"), "1");
  params.delete("first");
  assertEquals(params.has("first"), false);
  params.append("first", (1 as unknown) as string);
  params.append("first", (10 as unknown) as string);
  params.delete("first");
  assertEquals(params.has("first"), false);
});

// ref: https://github.com/web-platform-tests/wpt/blob/master/url/urlsearchparams-constructor.any.js#L176-L182
Deno.test("urlSearchParamsCustomSymbolIterator", function (): void {
  const params = new URLSearchParams();
  params[Symbol.iterator] = function* (): IterableIterator<[string, string]> {
    yield ["a", "b"];
  };
  const params1 = new URLSearchParams((params as unknown) as string[][]);
  assertEquals(params1.get("a"), "b");
});

Deno.test("urlSearchParamsCustomSymbolIteratorWithNonStringParams", function (): void {
  const params = {};
  // deno-lint-ignore no-explicit-any
  (params as any)[Symbol.iterator] = function* (): IterableIterator<
    [number, number]
  > {
    yield [1, 2];
  };
  const params1 = new URLSearchParams((params as unknown) as string[][]);
  assertEquals(params1.get("1"), "2");
});

// If a class extends URLSearchParams, override one method should not change another's behavior.
Deno.test("urlSearchParamsOverridingAppendNotChangeConstructorAndSet", function (): void {
  let overridedAppendCalled = 0;
  class CustomSearchParams extends URLSearchParams {
    append(name: string, value: string): void {
      ++overridedAppendCalled;
      super.append(name, value);
    }
  }
  new CustomSearchParams("foo=bar");
  new CustomSearchParams([["foo", "bar"]]);
  new CustomSearchParams(new CustomSearchParams({ foo: "bar" }));
  new CustomSearchParams().set("foo", "bar");
  assertEquals(overridedAppendCalled, 0);
});

Deno.test("urlSearchParamsOverridingEntriesNotChangeForEach", function (): void {
  class CustomSearchParams extends URLSearchParams {
    *entries(): IterableIterator<[string, string]> {
      yield* [];
    }
  }
  let loopCount = 0;
  const params = new CustomSearchParams({ foo: "bar" });
  params.forEach(() => void ++loopCount);
  assertEquals(loopCount, 1);
});
