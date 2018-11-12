// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, testPerm, assert, assertEqual } from "./test_util.ts";

test(function getItemFromUnknownKeyShouldReturnNull() {
  assertEqual(sessionStorage.getItem("unknonw_key"), null);
});

test(function getItemAfterSetItemShouldReturnGivenValue() {
  const value = "" + Math.random();
  sessionStorage.setItem("test_key", value);
  assertEqual(sessionStorage.getItem("test_key"), value);
});

test(function getItemAfterRemoveItemShouldReturnNull() {
  const value = "" + Math.random();
  sessionStorage.setItem("test_key", value);
  sessionStorage.removeItem("test_key");
  assertEqual(sessionStorage.getItem("test_key"), null);
});

test(function getItemAfterClearShouldReturnNull() {
  const value = "" + Math.random();
  sessionStorage.setItem("test_key", value);
  sessionStorage.clear();
  assertEqual(sessionStorage.getItem("test_key"), null);
});

test(function removeItemFromUnknownKeyShouldDoNothing() {
  assertEqual(sessionStorage.removeItem("unknonw_key"), undefined);
});

test(function keyMathodShouldReturnSetKey() {
  const key = "" + Math.random();

  sessionStorage.clear();
  sessionStorage.setItem(key, "value");
  assertEqual(sessionStorage.key(0), key);
});

test(function lengthShouldGrowWhenAddingItem() {
  const key = "" + Math.random();

  const previousLength = sessionStorage.length;
  sessionStorage.setItem(key, "value");
  assertEqual(sessionStorage.length, previousLength + 1);
});

test(function lengthShouldBe0WhenStorageIsCleared() {
  sessionStorage.clear();
  assertEqual(sessionStorage.length, 0);
});
