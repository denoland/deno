import "./imported.ts";

const eventHandler = (e: Event): void => {
  if (e.cancelable) {
    throw new Error("e.cancelable should be false");
  }
  console.log(`got ${e.type} event in event handler (main)`);
};

window.addEventListener("load", eventHandler);

window.addEventListener("unload", eventHandler);

window.onload = (e: Event): void => {
  if (e.cancelable) throw new Error("e.cancelable shoud be false");
  console.log(`got ${e.type} event in onload function`);
};

window.onunload = (e: Event): void => {
  if (e.cancelable) throw new Error("e.cancelable should be false");
  console.log(`got ${e.type} event in onunload function`);
};

console.log("log from main");
