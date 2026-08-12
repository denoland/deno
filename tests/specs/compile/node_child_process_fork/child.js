import process from "node:process";

process.on("message", (msg) => {
  if (msg.ask === "ping") {
    process.send({ reply: "pong" });
  }
});
