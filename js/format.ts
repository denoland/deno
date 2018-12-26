import { cwd, readDirSync, run } from "deno";

const rootPath = (): string => {
  return cwd();
};

/*
 *  walk through directory recursively and return string list of file paths of certain extensions
 */
const walkExtSync = (
  path: string,
  exts: string[],
  pathList: string[],
  skipPaths?: string[]
): string[] => {
  const files = readDirSync(path);
  files.forEach(file => {
    if (file.isDirectory()) {
      if (skipPaths) {
        let shouldSkip = false;
        for (let skipPath of skipPaths) {
          if (file.path.endsWith(skipPath)) {
            shouldSkip = true;
          }
        }
        if (!shouldSkip) {
          walkExtSync(file.path, exts, pathList);
        }
      } else {
        walkExtSync(file.path, exts, pathList);
      }
    } else {
      for (let ext of exts) {
        if (file.name.endsWith(ext)) {
          pathList.push(file.path);
        }
      }
    }
  });

  return pathList;
};
/*
 *  walk through directory recursively and return string list of file paths of certain extensions
 */
const findExtsSync = (path: string, exts: string[]): string[] => {
  const files = readDirSync(path);
  let fileWithTargetExts = [];
  files.forEach(file => {
    for (let ext of exts) {
      if (file.name.endsWith(ext)) {
        fileWithTargetExts.push(file.path);
      }
    }
  });

  return fileWithTargetExts;
};

const clangFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/clang-format";
};

const gnFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/gn";
};

const joinPath = (joinSet: string[]): string => {
  return joinSet.join("/");
};

const clangFormat = () => {
  console.log("clang Format");
  const fileList = walkExtSync(rootPath() + "/libdeno", [".cc", ".h"], []);
  run({
    args: [clangFormatPath(), "-i", "-style", "Google"].concat(fileList)
  });
};

const gnFormat = () => {
  console.log("gn Format");
  const fileList = walkExtSync(
    rootPath() + "/build_extra",
    [".gn", ".gni"],
    []
  ).concat(walkExtSync(rootPath() + "/libdeno", [".gn", ".gni"], []));
  for (let file of fileList) {
    run({
      args: [gnFormatPath(), "format", file]
    });
  }
};

const yapf = () => {
  console.log("yapf");
  let fileList = walkExtSync(rootPath() + "/build_extra", [".py"], []).concat(
    walkExtSync(rootPath() + "/tools", [".py"], [], ["tools/clang"])
  );
  // Not working now...
  /*
  run({
    args: [
      "python",
      rootPath() + "/third_party/python_packages/bin/yapf",
      "-i",
      "Google"
    ].concat(fileList)
  });
  */
};

const prettierFormat = () => {
  console.log("prettier");
  const prettier = joinPath([
    rootPath(),
    "third_party",
    "node_modules",
    "prettier",
    "bin-prettier.js"
  ]);
  let fileList = [];
  fileList = findExtsSync(rootPath(), [".json", ".md"])
    .concat("rollup.config.js")
    .concat(
      walkExtSync(
        rootPath() + "/js",
        [".js", ".json", ".ts", ".md"],
        [],
        ["js/deps"]
      )
    )
    .concat(
      walkExtSync(rootPath() + "/tests", [".js", ".json", ".ts", ".md"], [])
    )
    .concat(
      walkExtSync(
        rootPath() + "/tools",
        [".js", ".json", ".ts", ".md"],
        [],
        ["tools/clang"]
      )
    )
    .concat(
      walkExtSync(rootPath() + "/website", [".js", ".json", ".ts", ".md"], [])
    );

  run({
    args: ["node", prettier, "--write", "--loglevel=error"].concat(fileList)
  });
};

const rustfmt = () => {
  console.log("rustfmt");
};

function format() {
  // const toolsPath = joinPath([rootPath(), "tools"]);
  // const rustfmtConfig = joinPath([toolsPath, "rustfmt.toml"]);
  clangFormat();
  gnFormat();
  yapf();
  prettierFormat();
}

format();
