// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertRejects,
  assertThrows,
  deferred,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function symlinkSyncSuccess() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    Deno.symlinkSync(oldname, newname);
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink);
    assert(newNameInfoStat.isDirectory);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function symlinkSyncURL() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    Deno.symlinkSync(
      pathToAbsoluteFileUrl(oldname),
      pathToAbsoluteFileUrl(newname),
    );
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink);
    assert(newNameInfoStat.isDirectory);
  },
);

unitTest(function symlinkSyncPerm() {
  assertThrows(() => {
    Deno.symlinkSync("oldbaddir", "newbaddir");
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkSuccess() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    await Deno.symlink(oldname, newname);
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink, "NOT SYMLINK");
    assert(newNameInfoStat.isDirectory, "NOT DIRECTORY");
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkURL() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);
    await Deno.symlink(
      pathToAbsoluteFileUrl(oldname),
      pathToAbsoluteFileUrl(newname),
    );
    const newNameInfoLStat = Deno.lstatSync(newname);
    const newNameInfoStat = Deno.statSync(newname);
    assert(newNameInfoLStat.isSymlink, "NOT SYMLINK");
    assert(newNameInfoStat.isDirectory, "NOT DIRECTORY");
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function symlinkPermissionDeniedOnMissingSourceRead() {
    const testDir = Deno.makeTempDirSync();
    const oldname = testDir + "/oldname";
    const newname = testDir + "/newname";
    Deno.mkdirSync(oldname);

    const promise = deferred();
    const worker = new Worker(
      new URL("../testdata/symlink_permission.ts", import.meta.url).toString(),
      {
        type: "module",
        deno: {
          namespace: true,
          permissions: {
            write: [oldname, newname],
            read: [newname],
          },
        },
      },
    );
    worker.onmessage = (data) => {
      assert(data == "ok");
      // worker.terminate();
      promise.resolve();
    };

    worker.postMessage({ oldname, newname });
    await promise;
  },
);

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingDestinationRead() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "read", path: newname });
//     await assertRejects(async () => {
//       await Deno.symlink(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingSourceWrite() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "write", path: oldname });
//     await assertRejects(async () => {
//       await Deno.symlink(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingDestinationWrite() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "write", path: newname });
//     await assertRejects(async () => {
//       await Deno.symlink(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingSourceReadSync() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "read", path: oldname });
//     assertThrows(() => {
//       Deno.symlinkSync(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingDestinationReadSync() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "read", path: newname });
//     assertThrows(() => {
//       Deno.symlinkSync(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingSourceWriteSync() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "write", path: oldname });
//     assertThrows(() => {
//       Deno.symlinkSync(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );

// unitTest(
//   { perms: { read: true, write: true } },
//   async function symlinkPermissionDeniedOnMissingDestinationWriteSync() {
//     const testDir = Deno.makeTempDirSync();
//     const oldname = testDir + "/oldname";
//     const newname = testDir + "/newname";
//     Deno.mkdirSync(oldname);
//     await Deno.permissions.revoke({ name: "write", path: newname });
//     assertThrows(() => {
//       Deno.symlinkSync(oldname, newname);
//     }, Deno.errors.PermissionDenied);
//   },
// );
