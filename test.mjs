import nodemailer from "npm:nodemailer";

nodemailer.createTestAccount((err, account) => {
  if (err) {
    console.error("Failed to create a testing account. " + err.message);
    return Deno.exit(1);
  }

  console.log("Credentials obtained, sending message...");

  // Create a SMTP transporter object
  let transporter = nodemailer.createTransport({
    host: account.smtp.host,
    port: account.smtp.port,
    secure: account.smtp.secure,
    auth: {
      user: account.user,
      pass: account.pass,
    },
    logger: true,
  });

  console.log("transporter", transporter);

  // Message object
  let message = {
    from: "Sender Name <sender@example.com>",
    to: "Recipient <recipient@example.com>",
    subject: "Nodemailer is unicode friendly ✔",
    text: "Hello to myself!",
    html: "<p><b>Hello</b> to myself!</p>",
  };
  console.log("before send");
  transporter.sendMail(message, (err, info) => {
    console.log("after send");
    if (err) {
      throw err;
    }

    console.log("Message sent: %s", info.messageId);
    // Preview only available when sending through an Ethereal account
    console.log("Preview URL: %s", nodemailer.getTestMessageUrl(info));
  });
});
