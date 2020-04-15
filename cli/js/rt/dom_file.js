System.register("$deno$/web/dom_file.ts", ["$deno$/web/blob.ts"], function (
  exports_88,
  context_88
) {
  "use strict";
  let blob, DomFileImpl;
  const __moduleName = context_88 && context_88.id;
  return {
    setters: [
      function (blob_1) {
        blob = blob_1;
      },
    ],
    execute: function () {
      DomFileImpl = class DomFileImpl extends blob.DenoBlob {
        constructor(fileBits, fileName, options) {
          const { lastModified = Date.now(), ...blobPropertyBag } =
            options ?? {};
          super(fileBits, blobPropertyBag);
          // 4.1.2.1 Replace any "/" character (U+002F SOLIDUS)
          // with a ":" (U + 003A COLON)
          this.name = String(fileName).replace(/\u002F/g, "\u003A");
          // 4.1.3.3 If lastModified is not provided, set lastModified to the current
          // date and time represented in number of milliseconds since the Unix Epoch.
          this.lastModified = lastModified;
        }
      };
      exports_88("DomFileImpl", DomFileImpl);
    },
  };
});
