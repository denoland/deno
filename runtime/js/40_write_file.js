// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
((window) => {
  const { stat, statSync, chmod, chmodSync } = window.__bootstrap.fs;
  const { open, openSync } = window.__bootstrap.files;
  const { writeAll, writeAllSync } = window.__bootstrap.buffer;
  const { build } = window.__bootstrap.build;

  function writeFileSync(
    path,
    data,
    options = {},
  ) {
    if (options.create !== undefined) {
      const create = !!options.create;
      if (!create) {
        // verify that file exists
        statSync(path);
      }
    }

    const openOptions = options.append
      ? { write: true, create: true, append: true }
      : { write: true, create: true, truncate: true };
    const file = openSync(path, openOptions);

    if (
      options.mode !== undefined &&
      options.mode !== null &&
      build.os !== "windows"
    ) {
      chmodSync(path, options.mode);
    }

    writeAllSync(file, data);
    file.close();
  }

  async function writeFile(
    path,
    data,
    options = {},
  ) {
    if (options.create !== undefined) {
      const create = !!options.create;
      if (!create) {
        // verify that file exists
        await stat(path);
      }
    }

    const openOptions = options.append
      ? { write: true, create: true, append: true }
      : { write: true, create: true, truncate: true };
    const file = await open(path, openOptions);

    if (
      options.mode !== undefined &&
      options.mode !== null &&
      build.os !== "windows"
    ) {
      await chmod(path, options.mode);
    }

    await writeAll(file, data);
    file.close();
  }

  function writeTextFileSync(
    path,
    data,
    options = {},
  ) {
    const encoder = new TextEncoder();
    return writeFileSync(path, encoder.encode(data), options);
  }

  function writeTextFile(
    path,
    data,
    options = {},
  ) {
    const encoder = new TextEncoder();
    return writeFile(path, encoder.encode(data), options);
  }

  window.__bootstrap.writeFile = {
    writeTextFile,
    writeTextFileSync,
    writeFile,
    writeFileSync,
  };
})(this);
