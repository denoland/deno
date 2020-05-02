// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals } from "./test_util.ts";

unitTest(function getItemFromUnknownKeyShouldReturnNull() {
  assertEquals(sessionStorage.getItem("unknonw_key"), null);
  assertEquals(sessionStorage["unknonw_key"], null);
});

unitTest(function getItemAfterSetItemShouldReturnGivenValue() {
  const value = "" + Math.random();
  sessionStorage.setItem("test_key", value);
  assertEquals(sessionStorage.getItem("test_key"), value);
  assertEquals(sessionStorage["test_key"], value);
});

unitTest(function getItemAfterRemoveItemShouldReturnNull() {
  const value = "" + Math.random();
  sessionStorage.setItem("test_key", value);
  sessionStorage.removeItem("test_key");
  assertEquals(sessionStorage.getItem("test_key"), null);
  assertEquals(sessionStorage["test_key"], null);
});

unitTest(function getItemAfterClearShouldReturnNull() {
  const value = "" + Math.random();
  sessionStorage.setItem("test_key", value);
  sessionStorage.clear();
  assertEquals(sessionStorage.getItem("test_key"), null);
  assertEquals(sessionStorage["test_key"], null);
});

unitTest(function removeItemFromUnknownKeyShouldDoNothing() {
  assertEquals(sessionStorage.removeItem("unknonw_key"), undefined);
  assertEquals(delete sessionStorage["unknonw_key"], true);
});

unitTest(function keyMethodShouldReturnSetKey() {
  const key = "" + Math.random();

  sessionStorage.clear();
  sessionStorage.setItem(key, "value");
  assertEquals(sessionStorage.key(0), key);
});

unitTest(function lengthShouldGrowWhenAddingItem() {
  const key = "" + Math.random();

  const expectedLength = sessionStorage.length + 1;
  sessionStorage[key] = "value";
  assertEquals(sessionStorage.length, expectedLength);
});

unitTest(function lengthShouldNotGrowWhenUpdatingExistingItem() {
  const key = "" + Math.random();

  const expectedLength = sessionStorage.length + 1;
  sessionStorage.setItem(key, "value");
  assertEquals(sessionStorage.length, expectedLength);
  sessionStorage[key] = "other value";
  assertEquals(sessionStorage.length, expectedLength);
});

unitTest(function lengthShouldShrinkWhenRemovingAnItem() {
  const key = "" + Math.random();

  const expectedLength = sessionStorage.length;
  sessionStorage[key] = "value";
  assertEquals(sessionStorage.length, expectedLength + 1);
  delete sessionStorage[key];
  assertEquals(sessionStorage.length, expectedLength);
});

unitTest(function lengthShouldBeZeroWhenStorageIsCleared() {
  sessionStorage.clear();
  assertEquals(sessionStorage.length, 0);
});

unitTest(function getItemFromUnknownKeyShouldReturnNull() {
  assertEquals(localStorage.getItem("unknonw_key"), null);
  assertEquals(localStorage["unknonw_key"], null);
});

unitTest(function getItemAfterSetItemShouldReturnGivenValue() {
  const value = "" + Math.random();
  localStorage.setItem("test_key", value);
  assertEquals(localStorage.getItem("test_key"), value);
  assertEquals(localStorage["test_key"], value);
});

unitTest(function getItemAfterRemoveItemShouldReturnNull() {
  const value = "" + Math.random();
  localStorage.setItem("test_key", value);
  localStorage.removeItem("test_key");
  assertEquals(localStorage.getItem("test_key"), null);
  assertEquals(localStorage["test_key"], null);
});

unitTest(function getItemAfterClearShouldReturnNull() {
  const value = "" + Math.random();
  localStorage.setItem("test_key", value);
  localStorage.clear();
  assertEquals(localStorage.getItem("test_key"), null);
  assertEquals(localStorage["test_key"], null);
});

unitTest(function removeItemFromUnknownKeyShouldDoNothing() {
  assertEquals(localStorage.removeItem("unknonw_key"), undefined);
  assertEquals(delete localStorage["unknonw_key"], true);
});

// unitTest(function keyMethodShouldReturnSetKey() {
//   const key = "" + Math.random();

//   localStorage.clear();
//   localStorage.setItem(key, "value");
//   assertEquals(localStorage.key(0), key);
// });

unitTest(function lengthShouldGrowWhenAddingItem() {
  const key = "" + Math.random();

  const expectedLength = localStorage.length + 1;
  localStorage[key] = "value";
  assertEquals(localStorage.length, expectedLength);
});

unitTest(function lengthShouldNotGrowWhenUpdatingExistingItem() {
  const key = "" + Math.random();

  const expectedLength = localStorage.length + 1;
  localStorage.setItem(key, "value");
  assertEquals(localStorage.length, expectedLength);
  localStorage[key] = "other value";
  assertEquals(localStorage.length, expectedLength);
});

unitTest(function lengthShouldShrinkWhenRemovingAnItem() {
  const key = "" + Math.random();

  const expectedLength = localStorage.length;
  localStorage[key] = "value";
  assertEquals(localStorage.length, expectedLength + 1);
  delete localStorage[key];
  assertEquals(localStorage.length, expectedLength);
});

unitTest(function lengthShouldBeZeroWhenStorageIsCleared() {
  localStorage.clear();
  assertEquals(localStorage.length, 0);
});

// unitTest(function accessingLocalStorageShouldThrow() {
//   assertThrows(() => localStorage.length);
// });
