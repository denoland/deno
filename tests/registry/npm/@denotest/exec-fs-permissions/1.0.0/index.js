module.exports.getFileMode = function() {
  return Deno.lstatSync(__dirname + "/exec").mode;
};
