import { cwd, readDirSync, run } from "deno";

const rootPath = (): string => {
  // remove '/js' in end of cwd to go on root path
  return cwd();
};

const walkExtSync = (path: string, exts: string[], pathList: string[], skipPaths?: string[]): string[] => {
  const files = readDirSync(path)
  files.forEach((file) => {
    if (file.isDirectory()) {
      if (skipPaths) {
        let shouldSkip = false
        for (let skipPath of skipPaths) {
          if (file.path.endsWith(skipPath)) {
            shouldSkip = true
          }
        }
        if (!shouldSkip) {
          walkExtSync(file.path, exts, pathList)  
        }
      } else {
        walkExtSync(file.path, exts, pathList)
      }
    } else {
      for (let ext of exts) {
        if (file.name.endsWith(ext)) {
          pathList.push(file.path)
        }
      }
    }
  })

  // console.log(recursiveFileList)
  return pathList
}
// this should return array of file path which has extension in 'exts'
const findExtsSync = (path: string, exts: string[]): string[] => {
  const files = readDirSync(path)
  let fileWithTargetExts = []
  files.forEach((file) => {
    for (let ext of exts) {
      if (file.name.endsWith(ext)) {
        fileWithTargetExts.push(file.path)
      }
    }
  })

  return fileWithTargetExts
}

const clangFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/clang-format";
};

const gnFormatPath = (): string => {
  return rootPath() + "/third_party/depot_tools/gn";
}

const joinPath = (joinSet: string[]): string => {
  return joinSet.join("/");
};

const clangFormat = () => {
  console.log("clang Format");
  const fileList = walkExtSync(rootPath() + '/libdeno', ['.cc', '.h'], [])
  console.log(fileList)
  /*
  run({
    args: [
      clangFormatPath(),
      "-i",
      "-style",
      "Google",
    ].concat(fileList)
  });
  */
};

const gnFilePaths = [
  '',
  '/build_extra/rust',
  '/build_extra/flatbuffers',
  '/build_extra/flatbuffers/rust',
  '/libdeno'
]

const gnFormat = () => {
  console.log("gn Format");
  const fileList = walkExtSync(rootPath() + '/build_extra', ['.gn', '.gni'], [])
                    .concat(walkExtSync(rootPath() + '/libdeno', ['.gn', '.gni'], []))
  console.log(fileList)
  /*
  let filesToFormat = [];
  gnFilePaths.forEach((path) => {
    filesToFormat = filesToFormat.concat(findExts(rootPath() + path, ['.gn', '.gni']))
  })
  
  for (let file of filesToFormat) {
    run({
      args: [
        gnFormatPath(),
        "format",
        file
      ]
    });
  }
  */
};

const yapf = () => {
  console.log("yapf");
  let fileList = []
  fileList = findExtsSync(rootPath(), ['.json', '.md']).concat(['.github'])
  fileList.concat(walkExtSync(rootPath() + '/js', [".js", ".json", ".ts", ".md"], [], ["js/deps"]))
  fileList.concat(walkExtSync(rootPath() + '/tests', [".js", ".json", ".ts", ".md"], []))
  fileList.concat(walkExtSync(rootPath() + '/tools', [".js", ".json", ".ts", ".md"], [], ["tools/clang"]))
  fileList.concat(walkExtSync(rootPath() + '/website', [".js", ".json", ".ts", ".md"], []))

  console.log(fileList)
  /*
  qrun(["node", prettier, "--write", "--loglevel=error"] + ["rollup.config.js"] +
     glob("*.json") + glob("*.md") +
     find_exts([".github", "js", "tests", "tools", "website"],
               [".js", ".json", ".ts", ".md"],
               skip=["tools/clang", "js/deps"]))
               });
  */
 
  run({
    args: [
      'python',
      rootPath() + "/third_party/python_packages/bin/yapf",
      "-style",
      "Google",
    ].concat(findExts(rootPath() + '/libdeno', ['.cc', '.h']))
  });
};

const prettier = () => {
  console.log("prettier");
  /*
  qrun(["node", prettier, "--write", "--loglevel=error"] + ["rollup.config.js"] +
     glob("*.json") + glob("*.md") +
     find_exts([".github", "js", "tests", "tools", "website"],
               [".js", ".json", ".ts", ".md"],
               skip=["tools/clang", "js/deps"]))
  */
  const prettier = joinPath([
    rootPath(),
    "third_party",
    "node_modules",
    "prettier",
    "bin-prettier.js"
  ]);
  const targetFiles = ['rollup.config.js'].concat(findExts(rootPath(), ['.json', '.md']))
  

  run({
    args: [
      'node',
      prettier,
      '--write',
      '--loglevel=error'
    ]
  });
};

const rustfmt = () => {
  console.log("rustfmt");
};

function format() {
  
  const toolsPath = joinPath([rootPath(), "tools"]);
  const rustfmtConfig = joinPath([toolsPath, "rustfmt.toml"]);
  // clangFormat();
  // gnFormat();
}

format();
