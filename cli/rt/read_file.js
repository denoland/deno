System.register(
  "$deno$/read_file.ts",
  ["$deno$/files.ts", "$deno$/buffer.ts"],
  function (exports_54, context_54) {
    "use strict";
    let files_ts_3, buffer_ts_2;
    const __moduleName = context_54 && context_54.id;
    function readFileSync(path) {
      const file = files_ts_3.openSync(path);
      const contents = buffer_ts_2.readAllSync(file);
      file.close();
      return contents;
    }
    exports_54("readFileSync", readFileSync);
    async function readFile(path) {
      const file = await files_ts_3.open(path);
      const contents = await buffer_ts_2.readAll(file);
      file.close();
      return contents;
    }
    exports_54("readFile", readFile);
    return {
      setters: [
        function (files_ts_3_1) {
          files_ts_3 = files_ts_3_1;
        },
        function (buffer_ts_2_1) {
          buffer_ts_2 = buffer_ts_2_1;
        },
      ],
      execute: function () {},
    };
  }
);
