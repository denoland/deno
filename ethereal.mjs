import { SMTPClient } from "https://deno.land/x/denomailer@1.0.1/mod.ts";

const auth = {
  username: "renee.daugherty6@ethereal.email",
  password: "dUTtJR2qD1rFG5BdUr"
};

const client = new SMTPClient({
  connection: {
    hostname: "smtp.ethereal.email",
    port: 587,
    auth,
  },
});

const s = await client.send({
  from: auth.username,
  to: auth.username,
  subject: "Welcome!",
  content: "Hi from Vuelancer!",
});

console.log(s)
