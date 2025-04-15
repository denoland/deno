Object.defineProperty(exports, '__esModule', { value: true });

const pkg = require("@denotest/conditional-exports-node");

Object.keys(pkg).forEach(function (k) {
  if (k !== 'default' && !Object.prototype.hasOwnProperty.call(exports, k)) exports[k] = pkg[k];
});