const path = require("path");

module.exports = {
  entry: "./node_modules/jsdom/lib/api.js",
  output: {
    path: path.join(__dirname, "dist"),
    filename: "./jsdom.js",
    library: "jsdom",
    libraryTarget: "var"
  },
  mode: "production",
  node: {
    child_process: "empty",
    fs: "empty",
    net: "empty",
    tls: "empty"
  },
  performance: {
    // TODO: Investigate whether the bundle size can be optimized.
    hints: false
  }
};
