import inspector from "node:inspector";

const session = new inspector.Session();
session.connect();
session.on("inspectorNotification", (ev) => {
  console.log(ev);
});
session.post("Runtime.enable", undefined, (err, params) => {
  console.log(err, params);
});
session.post(
  "Runtime.evaluate",
  { executionContextId: 1, expression: "1 + 2" },
  (err, params) => {
    console.log(err, params);
  },
);
