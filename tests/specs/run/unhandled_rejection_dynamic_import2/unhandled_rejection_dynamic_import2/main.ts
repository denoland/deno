globalThis.addEventListener("unhandledrejection", (e) => {
  console.log("unhandled rejection", e.reason);
  e.preventDefault();
});

const dummyImport = (await import("./import.ts")).default;

let a = new Promise((resolve, reject) => {
  throw "errA";
});

let i = 0;
while (true) {
  await new Promise((resolve) => setTimeout(resolve, 100));
  i++;
  console.log("running...");

  if (i > 3) {
    break;
  }
}
