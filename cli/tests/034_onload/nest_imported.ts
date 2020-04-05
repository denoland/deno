const handler = (e: Event): void => {
  if (e.cancelable) throw new Error("e.cancelable must should be false");
  console.log(`got ${e.type} event in event handler (nest_imported)`);
};

window.addEventListener("load", handler);
window.addEventListener("unload", handler);
console.log("log from nest_imported script");
