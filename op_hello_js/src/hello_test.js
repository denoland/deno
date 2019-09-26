// As opposed to the TypeScript op_hello example, here we want to not rely on 
// deno_std, as it uses TypeScript. So here don't use a test runner.
import { hello } from "./hello.js";
hello();
