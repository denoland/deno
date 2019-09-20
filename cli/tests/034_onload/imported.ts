import "./nest_imported.ts";
window.addEventListener(
  "load",
  (e: Event): void => {
    console.log(`got ${e.type} event in event handler (imported)`);
  }
);
console.log("log from imported script");
