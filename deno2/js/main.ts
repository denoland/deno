const globalEval = eval;
const window = globalEval("this");
window["denoMain"] = () => {
  denoPrint("Hello world from foo");
  return "foo";
};
