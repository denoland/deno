define("subdir/print_hello", ["require", "exports"], function(
  require,
  exports
) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  function printHello() {
    console.log("Hello");
  }
  exports.printHello = printHello;
});
define("subdir/subdir2/mod2", [
  "require",
  "exports",
  "subdir/print_hello"
], function(require, exports, print_hello_ts_1) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  function returnsFoo() {
    return "Foo";
  }
  exports.returnsFoo = returnsFoo;
  function printHello2() {
    print_hello_ts_1.printHello();
  }
  exports.printHello2 = printHello2;
});
define("subdir/mod1", ["require", "exports", "subdir/subdir2/mod2"], function(
  require,
  exports,
  mod2_ts_1
) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  function returnsHi() {
    return "Hi";
  }
  exports.returnsHi = returnsHi;
  function returnsFoo2() {
    return mod2_ts_1.returnsFoo();
  }
  exports.returnsFoo2 = returnsFoo2;
  function printHello3() {
    mod2_ts_1.printHello2();
  }
  exports.printHello3 = printHello3;
  function throwsError() {
    throw Error("exception from mod1");
  }
  exports.throwsError = throwsError;
});
define("005_more_imports", ["require", "exports", "subdir/mod1"], function(
  require,
  exports,
  mod1_ts_1
) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  mod1_ts_1.printHello3();
  if (mod1_ts_1.returnsHi() !== "Hi") {
    throw Error("Unexpected");
  }
  if (mod1_ts_1.returnsFoo2() !== "Foo") {
    throw Error("Unexpected");
  }
});
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiYnVuZGxlLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZmlsZTovLy9Vc2Vycy9ra2VsbHkvZ2l0aHViL2Rlbm8vdGVzdHMvc3ViZGlyL3ByaW50X2hlbGxvLnRzIiwiZmlsZTovLy9Vc2Vycy9ra2VsbHkvZ2l0aHViL2Rlbm8vdGVzdHMvc3ViZGlyL3N1YmRpcjIvbW9kMi50cyIsImZpbGU6Ly8vVXNlcnMva2tlbGx5L2dpdGh1Yi9kZW5vL3Rlc3RzL3N1YmRpci9tb2QxLnRzIiwiZmlsZTovLy9Vc2Vycy9ra2VsbHkvZ2l0aHViL2Rlbm8vdGVzdHMvMDA1X21vcmVfaW1wb3J0cy50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiOzs7SUFBQSxTQUFnQixVQUFVO1FBQ3hCLE9BQU8sQ0FBQyxHQUFHLENBQUMsT0FBTyxDQUFDLENBQUM7SUFDdkIsQ0FBQztJQUZELGdDQUVDOzs7OztJQ0FELFNBQWdCLFVBQVU7UUFDeEIsT0FBTyxLQUFLLENBQUM7SUFDZixDQUFDO0lBRkQsZ0NBRUM7SUFFRCxTQUFnQixXQUFXO1FBQ3pCLDJCQUFVLEVBQUUsQ0FBQztJQUNmLENBQUM7SUFGRCxrQ0FFQzs7Ozs7SUNORCxTQUFnQixTQUFTO1FBQ3ZCLE9BQU8sSUFBSSxDQUFDO0lBQ2QsQ0FBQztJQUZELDhCQUVDO0lBRUQsU0FBZ0IsV0FBVztRQUN6QixPQUFPLG9CQUFVLEVBQUUsQ0FBQztJQUN0QixDQUFDO0lBRkQsa0NBRUM7SUFFRCxTQUFnQixXQUFXO1FBQ3pCLHFCQUFXLEVBQUUsQ0FBQztJQUNoQixDQUFDO0lBRkQsa0NBRUM7SUFFRCxTQUFnQixXQUFXO1FBQ3pCLE1BQU0sS0FBSyxDQUFDLHFCQUFxQixDQUFDLENBQUM7SUFDckMsQ0FBQztJQUZELGtDQUVDOzs7OztJQ2RELHFCQUFXLEVBQUUsQ0FBQztJQUVkLElBQUksbUJBQVMsRUFBRSxLQUFLLElBQUksRUFBRTtRQUN4QixNQUFNLEtBQUssQ0FBQyxZQUFZLENBQUMsQ0FBQztLQUMzQjtJQUVELElBQUkscUJBQVcsRUFBRSxLQUFLLEtBQUssRUFBRTtRQUMzQixNQUFNLEtBQUssQ0FBQyxZQUFZLENBQUMsQ0FBQztLQUMzQiIsInNvdXJjZXNDb250ZW50IjpbImV4cG9ydCBmdW5jdGlvbiBwcmludEhlbGxvKCk6IHZvaWQge1xuICBjb25zb2xlLmxvZyhcIkhlbGxvXCIpO1xufVxuIiwiaW1wb3J0IHsgcHJpbnRIZWxsbyB9IGZyb20gXCIuLi9wcmludF9oZWxsby50c1wiO1xuXG5leHBvcnQgZnVuY3Rpb24gcmV0dXJuc0ZvbygpOiBzdHJpbmcge1xuICByZXR1cm4gXCJGb29cIjtcbn1cblxuZXhwb3J0IGZ1bmN0aW9uIHByaW50SGVsbG8yKCk6IHZvaWQge1xuICBwcmludEhlbGxvKCk7XG59XG4iLCJpbXBvcnQgeyByZXR1cm5zRm9vLCBwcmludEhlbGxvMiB9IGZyb20gXCIuL3N1YmRpcjIvbW9kMi50c1wiO1xuXG5leHBvcnQgZnVuY3Rpb24gcmV0dXJuc0hpKCk6IHN0cmluZyB7XG4gIHJldHVybiBcIkhpXCI7XG59XG5cbmV4cG9ydCBmdW5jdGlvbiByZXR1cm5zRm9vMigpOiBzdHJpbmcge1xuICByZXR1cm4gcmV0dXJuc0ZvbygpO1xufVxuXG5leHBvcnQgZnVuY3Rpb24gcHJpbnRIZWxsbzMoKTogdm9pZCB7XG4gIHByaW50SGVsbG8yKCk7XG59XG5cbmV4cG9ydCBmdW5jdGlvbiB0aHJvd3NFcnJvcigpOiB2b2lkIHtcbiAgdGhyb3cgRXJyb3IoXCJleGNlcHRpb24gZnJvbSBtb2QxXCIpO1xufVxuIiwiaW1wb3J0IHsgcmV0dXJuc0hpLCByZXR1cm5zRm9vMiwgcHJpbnRIZWxsbzMgfSBmcm9tIFwiLi9zdWJkaXIvbW9kMS50c1wiO1xuXG5wcmludEhlbGxvMygpO1xuXG5pZiAocmV0dXJuc0hpKCkgIT09IFwiSGlcIikge1xuICB0aHJvdyBFcnJvcihcIlVuZXhwZWN0ZWRcIik7XG59XG5cbmlmIChyZXR1cm5zRm9vMigpICE9PSBcIkZvb1wiKSB7XG4gIHRocm93IEVycm9yKFwiVW5leHBlY3RlZFwiKTtcbn1cbiJdfQ==
