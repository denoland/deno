System.register("$deno$/web/promise.ts", [], function (exports_34, context_34) {
  "use strict";
  let PromiseState;
  const __moduleName = context_34 && context_34.id;
  return {
    setters: [],
    execute: function () {
      (function (PromiseState) {
        PromiseState[(PromiseState["Pending"] = 0)] = "Pending";
        PromiseState[(PromiseState["Fulfilled"] = 1)] = "Fulfilled";
        PromiseState[(PromiseState["Rejected"] = 2)] = "Rejected";
      })(PromiseState || (PromiseState = {}));
      exports_34("PromiseState", PromiseState);
    },
  };
});
