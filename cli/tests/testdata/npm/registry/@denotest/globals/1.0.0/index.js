exports.globalThis = globalThis;
exports.global = global;
exports.process = process;

exports.withNodeGlobalThis = function (action) {
  action(globalThis);
};
