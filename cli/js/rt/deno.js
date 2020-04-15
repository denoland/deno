// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
System.register(
  "$deno$/deno.ts",
  [
    "$deno$/buffer.ts",
    "$deno$/build.ts",
    "$deno$/ops/fs/chmod.ts",
    "$deno$/ops/fs/chown.ts",
    "$deno$/compiler/api.ts",
    "$deno$/web/console.ts",
    "$deno$/ops/fs/copy_file.ts",
    "$deno$/diagnostics.ts",
    "$deno$/ops/fs/dir.ts",
    "$deno$/ops/errors.ts",
    "$deno$/errors.ts",
    "$deno$/files.ts",
    "$deno$/ops/io.ts",
    "$deno$/ops/fs_events.ts",
    "$deno$/io.ts",
    "$deno$/ops/fs/link.ts",
    "$deno$/ops/fs/make_temp.ts",
    "$deno$/ops/runtime.ts",
    "$deno$/ops/fs/mkdir.ts",
    "$deno$/net.ts",
    "$deno$/ops/os.ts",
    "$deno$/permissions.ts",
    "$deno$/plugins.ts",
    "$deno$/ops/process.ts",
    "$deno$/process.ts",
    "$deno$/ops/fs/read_dir.ts",
    "$deno$/read_file.ts",
    "$deno$/ops/fs/read_link.ts",
    "$deno$/ops/fs/realpath.ts",
    "$deno$/ops/fs/remove.ts",
    "$deno$/ops/fs/rename.ts",
    "$deno$/ops/resources.ts",
    "$deno$/signals.ts",
    "$deno$/ops/fs/stat.ts",
    "$deno$/ops/fs/symlink.ts",
    "$deno$/tls.ts",
    "$deno$/ops/fs/truncate.ts",
    "$deno$/ops/tty.ts",
    "$deno$/ops/fs/umask.ts",
    "$deno$/ops/fs/utime.ts",
    "$deno$/version.ts",
    "$deno$/write_file.ts",
    "$deno$/testing.ts",
    "$deno$/core.ts",
    "$deno$/symbols.ts",
  ],
  function (exports_71, context_71) {
    "use strict";
    const __moduleName = context_71 && context_71.id;
    return {
      setters: [
        function (buffer_ts_4_1) {
          exports_71({
            Buffer: buffer_ts_4_1["Buffer"],
            readAll: buffer_ts_4_1["readAll"],
            readAllSync: buffer_ts_4_1["readAllSync"],
            writeAll: buffer_ts_4_1["writeAll"],
            writeAllSync: buffer_ts_4_1["writeAllSync"],
          });
        },
        function (build_ts_7_1) {
          exports_71({
            build: build_ts_7_1["build"],
          });
        },
        function (chmod_ts_2_1) {
          exports_71({
            chmodSync: chmod_ts_2_1["chmodSync"],
            chmod: chmod_ts_2_1["chmod"],
          });
        },
        function (chown_ts_1_1) {
          exports_71({
            chownSync: chown_ts_1_1["chownSync"],
            chown: chown_ts_1_1["chown"],
          });
        },
        function (api_ts_1_1) {
          exports_71({
            transpileOnly: api_ts_1_1["transpileOnly"],
            compile: api_ts_1_1["compile"],
            bundle: api_ts_1_1["bundle"],
          });
        },
        function (console_ts_3_1) {
          exports_71({
            inspect: console_ts_3_1["inspect"],
          });
        },
        function (copy_file_ts_1_1) {
          exports_71({
            copyFileSync: copy_file_ts_1_1["copyFileSync"],
            copyFile: copy_file_ts_1_1["copyFile"],
          });
        },
        function (diagnostics_ts_1_1) {
          exports_71({
            DiagnosticCategory: diagnostics_ts_1_1["DiagnosticCategory"],
          });
        },
        function (dir_ts_1_1) {
          exports_71({
            chdir: dir_ts_1_1["chdir"],
            cwd: dir_ts_1_1["cwd"],
          });
        },
        function (errors_ts_6_1) {
          exports_71({
            applySourceMap: errors_ts_6_1["applySourceMap"],
            formatDiagnostics: errors_ts_6_1["formatDiagnostics"],
          });
        },
        function (errors_ts_7_1) {
          exports_71({
            errors: errors_ts_7_1["errors"],
          });
        },
        function (files_ts_6_1) {
          exports_71({
            File: files_ts_6_1["File"],
            open: files_ts_6_1["open"],
            openSync: files_ts_6_1["openSync"],
            create: files_ts_6_1["create"],
            createSync: files_ts_6_1["createSync"],
            stdin: files_ts_6_1["stdin"],
            stdout: files_ts_6_1["stdout"],
            stderr: files_ts_6_1["stderr"],
            seek: files_ts_6_1["seek"],
            seekSync: files_ts_6_1["seekSync"],
          });
        },
        function (io_ts_5_1) {
          exports_71({
            read: io_ts_5_1["read"],
            readSync: io_ts_5_1["readSync"],
            write: io_ts_5_1["write"],
            writeSync: io_ts_5_1["writeSync"],
          });
        },
        function (fs_events_ts_1_1) {
          exports_71({
            fsEvents: fs_events_ts_1_1["fsEvents"],
          });
        },
        function (io_ts_6_1) {
          exports_71({
            EOF: io_ts_6_1["EOF"],
            copy: io_ts_6_1["copy"],
            toAsyncIterator: io_ts_6_1["toAsyncIterator"],
            SeekMode: io_ts_6_1["SeekMode"],
          });
        },
        function (link_ts_1_1) {
          exports_71({
            linkSync: link_ts_1_1["linkSync"],
            link: link_ts_1_1["link"],
          });
        },
        function (make_temp_ts_1_1) {
          exports_71({
            makeTempDirSync: make_temp_ts_1_1["makeTempDirSync"],
            makeTempDir: make_temp_ts_1_1["makeTempDir"],
            makeTempFileSync: make_temp_ts_1_1["makeTempFileSync"],
            makeTempFile: make_temp_ts_1_1["makeTempFile"],
          });
        },
        function (runtime_ts_5_1) {
          exports_71({
            metrics: runtime_ts_5_1["metrics"],
          });
        },
        function (mkdir_ts_1_1) {
          exports_71({
            mkdirSync: mkdir_ts_1_1["mkdirSync"],
            mkdir: mkdir_ts_1_1["mkdir"],
          });
        },
        function (net_ts_2_1) {
          exports_71({
            connect: net_ts_2_1["connect"],
            listen: net_ts_2_1["listen"],
            ShutdownMode: net_ts_2_1["ShutdownMode"],
            shutdown: net_ts_2_1["shutdown"],
          });
        },
        function (os_ts_2_1) {
          exports_71({
            dir: os_ts_2_1["dir"],
            env: os_ts_2_1["env"],
            exit: os_ts_2_1["exit"],
            execPath: os_ts_2_1["execPath"],
            hostname: os_ts_2_1["hostname"],
            loadavg: os_ts_2_1["loadavg"],
            osRelease: os_ts_2_1["osRelease"],
          });
        },
        function (permissions_ts_1_1) {
          exports_71({
            permissions: permissions_ts_1_1["permissions"],
            PermissionStatus: permissions_ts_1_1["PermissionStatus"],
            Permissions: permissions_ts_1_1["Permissions"],
          });
        },
        function (plugins_ts_2_1) {
          exports_71({
            openPlugin: plugins_ts_2_1["openPlugin"],
          });
        },
        function (process_ts_2_1) {
          exports_71({
            kill: process_ts_2_1["kill"],
          });
        },
        function (process_ts_3_1) {
          exports_71({
            run: process_ts_3_1["run"],
            Process: process_ts_3_1["Process"],
          });
        },
        function (read_dir_ts_1_1) {
          exports_71({
            readdirSync: read_dir_ts_1_1["readdirSync"],
            readdir: read_dir_ts_1_1["readdir"],
          });
        },
        function (read_file_ts_1_1) {
          exports_71({
            readFileSync: read_file_ts_1_1["readFileSync"],
            readFile: read_file_ts_1_1["readFile"],
          });
        },
        function (read_link_ts_1_1) {
          exports_71({
            readlinkSync: read_link_ts_1_1["readlinkSync"],
            readlink: read_link_ts_1_1["readlink"],
          });
        },
        function (realpath_ts_1_1) {
          exports_71({
            realpathSync: realpath_ts_1_1["realpathSync"],
            realpath: realpath_ts_1_1["realpath"],
          });
        },
        function (remove_ts_1_1) {
          exports_71({
            removeSync: remove_ts_1_1["removeSync"],
            remove: remove_ts_1_1["remove"],
          });
        },
        function (rename_ts_1_1) {
          exports_71({
            renameSync: rename_ts_1_1["renameSync"],
            rename: rename_ts_1_1["rename"],
          });
        },
        function (resources_ts_6_1) {
          exports_71({
            resources: resources_ts_6_1["resources"],
            close: resources_ts_6_1["close"],
          });
        },
        function (signals_ts_1_1) {
          exports_71({
            signal: signals_ts_1_1["signal"],
            signals: signals_ts_1_1["signals"],
            Signal: signals_ts_1_1["Signal"],
            SignalStream: signals_ts_1_1["SignalStream"],
          });
        },
        function (stat_ts_2_1) {
          exports_71({
            statSync: stat_ts_2_1["statSync"],
            lstatSync: stat_ts_2_1["lstatSync"],
            stat: stat_ts_2_1["stat"],
            lstat: stat_ts_2_1["lstat"],
          });
        },
        function (symlink_ts_1_1) {
          exports_71({
            symlinkSync: symlink_ts_1_1["symlinkSync"],
            symlink: symlink_ts_1_1["symlink"],
          });
        },
        function (tls_ts_1_1) {
          exports_71({
            connectTLS: tls_ts_1_1["connectTLS"],
            listenTLS: tls_ts_1_1["listenTLS"],
          });
        },
        function (truncate_ts_1_1) {
          exports_71({
            truncateSync: truncate_ts_1_1["truncateSync"],
            truncate: truncate_ts_1_1["truncate"],
          });
        },
        function (tty_ts_1_1) {
          exports_71({
            isatty: tty_ts_1_1["isatty"],
            setRaw: tty_ts_1_1["setRaw"],
          });
        },
        function (umask_ts_1_1) {
          exports_71({
            umask: umask_ts_1_1["umask"],
          });
        },
        function (utime_ts_1_1) {
          exports_71({
            utimeSync: utime_ts_1_1["utimeSync"],
            utime: utime_ts_1_1["utime"],
          });
        },
        function (version_ts_2_1) {
          exports_71({
            version: version_ts_2_1["version"],
          });
        },
        function (write_file_ts_1_1) {
          exports_71({
            writeFileSync: write_file_ts_1_1["writeFileSync"],
            writeFile: write_file_ts_1_1["writeFile"],
          });
        },
        function (testing_ts_1_1) {
          exports_71({
            runTests: testing_ts_1_1["runTests"],
            test: testing_ts_1_1["test"],
          });
        },
        function (core_ts_6_1) {
          exports_71({
            core: core_ts_6_1["core"],
          });
        },
        function (symbols_ts_1_1) {
          exports_71({
            symbols: symbols_ts_1_1["symbols"],
          });
        },
      ],
      execute: function () {
        exports_71("args", []);
      },
    };
  }
);
