import process from "node:process";

// Test that process.on("SIGWINCH") works
process.on("SIGWINCH", () => {
  console.log("resize event received");
  process.exit(0);
});

// Send SIGWINCH to ourselves to trigger the event
process.kill(process.pid, "SIGWINCH");

// Timeout fallback in case the event doesn't fire
setTimeout(() => {
  console.log("timeout: resize event not received");
  process.exit(1);
}, 5000);
