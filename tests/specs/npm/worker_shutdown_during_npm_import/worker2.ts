import specifiers from "./specifiers.ts";
// start importing, but close after waiting a short amount of time
specifiers.map((specifier) => import(specifier));
await new Promise((resolve) => setTimeout(() => resolve(), 20));
console.log(2);
self.close();
console.log("WILL NOT BE PRINTED");
