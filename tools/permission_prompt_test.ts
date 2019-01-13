import { args, listen, env, exit, makeTempDirSync, run } from "deno";

const name = args[1];
const test = {
  needsWrite: () => {
    makeTempDirSync();
  },
  needsEnv: () => {
    env().home;
  },
  needsNet: () => {
    listen("tcp", "127.0.0.1:4540");
  },
  needsRun: async () => {
    const process = run({
      args: [
        "python",
        "-c",
        "import sys; sys.stdout.write('hello'); sys.stdout.flush()"
      ]
    });
    await process.status();
  }
}[name];

if (!test) {
  console.log("Unknown test:", name);
  exit(1);
}

test();
