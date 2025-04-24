function realPrintName(name: string, level = 0) {
  console.log("-".repeat(level + 1) + " " + name);
}

const httpCacheNames: Map<string, string> = new Map();

function printName(name: string, level = 0) {
  const nameNoExt = name.replace(/\.js$/, "");
  const urlName = httpCacheNames.get(nameNoExt);
  if (urlName) {
    realPrintName(urlName + " (" + name + ")", level);
  } else {
    realPrintName(name, level);
  }
}

function walk(
  dir: string,
  fn: (
    { name, dir, level, kind }: {
      name: string;
      dir: string;
      level: number;
      kind: "file" | "dir" | "symlink";
    },
  ) => void,
  { maxLevel }: { maxLevel: number },
) {
  const walkRecursive = (dir: string, level: number) => {
    if (maxLevel > 0 && level > maxLevel) {
      return;
    }
    const files = Deno.readDirSync(dir).toArray();
    files.sort((a, b) => a.name.localeCompare(b.name));
    for (const file of files) {
      if (file.isDirectory) {
        fn({ name: file.name, dir, level, kind: "dir" });
        walkRecursive(dir + "/" + file.name, level + 1);
      } else if (file.isFile) {
        fn({ name: file.name, dir, level, kind: "file" });
      } else {
        fn({ name: file.name, dir, level, kind: "symlink" });
      }
    }
  };
  walkRecursive(dir, 0);
}

function printDir(dir: string, maxLevel = 0) {
  walk(dir, ({ name, level, kind }) => {
    printName(name + (kind === "symlink" ? " (symlink)" : ""), level);
  }, { maxLevel });
}

function collectHttpCacheNames(
  { name, dir, kind }: {
    name: string;
    dir: string;
    kind: "file" | "dir" | "symlink";
  },
) {
  if (
    kind === "file" &&
    name.length === 64 && name.match(/^[0-9a-f]{64}$/) && dir.includes("http")
  ) {
    const fullPath = dir + "/" + name;
    const contents = Deno.readTextFileSync(fullPath);
    const lastLine = contents.split("\n").pop();
    const cacheLine = "// denoCacheMetadata=";
    if (!lastLine?.startsWith(cacheLine)) {
      return;
    }
    const meta = JSON.parse(lastLine.slice(cacheLine.length));
    httpCacheNames.set(name, meta.url);
  }
}

let maxLevel = 0;
for (let i = 0; i < Deno.args.length; i++) {
  const arg = Deno.args[i];
  if (arg.startsWith("--max-level=")) {
    maxLevel = parseInt(arg.split("=")[1]) - 1;
    maxLevel = Math.max(0, maxLevel);
  } else {
    walk(arg, collectHttpCacheNames, { maxLevel });
    console.log(arg);
    printDir(arg, maxLevel);
    if (i < Deno.args.length - 1) {
      console.log("");
      maxLevel = 0;
    }
  }
}
