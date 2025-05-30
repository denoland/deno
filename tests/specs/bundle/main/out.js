var __getOwnPropNames = Object.getOwnPropertyNames;
import { createRequire } from "node:module";
var __require = createRequire(import.meta.url);

var __commonJS = (cb, mod) => function __require2() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

// uses_node_builtin.cjs
var require_uses_node_builtin = __commonJS({
  "uses_node_builtin.cjs"() {
    var { inspect } = __require("util");
    console.log(inspect({
      a: 1,
      b: "hello"
    }));
  }
});
export default require_uses_node_builtin();
