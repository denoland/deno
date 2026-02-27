// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals, test } from "checkin:testing";
import { getCallSite } from "checkin:callsite";

test(function testCallSiteOps() {
  const callSite = getCallSite();
  assertEquals(callSite.fileName, "test:///unit/callsite_test.ts");
  assertEquals(callSite.lineNumber, 6);
  assertEquals(callSite.columnNumber, 20);
});

test(function testCallSiteOpEval() {
  const callSite = eval("getCallSite()");
  assertEquals(callSite.fileName, "[eval]");
  assertEquals(callSite.lineNumber, 1);
  assertEquals(callSite.columnNumber, 1);
});
