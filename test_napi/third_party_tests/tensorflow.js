const tfjs = Deno.core.dlopen(
  "node_modules/@tensorflow/tfjs-node/build-tmp-napi-v8/Release/tfjs_binding.node",
);
console.log(tfjs);
