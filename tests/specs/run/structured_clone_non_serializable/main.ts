// Regression test for https://github.com/denoland/deno/issues/32914
// Web API platform types that aren't marked [Serializable] in their specs
// must throw DataCloneError when passed to structuredClone, matching Node
// and the Web Platform tests. Previously V8's serialiser saw these as
// plain objects with no own enumerable properties and silently produced
// `{}`.

function check(name: string, factory: () => unknown) {
  try {
    structuredClone(factory());
    console.log(`${name}: cloned (NO error)`);
  } catch (e) {
    const err = e as DOMException;
    console.log(`${name}: ${err.name} ${err.message}`);
  }
}

check("Response", () => new Response());
check("Request", () => new Request("http://localhost"));
check("Headers", () => new Headers());
check("ReadableStream", () => new ReadableStream());
check("WritableStream", () => new WritableStream());
check("TransformStream", () => new TransformStream());

// Sanity check: types that ARE serializable still clone successfully.
const obj = structuredClone({ a: 1, nested: { b: 2 } });
console.log("plain object:", JSON.stringify(obj));
const arr = structuredClone([1, [2, [3]]]);
console.log("array:", JSON.stringify(arr));
const d = structuredClone(new Date(0));
console.log("date:", d.toISOString());
