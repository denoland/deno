# Research: `Bun.file` API — Deno equivalents, gaps, and an opportunity

**Date:** 2026-06-14
**Status:** Research only (no implementation, no PR)
**Author:** Claude (commissioned research)

## 1. Summary / TL;DR

`Bun.file(path)` returns a **lazy, web-standard `Blob`** (a `BunFile`) that
references a file on disk (or a file descriptor, stdio stream, or `s3://` URL)
without doing any I/O until you read from it. Because it *is* a `Blob`/`File`,
you immediately get all the Web Platform ergonomics — `.text()`, `.json()`,
`.arrayBuffer()`, `.bytes()`, `.stream()`, `.slice()` — plus a few Bun-only
conveniences (`.exists()`, `.stat()`, `.delete()`, `.write()`, `.writer()`).
A companion static method, `Bun.write(dest, data)`, is a "do-what-I-mean" writer
that accepts almost any source and uses OS-level fast paths (e.g.
`copy_file_range`/`sendfile`) when copying file → file.

**Deno today has no single `Deno.file()`-style handle.** Instead it spreads the
same capabilities across `Deno.readFile`/`readTextFile`, `Deno.writeFile`/
`writeTextFile`, `Deno.open` → `Deno.FsFile`, `Deno.stat`, and the global
`Blob`/`File`/`Response` web APIs. Every individual task Bun solves with
`Bun.file` is achievable in Deno, but usually with more ceremony and without the
lazy-handle ergonomics.

A `Deno.file()` (or, better, a Web-aligned helper) would be a genuinely useful
addition — and Deno is well positioned to make it **more standards-faithful and
more capable** than Bun's version (true lazy `File`, AsyncIterable, permission
integration, `FileSystemFileHandle` alignment, URL support).

---

## 2. What `Bun.file` actually is

### 2.1 Construction

```ts
Bun.file(path: string | URL | ArrayBufferView, options?: { type?: string }): BunFile
Bun.file(fd: number, options?: { type?: string }): BunFile   // wrap a file descriptor
```

- `Bun.file("./package.json")` — reference a path. **No I/O happens here.**
- `Bun.file(13)` — wrap a numeric file descriptor.
- `Bun.file(path, { type: "application/json" })` — override the MIME type
  (otherwise inferred from the extension).
- Bun's standard streams are exposed as files/sinks: `Bun.stdin` (readable
  `BunFile`), `Bun.stdout`, `Bun.stderr` (writable sinks).
- Recent Bun also accepts `s3://` URLs (returns an `S3File`, a `BunFile`
  subclass) and treats remote object storage with the same interface.

### 2.2 `BunFile extends Blob`

The key design decision: **a `BunFile` is a `Blob`** (more precisely it behaves
like a lazy `File`). That single fact gives it the entire Web inheritance for
free:

| Member | Origin | Notes |
| --- | --- | --- |
| `.text()` | Web `Blob` | Read whole file as UTF-8 string |
| `.json()` | Bun conv. | Read + `JSON.parse` |
| `.arrayBuffer()` | Web `Blob` | Read as `ArrayBuffer` |
| `.bytes()` | Web `Blob` (newer) | Read as `Uint8Array` |
| `.stream()` | Web `Blob` | `ReadableStream<Uint8Array>` |
| `.slice(begin?, end?, type?)` | Web `Blob` | Returns a new lazy `BunFile` view — **no read** until consumed |
| `.formData()` | Web | Parse multipart/urlencoded |
| `.size` | Web `Blob` | Byte length (from `stat`; may be `Infinity`/unknown until read for pipes) |
| `.type` | Web `Blob` | MIME type (inferred from extension) |
| `.name` | Web `File` | File name/path |
| `.lastModified` | Web `File` | mtime (ms) |

Bun-specific additions on top of the `Blob` surface:

| Member | Signature | Purpose |
| --- | --- | --- |
| `.exists()` | `() => Promise<boolean>` | True if the file exists & is readable |
| `.stat()` | `() => Promise<fs.Stats>` | Node-style `fs.Stats` |
| `.write(data, opts?)` | `(data) => Promise<number>` | Overwrite the file's contents |
| `.writer(opts?)` | `({highWaterMark?}) => FileSink` | Incremental streaming writer |
| `.delete()` / `.unlink()` | `() => Promise<void>` | Remove the file |
| `.exists`, `.isFile()` etc. | | small helpers |

Because `slice()` returns another lazy `BunFile`, you can do
`Bun.file("big.bin").slice(0, 1024)` and only the first 1 KiB is ever read.

### 2.3 `Bun.write(...)` — the universal writer

