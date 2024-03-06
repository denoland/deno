console.log("Starting the main module");

setTimeout(() => {
  console.log("Dynamic importing");
  import("./import1.ts").then(() => console.log("Dynamic import done."));
}, 0);
