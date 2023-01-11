import pl from "npm:nodejs-polars";

function run() {
  const file = "./foods.csv";
  const df = pl.readCSV(file)
    .withColumns(
      pl.col("*"),
      pl.col("category").str.toUpperCase().cast(pl.Categorical),
      pl.col("calories").multiplyBy(2).cast(pl.Int32),
      pl.col("fats_g").str,
    )
    //   .filter(pl.col("calories").gt(100).over(pl.col("category")))

    .sort(pl.col("sugars_g"));
  console.log(df.toString());
}

run();
