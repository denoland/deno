import process from "node:process";

process.stderr.write("\0".repeat(32 * 1024), () => {
  process.stdout.write("stderr write callback\n");
});
