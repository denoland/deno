import mqtt from "npm:mqtt";

const client = mqtt.connect({
  port: 8883,
  host: "host",
  key: Deno.readTextFileSync("keyfile"),
  cert: Deno.readTextFileSync("clientfile"),
  rejectUnauthorized: false,
  ca: Deno.readTextFileSync("cacertfile"),
  protocol: "mqtts",
});

client.on("connect", () => {
  client.subscribe("topic");
});

client.on("message", (topic, paylo) => {
  console.log([topic, new TextDecoder().decode(payload)]);
});

setInterval(async () => {
  await client.publish("topic", "payload");
}, 2000);
