import version, {
  TestClass,
} from "deno:@denotest/no_module_graph@0.1.0/mod.ts";

console.log(version);
console.log(new TestClass());
