const path = require("path");

function childProcessFork(path) {
  const p = Deno.run({
    cmd: [Deno.execPath(), "run", "--unstable", "-A", path],
    env: {
      "DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE": Deno.core.ops.op_child_process_fork_state(),
    }
  });
  p.status().then(() => {
    console.log("Done.");
  });
}

module.exports = {
  run() {
    childProcessFork(path.join(__dirname, "forked_path.js"));
  }
};
