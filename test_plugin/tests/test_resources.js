import { createResource as createResourceOp } from "./ops.js";

const textEncoder = new TextEncoder();

function createResource(name) {
  const response = createResourceOp.dispatch(textEncoder.encode(name));
  const rid = new DataView(response.buffer, 0, 4).getUint32(0);
  return {
    rid
  };
}

function dropResource(resource) {
  Deno.close(resource.rid);
}

const one = createResource("one");
const two = createResource("two");
console.log(Deno.resources()[one.rid]);
console.log(Deno.resources()[two.rid]);
dropResource(two);
dropResource(one);
