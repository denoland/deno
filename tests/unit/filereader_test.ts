// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(function fileReaderConstruct() {
  const fr = new FileReader();
  assertEquals(fr.readyState, FileReader.EMPTY);

  assertEquals(FileReader.EMPTY, 0);
  assertEquals(FileReader.LOADING, 1);
  assertEquals(FileReader.DONE, 2);
});

Deno.test(async function fileReaderLoadBlob() {
  await new Promise<void>((resolve) => {
    const fr = new FileReader();
    const b1 = new Blob(["Hello World"]);

    assertEquals(fr.readyState, FileReader.EMPTY);

    const hasOnEvents = {
      load: false,
      loadend: false,
      loadstart: false,
      progress: 0,
    };
    const hasDispatchedEvents = {
      load: false,
      loadend: false,
      loadstart: false,
      progress: 0,
    };
    let result: string | null = null;

    fr.addEventListener("load", () => {
      hasDispatchedEvents.load = true;
    });
    fr.addEventListener("loadend", () => {
      hasDispatchedEvents.loadend = true;
    });
    fr.addEventListener("loadstart", () => {
      hasDispatchedEvents.loadstart = true;
    });
    fr.addEventListener("progress", () => {
      hasDispatchedEvents.progress += 1;
    });

    fr.onloadstart = () => {
      hasOnEvents.loadstart = true;
    };
    fr.onprogress = () => {
      assertEquals(fr.readyState, FileReader.LOADING);

      hasOnEvents.progress += 1;
    };
    fr.onload = () => {
      hasOnEvents.load = true;
    };
    fr.onloadend = (ev) => {
      hasOnEvents.loadend = true;
      result = fr.result as string;

      assertEquals(hasOnEvents.loadstart, true);
      assertEquals(hasDispatchedEvents.loadstart, true);
      assertEquals(hasOnEvents.load, true);
      assertEquals(hasDispatchedEvents.load, true);
      assertEquals(hasOnEvents.loadend, true);
      assertEquals(hasDispatchedEvents.loadend, true);

      assertEquals(fr.readyState, FileReader.DONE);

      assertEquals(result, "Hello World");
      assertEquals(ev.lengthComputable, true);
      resolve();
    };

    fr.readAsText(b1);
  });
});

Deno.test(async function fileReaderLoadBlobDouble() {
  // impl note from https://w3c.github.io/FileAPI/
  // Event handler for the load or error events could have started another load,
  // if that happens the loadend event for the first load is not fired

  const fr = new FileReader();
  const b1 = new Blob(["First load"]);
  const b2 = new Blob(["Second load"]);

  await new Promise<void>((resolve) => {
    let result: string | null = null;

    fr.onload = () => {
      result = fr.result as string;
      assertEquals(result === "First load" || result === "Second load", true);

      if (result === "First load") {
        fr.readAsText(b2);
      }
    };
    fr.onloadend = () => {
      assertEquals(result, "Second load");

      resolve();
    };

    fr.readAsText(b1);
  });
});

Deno.test(async function fileReaderLoadBlobArrayBuffer() {
  await new Promise<void>((resolve) => {
    const fr = new FileReader();
    const b1 = new Blob(["Hello World"]);
    let result: ArrayBuffer | null = null;

    fr.onloadend = (ev) => {
      assertEquals(fr.result instanceof ArrayBuffer, true);
      result = fr.result as ArrayBuffer;

      const decoder = new TextDecoder();
      const text = decoder.decode(result);

      assertEquals(text, "Hello World");
      assertEquals(ev.lengthComputable, true);
      resolve();
    };

    fr.readAsArrayBuffer(b1);
  });
});

Deno.test(async function fileReaderLoadBlobDataUrl() {
  await new Promise<void>((resolve) => {
    const fr = new FileReader();
    const b1 = new Blob(["Hello World"]);
    let result: string | null = null;

    fr.onloadend = (ev) => {
      result = fr.result as string;
      assertEquals(
        result,
        "data:application/octet-stream;base64,SGVsbG8gV29ybGQ=",
      );
      assertEquals(ev.lengthComputable, true);
      resolve();
    };

    fr.readAsDataURL(b1);
  });
});

Deno.test(async function fileReaderLoadBlobAbort() {
  await new Promise<void>((resolve) => {
    const fr = new FileReader();
    const b1 = new Blob(["Hello World"]);

    const hasOnEvents = {
      load: false,
      loadend: false,
      abort: false,
    };

    fr.onload = () => {
      hasOnEvents.load = true;
    };
    fr.onloadend = (ev) => {
      hasOnEvents.loadend = true;

      assertEquals(hasOnEvents.load, false);
      assertEquals(hasOnEvents.loadend, true);
      assertEquals(hasOnEvents.abort, true);

      assertEquals(fr.readyState, FileReader.DONE);
      assertEquals(fr.result, null);
      assertEquals(ev.lengthComputable, false);
      resolve();
    };
    fr.onabort = () => {
      hasOnEvents.abort = true;
    };

    fr.readAsDataURL(b1);
    fr.abort();
  });
});

Deno.test(async function fileReaderLoadBlobAbort() {
  await new Promise<void>((resolve) => {
    const fr = new FileReader();
    const b1 = new Blob(["Hello World"]);

    const hasOnEvents = {
      load: false,
      loadend: false,
      abort: false,
    };

    fr.onload = () => {
      hasOnEvents.load = true;
    };
    fr.onloadend = (ev) => {
      hasOnEvents.loadend = true;

      assertEquals(hasOnEvents.load, false);
      assertEquals(hasOnEvents.loadend, true);
      assertEquals(hasOnEvents.abort, true);

      assertEquals(fr.readyState, FileReader.DONE);
      assertEquals(fr.result, null);
      assertEquals(ev.lengthComputable, false);
      resolve();
    };
    fr.onabort = () => {
      hasOnEvents.abort = true;
    };

    fr.readAsDataURL(b1);
    fr.abort();
  });
});

Deno.test(
  async function fileReaderDispatchesEventsInCorrectOrder() {
    await new Promise<void>((resolve) => {
      const fr = new FileReader();
      const b1 = new Blob(["Hello World"]);
      let out = "";
      fr.addEventListener("loadend", () => {
        out += "1";
      });
      fr.onloadend = (_ev) => {
        out += "2";
      };
      fr.addEventListener("loadend", () => {
        assertEquals(out, "12");
        resolve();
      });

      fr.readAsDataURL(b1);
    });
  },
);
