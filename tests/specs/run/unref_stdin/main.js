import process from "node:process";

process.stdin.on("data", () => {
  console.log(2);
});
process.stdin.resume();
process.stdin.pause();

console.log(1);
