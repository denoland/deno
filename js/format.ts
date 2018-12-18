import { cwd, run } from "deno";

function qrun(args: string[]) {
  run({
    args
  });
}

const rootPath = (): string => {
  // remove '/js' in end of cwd to go on root path
  return cwd().slice(0, -3);
}

const clangFormatPath = (): string => {
  return rootPath() + '/third_party/depot_tools/clang-format'
}

const joinPath = (joinSet: string[]): string => {
  return joinSet.join('/')
}

const clangFormat = () => {
  console.log("clang_format");
// qrun([clangFormatPath(), "-i", "-style", "Google"] + find_exts(["libdeno"], [".cc", ".h"]))
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
  const prettier = joinPath([rootPath(), 'third_party', 'node_modules', "prettier", "bin-prettier.js"]);
  const toolsPath = joinPath([rootPath(), 'tools']);
  const rustfmtConfig = joinPath([toolsPath, 'rustfmt.toml']);
  console.log(rustfmtConfig);
}

format();
