// `unlock()` must not overtake a `lock()` that is still pending on the same
// file, otherwise the unlock is a no-op, the pending lock is acquired
// afterwards, and the file is left locked even though `unlock()` resolved.

// Locks are per open file description, so two handles to the same path contend
// with each other. Open with write access because on Windows LockFileEx
// requires GENERIC_READ/GENERIC_WRITE.
const openOptions = { read: true, write: true, create: true } as const;

const holder = await Deno.open("./test.lock", openOptions);
await holder.lock(true);

const other = await Deno.open("./test.lock", openOptions);
// blocks, because `holder` has the lock
const pendingLock = other.lock(true);
// must be serialized after `pendingLock`, so don't await it yet
const pendingUnlock = other.unlock();

// let `pendingLock` through
await holder.unlock();
holder.close();

await pendingLock;
await pendingUnlock;

// `other` unlocked, so a third handle must be able to take the lock
const probe = await Deno.open("./test.lock", openOptions);
console.log("lock released:", probe.tryLockSync(true));

probe.unlockSync();
probe.close();
other.close();
