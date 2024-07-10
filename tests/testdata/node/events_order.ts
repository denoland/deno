import process from "node:process";

let count = 0;
process.on("beforeExit", () => {
  if (count === 0 || count === 1) {
    setTimeout(() => console.log("more work done!", count), 10);
  }
  count++;
  console.log("beforeExit emitted from process.on");
});
process.on("exit", () => console.log("exit emitted from process.on"));

let countWeb = 0;
addEventListener("beforeunload", (event) => {
  if (countWeb == 0 || countWeb == 1) {
    event.preventDefault();
  }
  countWeb++;
  console.log("beforeunload emitted from addEventListener");
});

addEventListener(
  "unload",
  () => console.log("unload emitted from addEventListener"),
);
