import { spawn } from "node:child_process";
import process from "node:process";

if (process.argv[2] === "child") {
  process.send("hahah");
} else {
  const proc = spawn(process.execPath, ["./main.mjs", "child"], {
    stdio: ["ipc", "inherit", "inherit"],
  });

  proc.on("message", function (msg) {
    console.log(`msg: ${msg}`);
    proc.kill();
    Deno.exit(0);
  });
}
