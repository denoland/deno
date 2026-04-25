// Regression test for https://github.com/denoland/deno/issues/32921
// `propName in obj` evaluated inside a `vm.runInContext` script must walk
// the prototype chain of the contextified sandbox object, matching Node's
// behaviour. Previously the property_query interceptor used `HasRealNamed`
// (own-only), so properties reachable via `Object.setPrototypeOf` were
// invisible to the `in` operator inside the vm context.

"use strict";

const vm = require("vm");

class EventTarget {
  addEventListener() {}
}

const windowConstructor = function () {};
Object.setPrototypeOf(windowConstructor, EventTarget);
const windowPrototype = Object.create(EventTarget.prototype);

function Window() {
  vm.createContext(this);
  this._globalProxy = vm.runInContext("this", this);

  Object.setPrototypeOf(this, windowPrototype);

  const window = this;
  Object.defineProperty(this, "window", {
    get() {
      return window._globalProxy;
    },
    enumerable: true,
    configurable: true,
  });
}

const window = new Window();

console.log(vm.runInContext(`"addEventListener" in window`, window));
// Sanity check: own properties still resolve.
console.log(vm.runInContext(`"window" in window`, window));
// Negative case: properties not on the chain still report false.
console.log(vm.runInContext(`"doesNotExist" in window`, window));
