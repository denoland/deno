const path = require("path");
const childProcess = require("node:child_process");

function childProcessFork(path) {
  const child = childProcess.fork(path);
  child.on("exit", () => {
    console.log("Done.");
  });
}

module.exports = {
  run() {
    childProcessFork(path.join(__dirname, "forked_path.js"));
  }
};
