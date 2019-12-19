// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { testPerm, assertEquals } from "./test_util.ts";

testPerm({}, async function storage(): Promise<void> {
  assertEquals(sessionStorage instanceof Storage, true);
  assertEquals(sessionStorage.__proto__ === Storage.prototype, true);

  assertEquals(localStorage instanceof Storage, true);
  assertEquals(localStorage.__proto__ === Storage.prototype, true);
});

testPerm({}, async function sessionStorageBasic(): Promise<void> {
  assertEquals(sessionStorage.length, 0);
  assertEquals(sessionStorage.key(0), null);
  assertEquals(sessionStorage.key(1), null);
  assertEquals(sessionStorage.setItem("foo", "bar"), undefined);
  assertEquals(sessionStorage.length, 1);
  assertEquals(sessionStorage.key(0), "foo");
  assertEquals(sessionStorage.getItem("foo"), "bar");
  assertEquals(sessionStorage.removeItem("foo"), undefined);
  assertEquals(sessionStorage.length, 0);
  assertEquals(sessionStorage.key(0), null);
  assertEquals(sessionStorage.getItem("not_exist_key"), null);
  assertEquals(sessionStorage.setItem("bar", "foo"), undefined);
  assertEquals(sessionStorage.length, 1);
  assertEquals(sessionStorage.key(0), "bar");
  assertEquals(sessionStorage.clear(), undefined);
  assertEquals(sessionStorage.length, 0);
  assertEquals(sessionStorage.key(0), null);
});

testPerm({}, async function localStorageBasic(): Promise<void> {
  assertEquals(localStorage.length, 0);
  assertEquals(localStorage.key(0), null);
  assertEquals(localStorage.key(1), null);
  assertEquals(localStorage.setItem("foo", "bar"), undefined);
  assertEquals(localStorage.length, 1);
  assertEquals(localStorage.key(0), "foo");
  assertEquals(localStorage.getItem("foo"), "bar");
  assertEquals(localStorage.removeItem("foo"), undefined);
  assertEquals(localStorage.length, 0);
  assertEquals(localStorage.key(0), null);
  assertEquals(localStorage.getItem("not_exist_key"), null);
  assertEquals(localStorage.setItem("bar", "foo"), undefined);
  assertEquals(localStorage.length, 1);
  assertEquals(localStorage.key(0), "bar");
  assertEquals(localStorage.clear(), undefined);
  assertEquals(localStorage.length, 0);
  assertEquals(localStorage.key(0), null);
});

testPerm({}, async function localStoragePersistence(): Promise<void> {
  assertEquals(localStorage.length, 0);
  assertEquals(localStorage.key(0), null);
  assertEquals(localStorage.key(1), null);
  assertEquals(localStorage.setItem("foo", "bar"), undefined);
  assertEquals(localStorage.length, 1);
  assertEquals(localStorage.key(0), "foo");

  // TODO: start a child process and test if the data is persistent
});
