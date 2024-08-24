import inspector from "node:inspector";

const session = new inspector.Session();
session.connect();

console.log("sending message");
session.on("inspectorNotification", (ev) => {
  console.log("Received notification", ev);
});

console.log("sending message");

session.post("Runtime.enable", undefined, (err, params) => {
  console.log("Runtime.enable, err:", err, "params:", params);
});

console.log("sending message2");
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
