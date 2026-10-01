const fonts = await queryLocalFonts();
if (!Array.isArray(fonts)) throw new Error("expected array");
if (fonts.length === 0) throw new Error("expected at least one font");

const font = fonts[0];
if (typeof font.postscriptName !== "string") {
  throw new Error("bad postscriptName");
}
if (typeof font.fullName !== "string") throw new Error("bad fullName");
if (typeof font.family !== "string") throw new Error("bad family");
if (typeof font.style !== "string") throw new Error("bad style");

// Check sorted by postscriptName
for (let i = 1; i < fonts.length; i++) {
  if (fonts[i].postscriptName < fonts[i - 1].postscriptName) {
    throw new Error("not sorted");
  }
}

console.log("ok");
