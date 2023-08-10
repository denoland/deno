import { Sequelize } from "npm:sequelize";
// deno-types="npm:@types/pg@8.10.2"
import _pg from "npm:pg";

const sequelize = new Sequelize(
  "postgres://jcelrjot:cD8Lg2tMrk0h2ZdJuOgmIM1UWDP-7Kc5@trumpet.db.elephantsql.com/jcelrjot",
  {
    dialectOptions: {
      ssl: { require: true, rejectUnauthorized: false },
    },
  },
);

await sequelize.authenticate();
