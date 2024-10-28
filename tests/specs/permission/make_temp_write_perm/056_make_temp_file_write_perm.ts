Deno.mkdirSync("subdir");

// async file
{
  const path = await Deno.makeTempFile({ dir: `subdir` });
  try {
    if (!path.match(/^subdir[/\\][^/\\]+/)) {
      throw Error("bad " + path);
    }
    console.log("good", path);
  } finally {
    await Deno.remove(path);
  }
}
// sync file
{
  const path = Deno.makeTempFileSync({ dir: `subdir` });
  try {
    if (!path.match(/^subdir[/\\][^/\\]+/)) {
      throw Error("bad " + path);
    }
    console.log("good", path);
  } finally {
    await Deno.remove(path);
  }
}

// async dir
{
  const path = await Deno.makeTempDir({ dir: `subdir` });
  try {
    if (!path.match(/^subdir[/\\][^/\\]+/)) {
      throw Error("bad " + path);
    }
    console.log("good", path);
  } finally {
    await Deno.remove(path);
  }
}

// sync dir
{
  const path = Deno.makeTempDirSync({ dir: `subdir` });
  try {
    if (!path.match(/^subdir[/\\][^/\\]+/)) {
      throw Error("bad " + path);
    }
    console.log("good", path);
  } finally {
    await Deno.remove(path);
  }
}
