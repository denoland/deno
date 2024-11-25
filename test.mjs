import { confirm } from "npm:@inquirer/prompts";

const answer = await confirm({ message: "Continue?" });
console.log(answer);
