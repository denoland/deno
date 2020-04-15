// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register("$deno$/web/dom_util.ts", [], function (
  exports_105,
  context_105
) {
  "use strict";
  const __moduleName = context_105 && context_105.id;
  function getDOMStringList(arr) {
    Object.defineProperties(arr, {
      contains: {
        value(searchElement) {
          return arr.includes(searchElement);
        },
        enumerable: true,
      },
      item: {
        value(idx) {
          return idx in arr ? arr[idx] : null;
        },
      },
    });
    return arr;
  }
  exports_105("getDOMStringList", getDOMStringList);
  return {
    setters: [],
    execute: function () {},
  };
});
