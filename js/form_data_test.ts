// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEquals } from "./test_util.ts";

test(function formDataHasCorrectNameProp(): void {
  assertEquals(FormData.name, "FormData");
});

test(function formDataParamsAppendSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  assertEquals(formData.get("a"), "true");
});

test(function formDataParamsDeleteSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assertEquals(formData.get("b"), "false");
  formData.delete("b");
  assertEquals(formData.get("a"), "true");
  assertEquals(formData.get("b"), null);
});

test(function formDataParamsGetAllSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assertEquals(formData.getAll("a"), ["true", "null"]);
  assertEquals(formData.getAll("b"), ["false"]);
  assertEquals(formData.getAll("c"), []);
});

test(function formDataParamsGetSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  formData.append("d", undefined);
  formData.append("e", null);
  assertEquals(formData.get("a"), "true");
  assertEquals(formData.get("b"), "false");
  assertEquals(formData.get("c"), null);
  assertEquals(formData.get("d"), "undefined");
  assertEquals(formData.get("e"), "null");
});

test(function formDataParamsHasSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  assert(formData.has("a"));
  assert(formData.has("b"));
  assert(!formData.has("c"));
});

test(function formDataParamsSetSuccess(): void {
  const formData = new FormData();
  formData.append("a", "true");
  formData.append("b", "false");
  formData.append("a", "null");
  assertEquals(formData.getAll("a"), ["true", "null"]);
  assertEquals(formData.getAll("b"), ["false"]);
  formData.set("a", "false");
  assertEquals(formData.getAll("a"), ["false"]);
  formData.set("d", undefined);
  assertEquals(formData.get("d"), "undefined");
  formData.set("e", null);
  assertEquals(formData.get("e"), "null");
});

test(function formDataSetEmptyBlobSuccess(): void {
  const formData = new FormData();
  formData.set("a", new Blob([]), "blank.txt");
  formData.get("a");
  /* TODO Fix this test.
  assert(file instanceof File);
  if (typeof file !== "string") {
    assertEquals(file.name, "blank.txt");
  }
  */
});

test(function formDataParamsForEachSuccess(): void {
  const init = [["a", "54"], ["b", "true"]];
  const formData = new FormData();
  for (const [name, value] of init) {
    formData.append(name, value);
  }
  let callNum = 0;
  formData.forEach(
    (value, key, parent): void => {
      assertEquals(formData, parent);
      assertEquals(value, init[callNum][1]);
      assertEquals(key, init[callNum][0]);
      callNum++;
    }
  );
  assertEquals(callNum, init.length);
});

test(function formDataParamsArgumentsCheck(): void {
  const methodRequireOneParam = ["delete", "getAll", "get", "has", "forEach"];

  const methodRequireTwoParams = ["append", "set"];

  methodRequireOneParam.forEach(
    (method): void => {
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
      assertEquals(hasThrown, 2);
      assertEquals(
        errMsg,
        `FormData.${method} requires at least 1 argument, but only 0 present`
      );
    }
  );

  methodRequireTwoParams.forEach(
    (method: string): void => {
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
      assertEquals(hasThrown, 2);
      assertEquals(
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
      assertEquals(hasThrown, 2);
      assertEquals(
        errMsg,
        `FormData.${method} requires at least 2 arguments, but only 1 present`
      );
    }
  );
});

test(function toStringShouldBeWebCompatibility(): void {
  const formData = new FormData();
  assertEquals(formData.toString(), "[object FormData]");
});
