import $ from "https://deno.land/x/dax/mod.ts";
import { MultiProgressBar } from "https://deno.land/x/progress@v1.3.8/mod.ts";

if (Deno.args.length === 0) {
  console.log(
    "Usage: build_bench [-v] [--profile release|debug] commit1 [commit2 [comment3...]]",
  );
  Deno.exit(1);
}

const args = Deno.args.slice();
let verbose = false;
if (args[0] == "-v") {
  args.shift();
  verbose = true;
}

let profile = "release";
if (args[0] == "--profile") {
  args.shift();
  profile = args.shift();
}

const progresses: { completed: number; total: number; text: string }[] = [];

const bars = new MultiProgressBar({
  title: `building profile ${profile}`,
  display: "[:bar] :time :text",
});

function exit(msg: string) {
  if (!verbose) {
    bars.end();
  }
  console.error(msg);
  Deno.exit(1);
}

// Make sure the .git dir exists
const gitDir = Deno.cwd() + "/.git";
await Deno.stat(gitDir);

const GIT_STEPS = 8;
const infos = [];

async function runCommand(human, cmd) {
  if (verbose) {
    const out = await cmd;
    if (out.code != 0) {
      exit(human);
    }
  } else {
    const out = await cmd.stdout("piped").stderr("piped");
    if (out.code != 0) {
      console.log(out.stdout);
      console.log(out.stderr);
      exit(human);
    }
  }
}

async function buildGitCommit(commit) {
  const progress = { completed: 0, total: GIT_STEPS, text: "?" };
  progresses.push(progress);

  const tempDir = await Deno.makeTempDir();
  progress.completed++;

  const gitInfo =
    await $`git log --pretty=oneline --abbrev-commit -n1 ${commit}`.stdout(
      "piped",
    ).stderr("piped");
  if (gitInfo.code != 0) {
    console.log(gitInfo.stdout);
    console.log(gitInfo.stderr);
    exit(`Failed to get git info for commit ${commit}`);
  }
  progress.completed++;

  const hash = gitInfo.stdout.split(" ")[0];
  progress.text = hash;

  progress.text = `clone ${hash}`;
  await runCommand(
    `Failed to clone commit ${commit}`,
    $`git clone ${gitDir} ${tempDir}`,
  );
  progress.completed++;

  progress.text = `reset ${hash}`;
  await runCommand(
    `Failed to reset commit ${commit}`,
    $`git reset --hard ${hash}`.cwd(tempDir),
  );
  progress.completed++;

  progress.text = `build ${hash}`;
  if (profile === "debug") {
    await runCommand(
      `Failed to build commit ${commit}`,
      $`cargo build`.cwd(tempDir),
    );
  } else {
    await runCommand(
      `Failed to build commit ${commit}`,
      $`cargo build --profile ${profile}`.cwd(tempDir),
    );
  }
  progress.completed++;

  let file;
  if (profile === "release") {
    file = `deno-${hash}`;
  } else {
    file = `deno-${profile}-${hash}`;
  }
  await Deno.copyFile(`${tempDir}/target/${profile}/deno`, file);
  progress.completed++;

  progress.text = `cleanup ${hash}`;
  await Deno.remove(tempDir, { recursive: true });
  progress.completed++;

  progress.text = "done";
  infos.push(`Built ./${file} (${commit}): ${gitInfo.stdout}`);
  progress.completed++;
}

const promises = [];
for (const arg of args) {
  promises.push(buildGitCommit(arg));
}

let barUpdater;
if (!verbose) {
  barUpdater = setInterval(() => {
    bars.render(progresses);
  }, 100);
  bars.render(progresses);
}

await Promise.all(promises);

if (!verbose) {
  bars.render(progresses);
  clearInterval(barUpdater);
  bars.end();
}

for (const info of infos) {
  console.log(info);
}
