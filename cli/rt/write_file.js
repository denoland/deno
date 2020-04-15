System.register(
  "$deno$/write_file.ts",
  [
    "$deno$/ops/fs/stat.ts",
    "$deno$/files.ts",
    "$deno$/ops/fs/chmod.ts",
    "$deno$/buffer.ts",
    "$deno$/build.ts",
  ],
  function (exports_68, context_68) {
    "use strict";
    let stat_ts_1, files_ts_4, chmod_ts_1, buffer_ts_3, build_ts_6;
    const __moduleName = context_68 && context_68.id;
    function writeFileSync(path, data, options = {}) {
      if (options.create !== undefined) {
        const create = !!options.create;
        if (!create) {
          // verify that file exists
          stat_ts_1.statSync(path);
        }
      }
      const openMode = !!options.append ? "a" : "w";
      const file = files_ts_4.openSync(path, openMode);
      if (
        options.mode !== undefined &&
        options.mode !== null &&
        build_ts_6.build.os !== "win"
      ) {
        chmod_ts_1.chmodSync(path, options.mode);
      }
      buffer_ts_3.writeAllSync(file, data);
      file.close();
    }
    exports_68("writeFileSync", writeFileSync);
    async function writeFile(path, data, options = {}) {
      if (options.create !== undefined) {
        const create = !!options.create;
        if (!create) {
          // verify that file exists
          await stat_ts_1.stat(path);
        }
      }
      const openMode = !!options.append ? "a" : "w";
      const file = await files_ts_4.open(path, openMode);
      if (
        options.mode !== undefined &&
        options.mode !== null &&
        build_ts_6.build.os !== "win"
      ) {
        await chmod_ts_1.chmod(path, options.mode);
      }
      await buffer_ts_3.writeAll(file, data);
      file.close();
    }
    exports_68("writeFile", writeFile);
    return {
      setters: [
        function (stat_ts_1_1) {
          stat_ts_1 = stat_ts_1_1;
        },
        function (files_ts_4_1) {
          files_ts_4 = files_ts_4_1;
        },
        function (chmod_ts_1_1) {
          chmod_ts_1 = chmod_ts_1_1;
        },
        function (buffer_ts_3_1) {
          buffer_ts_3 = buffer_ts_3_1;
        },
        function (build_ts_6_1) {
          build_ts_6 = build_ts_6_1;
        },
      ],
      execute: function () {},
    };
  }
);
