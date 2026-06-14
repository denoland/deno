// `replace()` returns a promise, so per WebIDL every failure must reject the
// promise rather than throw synchronously, including the "not enough arguments"
// TypeError for a zero-argument call.
const sheet = new CSSStyleSheet();

// A valid call resolves with the sheet and applies the text.
const resolved = await sheet.replace("body { color: red; }");
console.log(resolved === sheet);
console.log(sheet.cssRules[0].cssText);

// An invalid argument rejects (DOMString conversion of a Symbol fails).
try {
  await sheet.replace(Symbol("nope") as unknown as string);
  console.log("invalid arg: did not throw");
} catch (e) {
  console.log("invalid arg rejected:", e instanceof TypeError);
}

// A missing argument must also reject, not throw synchronously. If it threw
// synchronously the call expression itself would throw before returning a
// promise, so awaiting it would never reach the catch with a rejection.
let threwSynchronously = false;
let promise: Promise<unknown> | undefined;
try {
  // @ts-expect-error intentionally calling without the required argument
  promise = sheet.replace();
} catch {
  threwSynchronously = true;
}
console.log("missing arg threw synchronously:", threwSynchronously);
try {
  await promise;
  console.log("missing arg: did not reject");
} catch (e) {
  console.log("missing arg rejected:", e instanceof TypeError);
  console.log((e as TypeError).message);
}
