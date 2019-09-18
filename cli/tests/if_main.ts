if (window.location.toString() == import.meta.url) {
  console.log("main");
} else {
  console.log("import.meta.url", import.meta.url);
  console.log("window.location", window.location.toString());
  throw Error("not main");
}
