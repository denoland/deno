import specifiers from "./specifiers.ts";
await new Promise((resolve) => setTimeout(() => resolve(), 20));

await Promise.all(specifiers.map((specifier) => import(specifier)));

console.log(1);
self.close();
