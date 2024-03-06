import { Client } from "npm:@temporalio/client";

const client = new Client();
const result = await client.workflow.execute<(arg: string) => Promise<void>>(
  "",
  {
    taskQueue: "default",
    workflowId: "my-business-id",
    args: ["Temporal"],
  },
);
console.log(result); // Hello, Temporal!
