import { args, env, exit, makeTempDirSync } from "deno";


const tests = {};
tests['test_needs_write'] = function() {
  const tempDir = makeTempDirSync();
}
tests['test_needs_env'] = function() {
  const home = env().home;
}
tests['test_needs_net'] = function() {
  fetch("http://localhost:4545");
}


const test_name = args[1]

if (test_name in tests) {
  tests[test_name]();
} else {
  console.log("Unknown test:", test_name);
  exit(1)
}

