import { spawnSync } from "node:child_process";
import fs from "node:fs";

// Open extra file descriptors
const fdA = fs.openSync("/dev/null", "r");
const fdB = fs.openSync("/dev/null", "r");

// Build a sparse stdio array that inherits these FDs by index
const maxFd = Math.max(fdA, fdB);
const stdio: (string | null)[] = new Array(maxFd + 1).fill(null);
stdio[0] = "pipe"; // stdin
stdio[1] = "pipe"; // stdout
stdio[2] = "pipe"; // stderr
stdio[fdA] = "inherit";
stdio[fdB] = "inherit";

const proc = spawnSync("ls", ["/dev/fd"], { stdio });
const stdout = proc.stdout?.toString().trim() ?? "";
const fds = stdout.split("\n").map(Number).filter((n) => !isNaN(n));

// The child should see the inherited FDs
if (!fds.includes(fdA)) {
  throw new Error(`Expected fd ${fdA} in child, got: ${fds.join(",")}`);
}
if (!fds.includes(fdB)) {
  throw new Error(`Expected fd ${fdB} in child, got: ${fds.join(",")}`);
}

console.log("ok");

fs.closeSync(fdA);
fs.closeSync(fdB);
