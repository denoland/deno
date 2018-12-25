// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function urlSearchParamsInitString() {
  const init = "c=4&a=2&b=3&%C3%A1=1";
  const searchParams = new URLSearchParams(init);
  assert(
    init === searchParams.toString(),
    "The init query string does not match"
  );
});

test(function urlSearchParamsInitIterable() {
  const init = [["a", "54"], ["b", "true"]];
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.toString(), "a=54&b=true");
});

test(function urlSearchParamsInitRecord() {
  const init = { a: "54", b: "true" };
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.toString(), "a=54&b=true");
});

test(function urlSearchParamsAppendSuccess() {
  const searchParams = new URLSearchParams();
  searchParams.append("a", "true");
  assertEqual(searchParams.toString(), "a=true");
});

test(function urlSearchParamsDeleteSuccess() {
  const init = "a=54&b=true";
  const searchParams = new URLSearchParams(init);
  searchParams.delete("b");
  assertEqual(searchParams.toString(), "a=54");
});

test(function urlSearchParamsGetAllSuccess() {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.getAll("a"), ["54", "true"]);
  assertEqual(searchParams.getAll("b"), ["true"]);
  assertEqual(searchParams.getAll("c"), []);
});

test(function urlSearchParamsGetSuccess() {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.get("a"), "54");
  assertEqual(searchParams.get("b"), "true");
  assertEqual(searchParams.get("c"), null);
});

test(function urlSearchParamsHasSuccess() {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  assert(searchParams.has("a"));
  assert(searchParams.has("b"));
  assert(!searchParams.has("c"));
});

test(function urlSearchParamsSetReplaceFirstAndRemoveOthers() {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  searchParams.set("a", "false");
  assertEqual(searchParams.toString(), "a=false&b=true");
});

test(function urlSearchParamsSetAppendNew() {
  const init = "a=54&b=true&a=true";
  const searchParams = new URLSearchParams(init);
  searchParams.set("c", "foo");
  assertEqual(searchParams.toString(), "a=54&b=true&a=true&c=foo");
});

test(function urlSearchParamsSortSuccess() {
  const init = "c=4&a=2&b=3&a=1";
  const searchParams = new URLSearchParams(init);
  searchParams.sort();
  assertEqual(searchParams.toString(), "a=2&a=1&b=3&c=4");
});

test(function urlSearchParamsForEachSuccess() {
  const init = [["a", "54"], ["b", "true"]];
  const searchParams = new URLSearchParams(init);
  let callNum = 0;
  searchParams.forEach((value, key, parent) => {
    assertEqual(searchParams, parent);
    assertEqual(value, init[callNum][1]);
    assertEqual(key, init[callNum][0]);
    callNum++;
  });
  assertEqual(callNum, init.length);
});

test(function urlSearchParamsMissingName() {
  const init = "=4";
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.get(""), "4");
  assertEqual(searchParams.toString(), "=4");
});

test(function urlSearchParamsMissingValue() {
  const init = "4=";
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.get("4"), "");
  assertEqual(searchParams.toString(), "4=");
});

test(function urlSearchParamsMissingEqualSign() {
  const init = "4";
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.get("4"), "");
  assertEqual(searchParams.toString(), "4=");
});

test(function urlSearchParamsMissingPair() {
  const init = "c=4&&a=54&";
  const searchParams = new URLSearchParams(init);
  assertEqual(searchParams.toString(), "c=4&a=54");
});

// If pair does not contain exactly two items, then throw a TypeError.
// ref https://url.spec.whatwg.org/#interface-urlsearchparams
test(function urlSearchParamsShouldThrowTypeError() {
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

  assertEqual(hasThrown, 2);
});

test(function urlSearchParamsAppendArgumentsCheck() {
  const methodRequireOneParam = ["delete", "getAll", "get", "has", "forEach"];

  const methodRequireTwoParams = ["append", "set"];

  methodRequireOneParam.concat(methodRequireTwoParams).forEach(method => {
    const searchParams = new URLSearchParams();
    let hasThrown = 0;
    try {
      searchParams[method]();
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEqual(hasThrown, 2);
  });

  methodRequireTwoParams.forEach(method => {
    const searchParams = new URLSearchParams();
    let hasThrown = 0;
    try {
      searchParams[method]("foo");
      hasThrown = 1;
    } catch (err) {
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEqual(hasThrown, 2);
  });
});
