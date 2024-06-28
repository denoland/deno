const path = require("path");

function childProcessFork(path) {
  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "-A", path],
    env: {
      "DENO_DONT_USE_INTERNAL_NODE_COMPAT_STATE": Deno[Deno.internal].core.ops.op_npm_process_state(),
    }
  });
  const child = command.spawn();
  child.status.then(() => {
    console.log("Done.");
  });
}

module.exports = {
  run() {
    childProcessFork(path.join(__dirname, "forked_path.js"));
  }
};
