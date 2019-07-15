window.addEventListener(
  "load",
  (e: Event): void => {
    console.log(`got ${e.type} event in event handler (nest_imported)`);
  }
);
console.log("log from nest_imported script");
