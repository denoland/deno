import process from "node:process";

// In the test runner, stdout is piped (not a TTY).
// Override isTTY to simulate a terminal so the resize event fires.
process.stdout.isTTY = true;

// Listen for the resize event on stdout
process.stdout.on("resize", () => {
  console.log("resize event received");
  process.exit(0);
});

// Send SIGWINCH to ourselves to trigger the resize event
process.kill(process.pid, "SIGWINCH");

// Timeout fallback in case the event doesn't fire
setTimeout(() => {
  console.log("timeout: resize event not received");
  process.exit(1);
}, 5000);
