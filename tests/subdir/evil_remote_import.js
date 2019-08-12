// We expect to get a permission denied error if we dynamically
// import this module without --allow-read.
export * from "file:///c:/etc/passwd";
console.log("Hello from evil_remote_import.js");