```ts
Bun.write(
  destination: string | URL | BunFile | number /*fd*/,
  input: string | Blob | BunFile | ArrayBuffer | TypedArray | Response | Array<...>,
  options?: { mode?: number },
): Promise<number>   // bytes written
```

Highlights:

- Accepts a huge variety of inputs, including a `Response` (so you can stream a
  download straight to disk) and another `BunFile`.
- When both source and destination are files, Bun uses OS fast paths
  (`copy_file_range` on Linux, `clonefile`/`fcopyfile` on macOS, `sendfile`),
  internally referred to as the `CopyFileBlob` optimization — no userspace copy.
- Creates the file if missing, truncates by default.

### 2.4 `FileSink` — incremental writes

```ts
const sink = Bun.file("out.log").writer({ highWaterMark: 1024 * 1024 });
sink.write("chunk ");
sink.write(new Uint8Array([1, 2, 3]));
await sink.flush();   // flush buffered bytes to disk
// ...later
await sink.end();     // flush + close
sink.ref(); sink.unref(); // keep process alive or not
```

`FileSink` is a fast buffered writer (same interface as `ArrayBufferSink`) for
files and pipes — ideal for logs or long-lived streaming output.

---

## 3. Is there an equivalent in Deno?

**No single equivalent object.** Deno deliberately splits these responsibilities
between dedicated `Deno.*` functions and the Web Platform globals. There is no
lazy `Blob`-backed file handle. Here is the capability-by-capability mapping
(verified against this repo: namespace in `runtime/js/90_deno_ns.js`, `FsFile`
class in `ext/fs/30_fs.js`).

| Bun | Deno equivalent today |
| --- | --- |
| `Bun.file(path)` | *(no direct equivalent)* — you choose a reader below |
| `await Bun.file(p).text()` | `await Deno.readTextFile(p)` |
| `await Bun.file(p).json()` | `JSON.parse(await Deno.readTextFile(p))` |
| `await Bun.file(p).arrayBuffer()` | `(await Deno.readFile(p)).buffer` |
| `await Bun.file(p).bytes()` | `await Deno.readFile(p)` (already a `Uint8Array`) |
| `Bun.file(p).stream()` | `(await Deno.open(p)).readable` |
| `Bun.file(p).slice(0, n)` | `Deno.open` + `seek` + `read`, or `(await Deno.readFile(p)).subarray(0, n)` |
| `Bun.file(p).size` | `(await Deno.stat(p)).size` |
| `Bun.file(p).type` | *(none — infer yourself, e.g. via `@std/media-types`)* |
| `Bun.file(p).lastModified` | `(await Deno.stat(p)).mtime?.getTime()` |
| `await Bun.file(p).exists()` | `await Deno.stat(p).then(() => true, () => false)`, or `@std/fs` `exists()` |
| `await Bun.file(p).stat()` | `await Deno.stat(p)` (Deno's `FileInfo`, not Node `Stats`) |
| `await Bun.file(p).write(data)` | `await Deno.writeFile(p, data)` / `Deno.writeTextFile(p, str)` |
| `Bun.write(dest, src)` | `Deno.writeFile` / `Deno.writeTextFile`; copy file→file via `Deno.copyFile` |
| `Bun.write(dest, response)` | `(await Deno.open(dest,{write:true,create:true})).writable` + `response.body.pipeTo(...)` |
| `Bun.file(p).writer()` (FileSink) | `(await Deno.open(p,{write:true,create:true})).writable.getWriter()` |
| `await Bun.file(p).delete()` | `await Deno.remove(p)` |
| `Bun.stdin/stdout/stderr` | `Deno.stdin/stdout/stderr` (with `.readable`/`.writable`) |

### 3.1 What Deno's `Deno.FsFile` already gives you

From `ext/fs/30_fs.js`, an open `Deno.FsFile` is feature-rich and in several
respects *more* capable than a `BunFile` for low-level work:

- `read`/`readSync`, `write`/`writeSync`, `seek`/`seekSync`,
  `truncate`/`truncateSync`
- `stat`/`statSync`, `sync`/`syncData` (fsync/fdatasync), `utime`
- `get readable` / `get writable` (Web Streams, with a 64 KiB write buffer)
- `lock`/`tryLock`/`unlock` (advisory file locks) — **Bun has no public file
  locking API**
- `setRaw`, `isTerminal` for TTY handling

So Deno's gap isn't capability — it's the **lazy, ergonomic, `Blob`-shaped front
door**. `Deno.FsFile` is an *open* resource (it holds a file descriptor and must
be closed); `Bun.file` is a *lazy reference* (holds nothing, opens on demand).
These are complementary, not the same thing.

### 3.2 The closest "do it with Web APIs" recipe

You can hand-roll a passable `Bun.file` clone in user space today:

```ts
// A minimal lazy file → Blob, using only Deno + Web APIs.
async function denoFile(path: string | URL): Promise<Blob> {
  const data = await Deno.readFile(path);                 // eager read
  const type = ""; // infer from extension via @std/media-types if desired
  return new File([data], String(path), { type });
}

// Stream form (truly lazy, no full read):
function denoFileStream(path: string | URL): ReadableStream<Uint8Array> {
  return new ReadableStream({
    async start(c) {
      using f = await Deno.open(path);
      await f.readable.pipeTo(new WritableStream({ write: (chunk) => c.enqueue(chunk) }));
      c.close();
    },
  });
}
```

The honest limitation: a real `File` constructed from `Deno.readFile` is **not
lazy** — it has already read the whole file. To get Bun's laziness you must keep
a custom object whose `.text()/.bytes()/.stream()` defer to `Deno.open`. That is
exactly the boilerplate a built-in would remove.

---

## 4. Would `Deno.file` be a good addition?

**Yes — with caveats.** Arguments for and against:

### 4.1 In favor

1. **Ergonomics & migration.** `Bun.file().text()/.json()` is one of the most
   loved Bun ergonomics. A matching API lowers the friction of porting Bun code
   to Deno and reduces "why is this 3 lines in Deno" complaints.
2. **Web-standards alignment.** A file-as-`Blob`/`File` plugs directly into
   `fetch`, `FormData`, `Response`, `URL.createObjectURL`, structured clone,
   `postMessage`, etc. Today users must manually `new File([await Deno.readFile])`.
3. **Lazy is genuinely better for common patterns.** "Reference a file, maybe
   read part of it, maybe pass it to `fetch`" is extremely common in HTTP
   servers; laziness avoids reading files you never send.
4. **It composes with Deno's strengths** — permissions, `URL` support, Web
   Streams, `using` disposal — in ways Bun's version does not.

### 4.2 Against / risks

1. **API-surface philosophy.** Deno has historically preferred a small,
   Web-first `Deno.*` namespace and pushed conveniences into `@std`. A new
   namespace primitive needs a strong justification and a stable design.
2. **Lazy `Blob` semantics are subtle.** Spec `Blob`/`File` snapshot their data
   at creation time; a file on disk can change or disappear. Bun's lazy `BunFile`
   technically violates `File`'s "snapshot state" expectation (size/contents can
   change between reads). Deno would need to decide how spec-faithful to be (see
   §6).
