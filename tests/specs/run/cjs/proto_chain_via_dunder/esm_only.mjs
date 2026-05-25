// Pure ESM programs that never touch CJS keep the hardened default:
// `Object.prototype.__proto__` stays deleted.
console.log(Object.hasOwn(Object.prototype, "__proto__"));
