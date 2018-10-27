// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function formDataParamsAppendSuccess() {
  const formData = new FormData();
  formData.append("a", "true");
  assertEqual(formData.get("a"), "true");
});

test(function formDataParamsDeleteSuccess() {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assertEqual(formData.get("b"), "false");
  formData.delete("b");
  assertEqual(formData.get("a"), "true");
  assertEqual(formData.get("b"), null);
});

test(function formDataParamsGetAllSuccess() {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assertEqual(formData.getAll("a"), ["true", "null"]);
  assertEqual(formData.getAll("b"), ["false"]);
  assertEqual(formData.getAll("c"), []);
});

test(function formDataParamsGetSuccess() {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  formData.append("d", undefined);
  formData.append("e", null);
  assertEqual(formData.get("a"), "true");
  assertEqual(formData.get("b"), "false");
  assertEqual(formData.get("c"), null);
  assertEqual(formData.get("d"), "undefined");
  assertEqual(formData.get("e"), "null");
});

test(function formDataParamsHasSuccess() {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assert(formData.has("a"));
  assert(formData.has("b"));
  assert(!formData.has("c"));
});

test(function formDataParamsSetSuccess() {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assertEqual(formData.getAll("a"), ["true", "null"]);
  assertEqual(formData.getAll("b"), ["false"]);
  formData.set("a", "false");
  assertEqual(formData.getAll("a"), ["false"]);
  formData.set("d", undefined);
  assertEqual(formData.get("d"), "undefined");
  formData.set("e", null);
  assertEqual(formData.get("e"), "null");
});

test(function formDataSetEmptyBlobSuccess() {
  const formData = new FormData();
  formData.set("a", new Blob([]), "blank.txt");
  const file = formData.get("a");
  assert(file instanceof File);
  if (typeof file !== "string") {
    assertEqual(file.name, "blank.txt");
  }
});

test(function formDataParamsForEachSuccess() {
  const init = [["a", "54"], ["b", "true"]];
  const formData = new FormData();
  for (const [name, value] of init) {
    formData.append(name, value);
  }
  let callNum = 0;
  formData.forEach((value, key, parent) => {
    assertEqual(formData, parent);
    assertEqual(value, init[callNum][1]);
    assertEqual(key, init[callNum][0]);
    callNum++;
  });
  assertEqual(callNum, init.length);
});
