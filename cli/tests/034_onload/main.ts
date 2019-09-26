import "./imported.ts";

window.addEventListener(
  "load",
  (e: Event): void => {
    console.log(`got ${e.type} event in event handler (main)`);
  }
);

window.onload = (e: Event): void => {
  console.log(`got ${e.type} event in onload function`);
};

console.log("log from main");
