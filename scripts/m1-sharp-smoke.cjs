"use strict";

// CI-only secondary AVIF feasibility baseline. Sharp's resize, gamma,
// chroma and encoder quality settings are NOT matched to Ferrixel.
const sharp = require("sharp");

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  process.stderr.write("usage: node m1-sharp-smoke.cjs input.jpg output.avif\n");
  process.exit(2);
}
sharp(input)
  .resize(640, 427, { fit: "fill", kernel: sharp.kernel.lanczos3 })
  .avif({ quality: 80, effort: 0 })
  .toFile(output)
  .then(() => {})
  .catch((error) => {
    process.stderr.write(String(error) + "\n");
    process.exitCode = 1;
  });
