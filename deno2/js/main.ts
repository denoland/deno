const globalEval = eval;
const window = globalEval("this");
window['foo'] = () => {
  deno_print("Hello world from foo");
  return "foo";
}

