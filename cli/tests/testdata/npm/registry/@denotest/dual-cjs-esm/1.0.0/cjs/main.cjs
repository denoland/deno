const root = require("../");
const subPath = require("../subpath");

module.exports.getKind = function() {
  return root.getKind();
};

module.exports.getSubPathKind = function() {
  return subPath.getSubPathKind();
};
