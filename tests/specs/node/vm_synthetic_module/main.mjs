// Mirrors test/parallel/test-vm-module-evaluate-synthethic-module.js from
// upstream Node.js (https://github.com/nodejs/node) — verifies the
// vm.SyntheticModule polyfill.
import assert from "node:assert";
import { inspect } from "node:util";
import vm from "node:vm";

// Synthetic modules with a synchronous evaluation step evaluate to a promise
// synchronously resolved to undefined.
{
  let called = 0;
  const mod = new vm.SyntheticModule(["a"], () => {
    called++;
    mod.setExport("a", 42);
  });
  const promise = mod.evaluate();
  assert.match(inspect(promise), /Promise { undefined }/);
  assert.strictEqual(mod.namespace.a, 42);

  await promise.then((value) => {
    assert.strictEqual(value, undefined);
    const promise2 = mod.evaluate();
    assert.match(inspect(promise2), /Promise { undefined }/);
    return promise2.then((v2) => assert.strictEqual(v2, undefined));
  });

  assert.strictEqual(called, 1, "evaluation steps should run exactly once");
  console.log("ok sync");
}

// Synthetic modules with an asynchronous evaluation step still evaluate to a
// promise synchronously resolved to undefined; the namespace export remains
// unset until the async work completes (Node discards the callback's return
// value, the user has to call setExport explicitly).
{
  const mod = new vm.SyntheticModule(["a"], async () => {
    const result = await Promise.resolve(42);
    mod.setExport("a", result);
    return result;
  });
  const promise = mod.evaluate();
  assert.match(inspect(promise), /Promise { undefined }/);
  assert.strictEqual(mod.namespace.a, undefined);

  await promise.then((value) => {
    assert.strictEqual(value, undefined);
    const promise2 = mod.evaluate();
    assert.match(inspect(promise2), /Promise { undefined }/);
    return promise2.then((v2) => assert.strictEqual(v2, undefined));
  });

  console.log("ok async");
}

// Synchronous error during the evaluation step → synchronously rejected
// promise, and `mod.error` matches.
{
  const mod = new vm.SyntheticModule(["a"], () => {
    throw new Error("synchronous synthethic module");
  });
  const promise = mod.evaluate();
  assert.match(inspect(promise), /rejected/);
  assert.ok(mod.error, "Expected mod.error to be set");
  assert.strictEqual(mod.error.message, "synchronous synthethic module");

  await promise.catch((err) => {
    assert.strictEqual(err, mod.error);
    const promise2 = mod.evaluate();
    assert.match(inspect(promise2), /rejected/);
    return promise2.catch((err2) => assert.strictEqual(err, err2));
  });

  console.log("ok throw");
}

// Constructor validation: bad exportNames, duplicate names, non-function
// callback.
{
  assert.throws(
    () => new vm.SyntheticModule("a", () => {}),
    { code: "ERR_INVALID_ARG_TYPE" },
  );
  assert.throws(
    () => new vm.SyntheticModule([1], () => {}),
    { code: "ERR_INVALID_ARG_TYPE" },
  );
  assert.throws(
    () => new vm.SyntheticModule(["a", "a"], () => {}),
    { code: "ERR_INVALID_ARG_VALUE" },
  );
  assert.throws(
    () => new vm.SyntheticModule(["a"], "not a function"),
    { code: "ERR_INVALID_ARG_TYPE" },
  );

  // identifier option propagates to module.identifier.
  const mod = new vm.SyntheticModule(["x"], () => mod.setExport("x", 1), {
    identifier: "my:synthetic",
  });
  assert.strictEqual(mod.identifier, "my:synthetic");
  // `this` inside the callback is the module instance.
  const mod2 = new vm.SyntheticModule(["x"], function () {
    this.setExport("x", "self");
  });
  await mod2.evaluate();
  assert.strictEqual(mod2.namespace.x, "self");

  console.log("ok options");
}
