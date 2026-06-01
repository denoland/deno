import process from "node:process";

const originalStdoutWrite = process.stdout.write;
const originalStderrWrite = process.stderr.write;
let stdout = "";
let stderr = "";

process.stdout.write = function (chunk, encoding, callback) {
  stdout += String(chunk);
  if (typeof encoding === "function") {
    encoding();
  } else if (typeof callback === "function") {
    callback();
  }
  return true;
};

process.stderr.write = function (chunk, encoding, callback) {
  stderr += String(chunk);
  if (typeof encoding === "function") {
    encoding();
  } else if (typeof callback === "function") {
    callback();
  }
  return true;
};

console.log("stdout through process");
console.error("stderr through process");

process.stdout.write = originalStdoutWrite;
process.stderr.write = originalStderrWrite;

if (stdout !== "stdout through process\n") {
  throw new Error(`unexpected stdout capture: ${JSON.stringify(stdout)}`);
}

if (stderr !== "stderr through process\n") {
  throw new Error(`unexpected stderr capture: ${JSON.stringify(stderr)}`);
}

console.log("ok");
