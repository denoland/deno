System.register("$deno$/file_info.ts", ["$deno$/build.ts"], function (
  exports_39,
  context_39
) {
  "use strict";
  let build_ts_2, FileInfoImpl;
  const __moduleName = context_39 && context_39.id;
  return {
    setters: [
      function (build_ts_2_1) {
        build_ts_2 = build_ts_2_1;
      },
    ],
    execute: function () {
      // @internal
      FileInfoImpl = class FileInfoImpl {
        /* @internal */
        constructor(res) {
          const isUnix =
            build_ts_2.build.os === "mac" || build_ts_2.build.os === "linux";
          const modified = res.modified;
          const accessed = res.accessed;
          const created = res.created;
          const name = res.name;
          // Unix only
          const {
            dev,
            ino,
            mode,
            nlink,
            uid,
            gid,
            rdev,
            blksize,
            blocks,
          } = res;
          this.#isFile = res.isFile;
          this.#isDirectory = res.isDirectory;
          this.#isSymlink = res.isSymlink;
          this.size = res.size;
          this.modified = modified ? modified : null;
          this.accessed = accessed ? accessed : null;
          this.created = created ? created : null;
          this.name = name ? name : null;
          // Only non-null if on Unix
          this.dev = isUnix ? dev : null;
          this.ino = isUnix ? ino : null;
          this.mode = isUnix ? mode : null;
          this.nlink = isUnix ? nlink : null;
          this.uid = isUnix ? uid : null;
          this.gid = isUnix ? gid : null;
          this.rdev = isUnix ? rdev : null;
          this.blksize = isUnix ? blksize : null;
          this.blocks = isUnix ? blocks : null;
        }
        #isFile;
        #isDirectory;
        #isSymlink;
        isFile() {
          return this.#isFile;
        }
        isDirectory() {
          return this.#isDirectory;
        }
        isSymlink() {
          return this.#isSymlink;
        }
      };
      exports_39("FileInfoImpl", FileInfoImpl);
    },
  };
});
