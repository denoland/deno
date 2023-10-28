exports.globalThis = globalThis;
exports.global = global;
exports.process = process;

exports.deleteSetTimeout = function () {
  delete globalThis.setTimeout;
};

exports.getSetTimeout = function () {
  return globalThis.setTimeout;
};

exports.checkProcessGlobal = function () {
  console.log("process" in globalThis);
  console.log(Object.getOwnPropertyDescriptor(globalThis, "process") !== undefined);
};

exports.checkWindowGlobal = function () {
  console.log("window" in globalThis);
  console.log(Object.getOwnPropertyDescriptor(globalThis, "window") !== undefined);
}

exports.getFoo = function () {
  return globalThis.foo;
}