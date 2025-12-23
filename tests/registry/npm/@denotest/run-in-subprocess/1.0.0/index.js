import process from "node:process";
import { spawn } from "node:child_process";

export function spawnInSubprocess(args) {
  spawn(process.execPath, args, {
    stdio: "inherit",
  }).on("close", (code) => {
    process.exit(code);
  });
}

