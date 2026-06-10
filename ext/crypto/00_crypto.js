// Copyright 2018-2026 the Deno authors. MIT license.

// This module is intentionally thin: every WebCrypto algorithm body
// (`SubtleCrypto.{digest,encrypt,decrypt,sign,verify,deriveBits,deriveKey,
// importKey,exportKey,wrapKey,unwrapKey,generateKey,getPublicKey,
// encapsulateKey,encapsulateBits,decapsulateKey,decapsulateBits,supports}`,
// and the `Crypto.{getRandomValues,randomUUID,subtle}` members) is
// implemented natively on the cppgc-wrapped Rust classes in
// `ext/crypto/{crypto,subtle_crypto,crypto_key}.rs` and the per-algorithm
// modules they delegate to. What remains in JS is bookkeeping the v8 layer
// requires us to do outside cppgc: the privateCustomInspect decoration on
// the three prototypes, the lazy minting of the `Crypto` / `SubtleCrypto`
// singletons (cppgc allocations can't run at snapshot-build time), the
// structured-clone resurrection callback, and the small `deriveBits`
// forwarder that gives the spec-mandated `Function.length === 2`.

(function () {
const { core, primordials, internals } = __bootstrap;
const {
  Crypto,
  CryptoKey,
  SubtleCrypto,
  op_create_crypto,
  op_create_subtle_crypto,
} = core.ops;
const {
  FunctionPrototypeCall,
  ObjectAssign,
  ObjectDefineProperty,
  ObjectPrototypeIsPrototypeOf,
  SafeArrayIterator,
  SymbolFor,
} = primordials;

const webidl = core.loadExtScript("ext:deno_webidl/00_webidl.js");
const { createFilteredInspectProxy } = core.loadExtScript(
  "ext:deno_web/01_console.js",
);
const { kKeyObject } = internals;

// op2-generated interface constructors expose the macro's internal
// new-target signal (`_: bool`) as a formal parameter, giving the
// constructor's `.length` a value of 1. Per Web IDL, the interface
// object's `.length` is the minimum-overload required-argument count -- 0
// for all three classes (`Crypto`, `CryptoKey`, `SubtleCrypto` have no
// constructor exposed). Also pin the `prototype` slot to non-writable:
// V8's FunctionTemplate-derived constructors default to a writable
// prototype, but Web IDL requires
// `{ writable: false, enumerable: false, configurable: false }`. The
// `configurable: false` slot is already set, and ECMAScript permits
// downgrading `writable: true -> false` on a non-configurable property.
function applyWebIdlInterfaceShape(interface_) {
  ObjectDefineProperty(interface_, "length", {
    __proto__: null,
    value: 0,
    writable: false,
    enumerable: false,
    configurable: true,
  });
  ObjectDefineProperty(interface_, "prototype", {
    __proto__: null,
    writable: false,
  });
}

// `CryptoKey` is the cppgc-wrapped Rust class imported above; the `type`,
// `extractable`, `usages` and `algorithm` getters and the underlying state
// all live in Rust (`ext/crypto/crypto_key.rs`). The JS shim only attaches
// the `Deno.privateCustomInspect` symbol to the prototype.
const CryptoKeyPrototype = CryptoKey.prototype;
ObjectDefineProperty(
  CryptoKeyPrototype,
  SymbolFor("Deno.privateCustomInspect"),
  {
    __proto__: null,
    value: function (inspect, inspectOptions) {
      return inspect(
        createFilteredInspectProxy({
          object: this,
          evaluate: ObjectPrototypeIsPrototypeOf(CryptoKeyPrototype, this),
          keys: [
            "type",
            "extractable",
            "algorithm",
            "usages",
          ],
        }),
        inspectOptions,
      );
    },
    enumerable: false,
    configurable: true,
    writable: true,
  },
);
webidl.configureInterface(CryptoKey);
applyWebIdlInterfaceShape(CryptoKey);

// Structured-clone resurrection. The host-object brand stamped onto every
// CryptoKey by `make_crypto_key` (`ext/crypto/make_key.rs`) returns a
// snapshot with shape `{ type: "CryptoKey", keyType, extractable, usages,
// algorithm, keyData }`; the static method `CryptoKey.fromCloneData(data)`
// (`ext/crypto/node_interop.rs::from_clone_data`) parses the snapshot back
// into a freshly-minted cppgc instance.
core.registerCloneableResource(
  "CryptoKey",
  (data) => CryptoKey.fromCloneData(data),
);

// `SubtleCrypto.prototype.deriveBits` is a cppgc method declared with three
// formal params (`algorithm`, `baseKey`, `length`). The op2 macro has no
// way to declare an *optional* param while keeping the macro-level
// minimum-arg check, so we cannot use `#[required(2)]` (it would also cap
// the `Function.length` slot to 2 and route through `async_op_2`, which
// silently drops the third user argument before it reaches Rust -- see
// `setUpAsyncStub` in `libs/core/00_infra.js`). The spec (and WebIDL idl
// harness) requires `Function.length === 2` for
// `deriveBits(AlgorithmIdentifier, CryptoKey, optional unsigned long?)`,
// so wrap the cppgc method in a small forwarder whose declared params
// give it `length === 2` (the `length` default doesn't count) and
// explicitly pass all three args through.
const SubtleCryptoPrototype = SubtleCrypto.prototype;
const cppgcDeriveBits = SubtleCryptoPrototype.deriveBits;
const deriveBitsForwarder = {
  async deriveBits(algorithm, baseKey, length = undefined) {
    return await FunctionPrototypeCall(
      cppgcDeriveBits,
      this,
      algorithm,
      baseKey,
      length,
    );
  },
}.deriveBits;
ObjectDefineProperty(SubtleCryptoPrototype, "deriveBits", {
  __proto__: null,
  value: deriveBitsForwarder,
  writable: true,
  enumerable: true,
  configurable: true,
});

// The WebCrypto spec declares `importKey`, `getPublicKey`, `unwrapKey`,
// `encapsulateKey`, and `decapsulateKey` as returning Promises. The cppgc
// impls (`subtle_crypto.rs`) run their bodies synchronously because the
// per-algorithm work is bounded (no IO, no large key derivation off-CPU),
// but a sync error path would propagate to JS as a synchronous `throw`.
// That breaks `assertRejects` callers in the test suite and any
// `.catch()`-only consumer. The async wrappers below coerce both the
// success and error paths through a Promise, matching the legacy JS
// async-fn shape.
function makeAsyncForwarder(name, methodName, arity) {
  const cppgc = SubtleCryptoPrototype[methodName];
  // The `name` parameter is captured in the wrapper's `Function.name`
  // slot via a property assignment because async-arrow `function.name`
  // would otherwise be `"makeAsyncForwarder"` from the surrounding fn.
  const wrapper = {
    async [methodName](...args) {
      // `await` keeps `dlint require-await` happy and is a no-op when the
      // underlying cppgc method returns a non-thenable (the WebCrypto
      // surface guarantees a CryptoKey/array/dict, not a Promise).
      return await FunctionPrototypeCall(
        cppgc,
        this,
        ...new SafeArrayIterator(args),
      );
    },
  }[methodName];
  // `Function.length` of `(...args) => ...` is 0, but the WebIDL idl-harness
  // test (`SubtleCrypto interface: operation <name>(...)`) requires it to
  // match the operation's required-argument count per the spec.
  ObjectDefineProperty(wrapper, "length", {
    __proto__: null,
    value: arity,
    configurable: true,
  });
  ObjectDefineProperty(SubtleCryptoPrototype, name, {
    __proto__: null,
    value: wrapper,
    writable: true,
    enumerable: true,
    configurable: true,
  });
}
// Per WebCrypto spec, every SubtleCrypto method returns a Promise. The
// op2-generated dispatchers invoke `WebIdlConverter`s synchronously before
// the async body runs, so a converter-level throw (`TypeError: Missing
// 'modulusLength'`, `Unrecognized algorithm`, etc.) reaches the call site
// as a synchronous exception. WPT's `promise_rejects_dom` wraps the call
// in `fn.call(undefined)`, which then surfaces the throw as
// `TypeError: Failed to execute 'call' on 'SubtleCrypto': ...` -- a wrong
// shape compared to the spec's "rejected promise". Forward every method
// through `async` so the throw becomes a Promise rejection.
// The third argument is the required-arg count from the WebCrypto IDL,
// applied to the wrapper's `Function.length` for idlharness compliance.
makeAsyncForwarder("digest", "digest", 2);
makeAsyncForwarder("encrypt", "encrypt", 3);
makeAsyncForwarder("decrypt", "decrypt", 3);
makeAsyncForwarder("sign", "sign", 3);
makeAsyncForwarder("verify", "verify", 4);
makeAsyncForwarder("deriveKey", "deriveKey", 5);
makeAsyncForwarder("importKey", "importKey", 5);
makeAsyncForwarder("exportKey", "exportKey", 2);
makeAsyncForwarder("generateKey", "generateKey", 3);
makeAsyncForwarder("getPublicKey", "getPublicKey", 2);
makeAsyncForwarder("wrapKey", "wrapKey", 4);
makeAsyncForwarder("unwrapKey", "unwrapKey", 7);
makeAsyncForwarder("encapsulateBits", "encapsulateBits", 2);
makeAsyncForwarder("encapsulateKey", "encapsulateKey", 5);
makeAsyncForwarder("decapsulateBits", "decapsulateBits", 3);
makeAsyncForwarder("decapsulateKey", "decapsulateKey", 6);

// `SubtleCrypto`'s prototype keeps a single privateCustomInspect helper so
// `Deno.inspect(crypto.subtle)` prints `SubtleCrypto {}` rather than the
// internal cppgc shape.
ObjectAssign(SubtleCryptoPrototype, {
  [SymbolFor("Deno.privateCustomInspect")](inspect, inspectOptions) {
    return `${this.constructor.name} ${inspect({}, inspectOptions)}`;
  },
});

webidl.configureInterface(SubtleCrypto);
applyWebIdlInterfaceShape(SubtleCrypto);

// The `SubtleCrypto` singleton (reachable as `globalThis.crypto.subtle`) is
// minted lazily: `op_create_subtle_crypto` allocates a cppgc-wrapped
// instance, and the cppgc heap isn't attached to the V8 isolate at
// snapshot-build time. The first runtime read of `crypto.subtle` calls
// `getSubtleSingleton`, which also stamps the `webidl.brand` symbol onto
// the instance so the `assertBranded` checks at the top of every method
// body pass. The same call hands `webidl.brand` and `kKeyObject` to Rust
// so freshly-minted `CryptoKey`s carry both brands.
let subtleSingleton;
function getSubtleSingleton() {
  if (subtleSingleton === undefined) {
    Crypto.registerSymbols(webidl.brand, kKeyObject);
    subtleSingleton = op_create_subtle_crypto();
    subtleSingleton[webidl.brand] = webidl.brand;
  }
  return subtleSingleton;
}

// `Crypto` is the cppgc-wrapped Rust class imported above; `getRandomValues`,
// `randomUUID` and the `subtle` getter are implemented natively in
// `crypto.rs`. Here we only decorate the prototype with the inspector hook
// and the WebIDL `Symbol.toStringTag` machinery, then mint the singleton
// via `op_create_crypto`.
const CryptoPrototype = Crypto.prototype;
ObjectDefineProperty(CryptoPrototype, SymbolFor("Deno.privateCustomInspect"), {
  __proto__: null,
  value: function (inspect, inspectOptions) {
    return inspect(
      createFilteredInspectProxy({
        object: this,
        evaluate: ObjectPrototypeIsPrototypeOf(CryptoPrototype, this),
        keys: ["subtle"],
      }),
      inspectOptions,
    );
  },
  enumerable: false,
  configurable: true,
  writable: true,
});
webidl.configureInterface(Crypto);
applyWebIdlInterfaceShape(Crypto);

let cryptoSingleton;
function getCryptoSingleton() {
  if (cryptoSingleton === undefined) {
    cryptoSingleton = op_create_crypto(getSubtleSingleton());
    // Stamp the WebIDL brand so `Reflect.getPrototypeOf(crypto)` and
    // the IDL `Crypto interface: operation randomUUID()` invariants
    // resolve through the same brand-check path as `SubtleCrypto`.
    cryptoSingleton[webidl.brand] = webidl.brand;
  }
  return cryptoSingleton;
}

// Bridge functions for Node.js KeyObject interop -- thin trampolines onto
// the cppgc static methods declared on the `CryptoKey` class in
// `ext/crypto/crypto_key.rs` (which delegate to `node_interop.rs`).
function cryptoKeyExportNodeKeyMaterial(cryptoKey) {
  return CryptoKey.exportNodeMaterial(cryptoKey);
}

function importCryptoKeySync(format, keyData, algorithm, extractable, usages) {
  return CryptoKey.importSync(format, keyData, algorithm, extractable, usages);
}

return {
  Crypto,
  get crypto() {
    return getCryptoSingleton();
  },
  CryptoKey,
  cryptoKeyExportNodeKeyMaterial,
  importCryptoKeySync,
  SubtleCrypto,
};
})();
