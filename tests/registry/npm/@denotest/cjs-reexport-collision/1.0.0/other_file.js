"use strict";
class Hello {
  sayHello() {
    console.log("Hi.");
  }
}
// conflict will be with __esModule
Object.defineProperty(exports, "__esModule", { value: true });
exports.hello = new Hello();
exports.default = new Hello();
