// Tests that Ctrl+C (0x03) is correctly delivered as data in raw mode.
// Without proper INPUT_RECORD processing via ReadConsoleInputW, the
// event loop would block on non-character events and Ctrl+C data
// would never arrive.
import process from "node:process";

process.stdin.setRawMode(true);
process.stdin.resume();

process.stdin.on("data", (data: Buffer) => {
  if (data[0] === 0x03) {
    console.log("GOT_CTRL_C");
    process.stdin.setRawMode(false);
    process.stdin.pause();
    process.exit(0);
  }
});

console.log("READY");
