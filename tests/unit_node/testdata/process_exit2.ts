import process from "node:process";

process.on("exit", () => console.log("exit"));
process.exit();
