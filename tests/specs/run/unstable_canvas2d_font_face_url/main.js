// A relative url() resolves against `location`, so without --location fetch
// cannot build a Request. That must still surface as a NetworkError, not as
// the raw TypeError fetch throws.
const face = new FontFace("MyFont", 'url("./nope.otf") format("opentype")');
try {
  await face.load();
  console.log("unexpectedly loaded");
} catch (e) {
  console.log(e.constructor.name, e.name);
  console.log(face.status);
}
