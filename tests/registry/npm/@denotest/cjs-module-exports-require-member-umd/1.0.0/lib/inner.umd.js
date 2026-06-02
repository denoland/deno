(function (global, factory) {
  typeof exports === 'object' && typeof module !== 'undefined' ? factory(exports) :
  typeof define === 'function' && define.amd ? define(['exports'], factory) :
  (global = typeof globalThis !== 'undefined' ? globalThis : global || self, factory(global.gqlmod = {}));
}(this, (function (exports) {
  'use strict';
  function gql() { return "gql-result"; }
  function resetCaches() { return "reset"; }
  function disableFragmentWarnings() { return "disabled"; }
  var extras = {
    gql: gql,
    resetCaches: resetCaches,
    disableFragmentWarnings: disableFragmentWarnings,
  };
  (function (gql_1) {
    gql_1.gql = extras.gql;
    gql_1.resetCaches = extras.resetCaches;
    gql_1.disableFragmentWarnings = extras.disableFragmentWarnings;
  })(gql || (gql = {}));
  gql["default"] = gql;
  exports.default = gql;
  exports.gql = gql;
  exports.resetCaches = resetCaches;
  exports.disableFragmentWarnings = disableFragmentWarnings;
  Object.defineProperty(exports, '__esModule', { value: true });
})));
