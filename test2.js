const inputBlob = new Blob([await Deno.readFile("./input.png")], {
  type: "image/png",
});
const bitmap = await createImageBitmap(inputBlob);
const canvas = new OffscreenCanvas(200, 200);
const bitmaprenderer = canvas.getContext("bitmaprenderer");
bitmaprenderer.transferFromImageBitmap(bitmap);
const outputBlob = await canvas.convertToBlob();
await Deno.writeFile("./output.png", outputBlob.stream());
