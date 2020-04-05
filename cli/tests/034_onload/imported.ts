import "./nest_imported.ts";

const handler = (e: Event): void => {
  if (e.cancelable) {
    throw new Error("e.cancelable should be false");
  }
  console.log(`got ${e.type} event in event handler (imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("unload", handler);
console.log("log from imported script");
