class Test {}
const test = new Test();
Object.setPrototypeOf(test, { test: "test" });
console.log("Object.getPrototypeOf: ", Object.getPrototypeOf(test));
console.log("__proto__: ", test.__proto__);
