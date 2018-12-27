// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";

test(function formDataHasCorrectNameProp() {
  assertEqual(FormData.name, "FormData");
});

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
  /* TODO Fix this test.
  assert(file instanceof File);
  if (typeof file !== "string") {
    assertEqual(file.name, "blank.txt");
  }
  */
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

test(function formDataParamsArgumentsCheck() {
  const methodRequireOneParam = ["delete", "getAll", "get", "has", "forEach"];

  const methodRequireTwoParams = ["append", "set"];

  methodRequireOneParam.forEach(method => {
    const formData = new FormData();
    let hasThrown = 0;
    let errMsg = "";
    try {
      formData[method]();
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEqual(hasThrown, 2);
    assertEqual(
      errMsg,
      `FormData.${method} requires at least 1 argument, but only 0 present`
    );
  });

  methodRequireTwoParams.forEach(method => {
    const formData = new FormData();
    let hasThrown = 0;
    let errMsg = "";

    try {
      formData[method]();
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEqual(hasThrown, 2);
    assertEqual(
      errMsg,
      `FormData.${method} requires at least 2 arguments, but only 0 present`
    );

    hasThrown = 0;
    errMsg = "";
    try {
      formData[method]("foo");
      hasThrown = 1;
    } catch (err) {
      errMsg = err.message;
      if (err instanceof TypeError) {
        hasThrown = 2;
      } else {
        hasThrown = 3;
      }
    }
    assertEqual(hasThrown, 2);
    assertEqual(
      errMsg,
      `FormData.${method} requires at least 2 arguments, but only 1 present`
    );
  });
});
