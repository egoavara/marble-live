import sharp from "sharp";
import pkg from "gifenc";
const { GIFEncoder, quantize, applyPalette } = pkg;
import fs from "node:fs";
import path from "node:path";

const CHECKER_LIGHT = 204; // #CCC
const CHECKER_DARK = 170; // #AAA
const CHECKER_TILE = 16; // 16px tiles

const scriptDir = path.dirname(new URL(import.meta.url).pathname);
const compositeDir = path.join(scriptDir, "output", "composite");
const previewDir = path.join(scriptDir, "output", "preview");

function createCheckerboard(size) {
  const buf = Buffer.alloc(size * size * 3);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const tile =
        (Math.floor(x / CHECKER_TILE) + Math.floor(y / CHECKER_TILE)) % 2;
      const val = tile === 0 ? CHECKER_LIGHT : CHECKER_DARK;
      const idx = (y * size + x) * 3;
      buf[idx] = val;
      buf[idx + 1] = val;
      buf[idx + 2] = val;
    }
  }
  return sharp(buf, { raw: { width: size, height: size, channels: 3 } })
    .png()
    .toBuffer();
}

function findFrames(characterName, animName) {
  if (!fs.existsSync(compositeDir)) {
    console.error(`Composite directory not found: ${compositeDir}`);
    console.error("Run 'npm run render' first to generate composite PNGs.");
    process.exit(1);
  }

  const pattern = new RegExp(
    `^${characterName}_${animName}_\\d+\\.png$`
  );
  const files = fs
    .readdirSync(compositeDir)
    .filter((f) => pattern.test(f))
    .sort();

  return files.map((f) => path.join(compositeDir, f));
}

async function createGifPreview(framePaths, outputPath, fps, size) {
  const gif = GIFEncoder();
  const checkerBg = await createCheckerboard(size);

  for (const framePath of framePaths) {
    // Composite frame onto checkerboard background
    const { data } = await sharp(checkerBg)
      .composite([{ input: await sharp(framePath).resize(size, size).toBuffer() }])
      .removeAlpha()
      .raw()
      .toBuffer({ resolveWithObject: true });

    // gifenc expects RGBA, pad RGB to RGBA
    const rgb = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    const rgba = new Uint8Array(size * size * 4);
    for (let i = 0; i < size * size; i++) {
      rgba[i * 4] = rgb[i * 3];
      rgba[i * 4 + 1] = rgb[i * 3 + 1];
      rgba[i * 4 + 2] = rgb[i * 3 + 2];
      rgba[i * 4 + 3] = 255;
    }

    const palette = quantize(rgba, 256);
    const index = applyPalette(rgba, palette);

    gif.writeFrame(index, size, size, {
      palette,
      delay: Math.round(1000 / fps),
    });
  }

  gif.finish();
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, Buffer.from(gif.bytes()));
  console.log(`[gif] ${outputPath} (${framePaths.length} frames, ${fps}fps)`);
}

async function createFilmstrip(framePaths, outputPath, size) {
  const count = framePaths.length;

  const resizedBuffers = await Promise.all(
    framePaths.map((p) => sharp(p).resize(size, size).png().toBuffer())
  );

  const composites = resizedBuffers.map((buf, i) => ({
    input: buf,
    left: i * size,
    top: 0,
  }));

  fs.mkdirSync(path.dirname(outputPath), { recursive: true });

  await sharp({
    create: {
      width: size * count,
      height: size,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite(composites)
    .png()
    .toFile(outputPath);

  console.log(`[filmstrip] ${outputPath} (${count} frames)`);
}

// --- CLI ---
const args = process.argv.slice(2);
const filmstripMode = args.includes("--filmstrip");

const fpsArg = args.find((a) => a.startsWith("--fps="));
const fps = fpsArg ? parseInt(fpsArg.split("=")[1], 10) : 8;

const sizeArg = args.find((a) => a.startsWith("--size="));
const size = sizeArg ? parseInt(sizeArg.split("=")[1], 10) : 256;

const positional = args.filter((a) => !a.startsWith("--"));

if (positional.length < 2) {
  console.error("Usage:");
  console.error(
    "  npm run preview -- <character> <animation>              # animated GIF"
  );
  console.error(
    "  npm run preview:filmstrip -- <character> <animation>    # filmstrip PNG"
  );
  console.error("");
  console.error("Options:");
  console.error("  --fps=N       Frame rate (default: 8)");
  console.error("  --size=N      Frame size in px (default: 256)");
  console.error("");
  console.error("Example:");
  console.error("  npm run preview -- poland idle");
  console.error("  npm run preview -- usa idle --fps=12");
  console.error("  npm run preview:filmstrip -- poland idle");
  process.exit(1);
}

const [characterName, animName] = positional;
const framePaths = findFrames(characterName, animName);

if (framePaths.length === 0) {
  console.error(
    `No frames found for "${characterName}_${animName}_*.png" in ${compositeDir}`
  );
  console.error(
    "Make sure you've rendered all frames first with 'npm run render'."
  );
  process.exit(1);
}

console.log(
  `Found ${framePaths.length} frames for ${characterName}/${animName}`
);

fs.mkdirSync(previewDir, { recursive: true });

if (filmstripMode) {
  const outPath = path.join(
    previewDir,
    `${characterName}_${animName}_filmstrip.png`
  );
  await createFilmstrip(framePaths, outPath, size);
} else {
  const outPath = path.join(
    previewDir,
    `${characterName}_${animName}.gif`
  );
  await createGifPreview(framePaths, outPath, fps, size);
}
