import inspector from "node:inspector";

let resolve_;
const deferred = new Promise((resolve) => {
  resolve_ = resolve;
});

const session = new inspector.Session();
session.connect();
console.log("Connected");

session.on("inspectorNotification", (ev) => {
  console.log("Received notification", ev.method);
});

session.post("Runtime.enable", undefined, (err, params) => {
  console.log("Runtime.enable, err:", err, "params:", params);
});

session.post(
  "Runtime.evaluate",
  {
    contextId: 1,
    expression: "new Promise(resolve => setTimeout(resolve, 1000))",
    awaitPromise: true,
  },
  (err, params) => {
    console.log("Runtime.evaluate, err:", err, "params:", params);
    // resolve_();
  },
);

// await deferred;
session.disconnect();
session.disconnect();

session.connect();
console.log("Connected again");
session.post("Runtime.enable", undefined, (err, params) => {
  console.log("Runtime.enable, err:", err, "params:", params);
});

session.post(
  "Runtime.evaluate",
  {
    contextId: 1,
    expression: "1 + 2",
  },
  (err, params) => {
    console.log("Runtime.evaluate, err:", err, "params:", params);
  },
);
