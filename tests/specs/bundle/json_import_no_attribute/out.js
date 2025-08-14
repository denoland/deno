var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function __require() {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

// foo.json
var require_foo = __commonJS({
  "foo.json"(exports, module) {
    module.exports = {
      name: "foo"
    };
  }
});

// main.cjs
var require_main = __commonJS({
  "main.cjs"() {
    var json = require_foo();
    console.log(json);
  }
});
export default require_main();
