window.onunhandledrejection = (e) => {
  console.log("unhandled rejection", e.reason, e.promise);

  // e.preventDefault();
};

Deno.readTextFile("foo.json");