3. **Permission model.** Reads must respect `--allow-read`; the *lazy* nature
   means the permission check can't happen at construction — only at first I/O.
   That's defensible but must be designed deliberately and documented.
4. **Error timing.** With laziness, "file not found" surfaces at `.text()` time,
   not at `Deno.file()` time. This can surprise users (Bun has the same trait).

### 4.3 Recommendation

Add it, but **lead with Web standards** rather than copying Bun 1:1:

- Prefer returning something that **is a `File`/`Blob`** (or a thin lazy
  subclass) so it drops into `fetch`/`FormData` with zero glue.
- Consider naming/shape that signals laziness, e.g. `Deno.openLazy()` or a
  `Deno.file()` that returns a `Deno.FsFileBlob` documented as lazy.
- Reuse the existing `Deno.FsFile` plumbing for the actual I/O.
- Make permissions and error-timing semantics explicit and tested.

A pragmatic first step that avoids namespace risk: ship it in **`@std/fs`** as
`file()` / `lazyFile()` first, gather usage, then graduate to `Deno.file()` if
warranted.

---

## 5. How Deno could make it *more* useful than Bun

Concrete ways to leapfrog Bun's version:

1. **True `File`/`Blob` subclass.** Return an actual `instanceof Blob`/`File`
   (Bun's is `Blob`-like but historically had rough edges). Guarantees seamless
   `fetch(url, { body: file })`, `formData.append("f", file)`,
   `new Response(file)`, structured clone over `postMessage`.

2. **First-class `URL` + remote sources, uniformly.** Bun bolted on `s3://`.
   Deno could accept `file:`, `http(s):`, `data:`, and `blob:` URLs behind one
   `Deno.file(url)` that returns the same lazy interface — backed by `fetch` for
   remote, `Deno.open` for local — so a single abstraction spans local and
   remote without an S3-specific subclass.

3. **Permission-aware and safe by default.** Integrate with `--allow-read`/
   `--allow-write` and report a clear `PermissionDenied` at first access. Offer a
   `Deno.file(path, { mode: "read" })`-style hint so the runtime can pre-flight
   the permission prompt. This is something Bun simply does not have.

4. **`using` / explicit-resource-management throughout.** Because Deno already
   ships `using`/`[Symbol.asyncDispose]` on `FsFile`, the lazy file's
   `.stream()`/`.writer()` can be disposed deterministically:
   `await using sink = Deno.file("out").writer();`.

5. **`AsyncIterable<Uint8Array>`** on the file itself, so
   `for await (const chunk of Deno.file(p)) { ... }` works without calling
   `.stream()` — nicer than Bun.

6. **Atomic writes as an option.** `Deno.file(p).write(data, { atomic: true })`
   (write-to-temp + rename) — a frequently hand-rolled pattern that Bun doesn't
   provide.

7. **`FileSystemFileHandle` alignment.** The W3C File System Access API defines
   `FileSystemFileHandle.getFile()` → `File` and
   `.createWritable()` → `FileSystemWritableFileStream`. Aligning a `Deno.file`
   with that vocabulary (or even exposing `FileSystemFileHandle` for local paths)
   would be *more* standards-forward than Bun and reuse knowledge developers
   already have from browsers.

8. **Cheap `slice()` that stays lazy** and maps to `seek`+bounded reads, plus a
   `.bytes({ start, end })` range read — so partial reads of huge files never
   allocate the whole buffer (Bun's `.slice()` is lazy but the partial-range
   ergonomics can be richer).

9. **Built-in content-type inference via `@std/media-types`** for `.type`, kept
   consistent with how Deno's file server already guesses MIME types.

10. **Fast file→file copy** (`copy_file_range`/`clonefile`) exposed through the
    same write path, matching Bun's `CopyFileBlob` optimization while reusing
    Deno's existing `Deno.copyFile` op.

---

## 6. Open design questions

- **Spec fidelity of laziness.** Should `Deno.file()` be a real `File` (snapshot
  semantics, but then it must read eagerly) or a lazy subclass (ergonomic, but
  `size`/contents can change between reads, mildly violating `File`)? Bun chose
  lazy. A compromise: lazy `size` resolved from `stat`, contents read on demand,
  documented clearly that it's a *live* reference.
- **When does the permission check fire?** At construction (eager, predictable)
  or at first I/O (lazy, matches Bun)? Likely first-I/O, but surface a way to
  pre-check.
- **Namespace vs. `@std`.** Stabilizing a new `Deno.*` primitive is a long-term
  commitment; `@std/fs` is a lower-risk proving ground.
- **Naming.** `Deno.file` collides conceptually with the global `File`. Consider
  `Deno.openLazy`, `Deno.fileRef`, or returning from `@std/fs`'s `file()`.

---

## 7. Conclusion

`Bun.file` is popular because it unifies "reference a file" + "read it however I
want" + "it's already a `Blob`" into one lazy object, with `Bun.write` as the
mirror-image writer. Deno can already *do* every task — via `Deno.readFile`/
`writeFile`, `Deno.open`/`FsFile`, `Deno.stat`, and Web globals — but with more
boilerplate and no lazy `Blob` handle.

Adding such a handle would be a real ergonomic win and a natural fit for Deno's
Web-standards posture. The opportunity is not to clone Bun but to do it *more*
correctly and more capably: a true lazy `File` subclass, uniform local+remote
URL support, permission-awareness, `using`-based disposal, `AsyncIterable`,
atomic writes, and `FileSystemFileHandle` alignment. Recommended path: prototype
in `@std/fs`, validate ergonomics and semantics (especially permission/error
timing and spec-fidelity of laziness), then consider promotion to `Deno.file`.

---

## Sources

- Bun — File I/O guide: https://bun.com/docs/runtime/file-io
- Bun — `Bun.file` reference: https://bun.com/reference/bun/file
- Bun — `BunFile` interface: https://bun.com/reference/bun/BunFile
- Bun — `Bun.write` reference: https://bun.com/reference/bun/write
- Bun — `FileSink` interface: https://bun.com/reference/bun/FileSink
- Bun — `BunFile.slice`: https://bun.com/reference/bun/BunFile/slice
- Bun — incremental write (FileSink) guide: https://bun.sh/guides/write-file/filesink
- DeepWiki — Bun File and Blob APIs: https://deepwiki.com/oven-sh/bun/9.2-file-and-blob-apis
- Deno — File System API: https://docs.deno.com/api/deno/file-system
- Deno — Reading files example: https://docs.deno.com/examples/reading_files/
- MDN — `Blob`: https://developer.mozilla.org/en-US/docs/Web/API/Blob
- MDN — `File`: https://developer.mozilla.org/en-US/docs/Web/API/File
- W3C — File System Access (`FileSystemFileHandle`): https://wicg.github.io/file-system-access/
- This repo: `runtime/js/90_deno_ns.js` (namespace), `ext/fs/30_fs.js` (`FsFile`)
