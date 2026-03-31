// Tests that arrow keys and other special keys are correctly mapped to
// VT100 escape sequences in raw mode. This exercises the
// get_vt100_fn_key mapping and ReadConsoleInputW-based raw read path.
import process from "node:process";

process.stdin.setRawMode(true);
process.stdin.resume();
process.stdin.setEncoding("utf8");

let received = 0;
const expected = 4;

process.stdin.on("data", (data: string) => {
  const hex = Buffer.from(data).toString("hex");
  if (data === "\x1b[A") {
    console.log("UP");
    received++;
  } else if (data === "\x1b[B") {
    console.log("DOWN");
    received++;
  } else if (data === "\x1b[C") {
    console.log("RIGHT");
    received++;
  } else if (data === "\x1b[D") {
    console.log("LEFT");
    received++;
  } else {
    console.log("OTHER:" + hex);
    received++;
  }

  if (received >= expected) {
    process.stdin.setRawMode(false);
    process.stdin.pause();
  }
});
