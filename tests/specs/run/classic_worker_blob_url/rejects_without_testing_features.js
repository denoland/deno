const code = `
self.onmessage = function(e) {
  postMessage({ result: e.data });
};
`;
const blob = new Blob([code], { type: "text/javascript" });
const url = URL.createObjectURL(blob);

try {
  new Worker(url);
  console.log("unexpected success");
} catch (error) {
  console.log(error.name);
  console.log(error.message);
}
