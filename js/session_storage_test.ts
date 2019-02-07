import { test, assertEqual } from "./test_util.ts";

test(function storeItems() {
  assertEqual(sessionStorage.getItem("test"), undefined);
  sessionStorage.setItem("test", "John Doe");
  assertEqual(sessionStorage.getItem("test"), "John Doe");
  sessionStorage.clear(); // Clear for next tests
});

test(function removeItems() {
  assertEqual(sessionStorage.getItem("test"), undefined);
  sessionStorage.setItem("test", "John Doe");
  assertEqual(sessionStorage.getItem("test"), "John Doe");
  sessionStorage.removeItem("test");
  assertEqual(sessionStorage.getItem("test"), undefined);
  sessionStorage.clear(); // Clear for next tests
});

test(function clear() {
  sessionStorage.setItem("test", "John Doe");
  assertEqual(sessionStorage.getItem("test"), "John Doe");
  assertEqual(sessionStorage.length, 1);
  sessionStorage.clear();
  assertEqual(sessionStorage.getItem("test"), undefined);
  assertEqual(sessionStorage.length, 0);
});

test(function length() {
  assertEqual(sessionStorage.length, 0);
  sessionStorage.setItem("test", "John Doe");
  assertEqual(sessionStorage.length, 1);
  sessionStorage.setItem("test1", "Jane Doe");
  assertEqual(sessionStorage.length, 2);
  sessionStorage.clear();
  assertEqual(sessionStorage.length, 0);
});

test(function key() {
  sessionStorage.setItem("test", "John Doe");
  sessionStorage.setItem("test1", "Jane Doe");
  assertEqual(sessionStorage.key(0), "test");
  assertEqual(sessionStorage.key(1), "test1");
  sessionStorage.clear();
  assertEqual(sessionStorage.key(0), undefined);
});
