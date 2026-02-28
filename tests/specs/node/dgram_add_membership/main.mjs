import * as dgram from "node:dgram";

const s = dgram.createSocket("udp4");
s.bind(0, () => {
  try {
    s.addMembership("224.0.0.114");
    console.log("success");
  } catch (e) {
    console.log("error:", e.message);
  } finally {
    s.close();
  }
});
