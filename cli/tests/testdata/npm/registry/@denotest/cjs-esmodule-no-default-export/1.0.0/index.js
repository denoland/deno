"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});

exports.myValue = 5;
exports.default = function() {};

// trick the analyzer and remove the default export at runtime
// doing something that probably won't ever be supported
let name = "defaul";
name += "t";
delete exports[name];
