// import * as msg from "gen/msg_generated";
import { args, cwd, exit, run, chdir } from "deno";
// import { symlink } from "./symlink";

function qrun(args: string[]) {
  run({
    args
  });
}

const rootPath = (): string => {
  // remove '/js' in end of cwd to go on root path
  return cwd().slice(0, -3);
};

const joinPath = (joinSet: string[]): string => {
  return joinSet.join("/");
};

const clangFormat = () => {
  console.log("clang_format");
};

const gnFormat = () => {
  console.log("gn Format");
};

const yapf = () => {
  console.log("yapf");
};

const prettier = () => {
  console.log("prettier");
};

const rustfmt = () => {
  console.log("rustfmt");
};

function format() {
  const prettier = joinPath([
    rootPath(),
    "third_party",
    "node_modules",
    "prettier",
    "bin-prettier.js"
  ]);
  const toolsPath = joinPath([rootPath(), "tools"]);
  const rustfmtConfig = joinPath([toolsPath, "rustfmt.toml"]);
  console.log(rustfmtConfig);
}

format();
