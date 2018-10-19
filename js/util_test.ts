// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import { CreateIterableIterator } from "./util";

test(function CreateIterableIteratorSuccess() {
  const list = [1, 2, 3, 4, 5];
  const listIterators = new CreateIterableIterator(list);
   /* tslint:disable-next-line:max-line-length */
  assertEqual(Object.prototype.toString.call(listIterators), "[object Iterator]");
  let idx = 0;
  for (const it of listIterators) {
    assertEqual(it, list[idx++]);
  }
  const obj = {
    a: "foo",
    b: "bar",
    c: "baz"
  };
  const list1 = [];
  const keys = Object.keys(obj);
  keys.forEach(key => list1.push([key, obj[key]]));
  const objectIterators = new CreateIterableIterator(list1);
  /* tslint:disable-next-line:max-line-length */
  assertEqual(Object.prototype.toString.call(objectIterators), "[object Iterator]");
  for (const it of objectIterators) {
    const [key, value] = it;
    assert(key in obj);
    assertEqual(value, obj[key]);
  }
});