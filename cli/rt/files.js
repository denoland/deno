System.register(
  "$deno$/files.ts",
  [
    "$deno$/ops/resources.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/fs/seek.ts",
    "$deno$/ops/fs/open.ts",
  ],
  function (exports_32, context_32) {
    "use strict";
    let resources_ts_1, io_ts_3, seek_ts_1, open_ts_1, File;
    const __moduleName = context_32 && context_32.id;
    /**@internal*/
    function openSync(path, modeOrOptions = "r") {
      let openMode = undefined;
      let options = undefined;
      if (typeof modeOrOptions === "string") {
        openMode = modeOrOptions;
      } else {
        checkOpenOptions(modeOrOptions);
        options = modeOrOptions;
      }
      const rid = open_ts_1.openSync(path, openMode, options);
      return new File(rid);
    }
    exports_32("openSync", openSync);
    /**@internal*/
    async function open(path, modeOrOptions = "r") {
      let openMode = undefined;
      let options = undefined;
      if (typeof modeOrOptions === "string") {
        openMode = modeOrOptions;
      } else {
        checkOpenOptions(modeOrOptions);
        options = modeOrOptions;
      }
      const rid = await open_ts_1.open(path, openMode, options);
      return new File(rid);
    }
    exports_32("open", open);
    function createSync(path) {
      return openSync(path, "w+");
    }
    exports_32("createSync", createSync);
    function create(path) {
      return open(path, "w+");
    }
    exports_32("create", create);
    function checkOpenOptions(options) {
      if (Object.values(options).filter((val) => val === true).length === 0) {
        throw new Error("OpenOptions requires at least one option to be true");
      }
      if (options.truncate && !options.write) {
        throw new Error("'truncate' option requires 'write' option");
      }
      const createOrCreateNewWithoutWriteOrAppend =
        (options.create || options.createNew) &&
        !(options.write || options.append);
      if (createOrCreateNewWithoutWriteOrAppend) {
        throw new Error(
          "'create' or 'createNew' options require 'write' or 'append' option"
        );
      }
    }
    return {
      setters: [
        function (resources_ts_1_1) {
          resources_ts_1 = resources_ts_1_1;
        },
        function (io_ts_3_1) {
          io_ts_3 = io_ts_3_1;
        },
        function (seek_ts_1_1) {
          seek_ts_1 = seek_ts_1_1;
          exports_32({
            seek: seek_ts_1_1["seek"],
            seekSync: seek_ts_1_1["seekSync"],
          });
        },
        function (open_ts_1_1) {
          open_ts_1 = open_ts_1_1;
        },
      ],
      execute: function () {
        File = class File {
          constructor(rid) {
            this.rid = rid;
          }
          write(p) {
            return io_ts_3.write(this.rid, p);
          }
          writeSync(p) {
            return io_ts_3.writeSync(this.rid, p);
          }
          read(p) {
            return io_ts_3.read(this.rid, p);
          }
          readSync(p) {
            return io_ts_3.readSync(this.rid, p);
          }
          seek(offset, whence) {
            return seek_ts_1.seek(this.rid, offset, whence);
          }
          seekSync(offset, whence) {
            return seek_ts_1.seekSync(this.rid, offset, whence);
          }
          close() {
            resources_ts_1.close(this.rid);
          }
        };
        exports_32("File", File);
        exports_32("stdin", new File(0));
        exports_32("stdout", new File(1));
        exports_32("stderr", new File(2));
      },
    };
  }
);
