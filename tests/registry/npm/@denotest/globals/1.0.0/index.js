exports.globalThis = globalThis;
exports.global = global;
exports.process = process;

exports.deleteSetTimeout = function () {
  delete globalThis.setTimeout;
};

exports.getSetTimeout = function () {
  return globalThis.setTimeout;
};

exports.checkWindowGlobal = function () {
  console.log("window" in globalThis);
  console.log(Object.getOwnPropertyDescriptor(globalThis, "window") !== undefined);
}

exports.checkSelfGlobal = function () {
  console.log("self" in globalThis);
  console.log(Object.getOwnPropertyDescriptor(globalThis, "self") !== undefined);
}

exports.getFoo = function () {
  return globalThis.foo;
}
