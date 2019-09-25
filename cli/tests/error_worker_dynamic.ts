const b = new Blob(['throw new Error("hello");']);
const blobURL = URL.createObjectURL(b);
new Worker(blobURL);
