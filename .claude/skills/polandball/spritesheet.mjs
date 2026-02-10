import sharp from "sharp";
import fs from "node:fs";
import path from "node:path";

const FRAME_SIZE = 256;

const scriptDir = path.dirname(new URL(import.meta.url).pathname);
const partsDir = path.join(scriptDir, "output", "parts");
const sheetsDir = path.join(scriptDir, "output", "sheets");

function discoverAnimations(characterName) {
  if (!fs.existsSync(partsDir)) {
    console.error(`Parts directory not found: ${partsDir}`);
    console.error("Run 'npm run render:parts' first to generate part PNGs.");
    process.exit(1);
  }

  const files = fs.readdirSync(partsDir).filter((f) => f.endsWith(".png"));
  const pattern = new RegExp(
    `^${characterName}_([a-z]+)_(\\d+)_([a-z]+)\\.png$`
  );

  // Map: partName -> animName -> frame numbers
  const partAnimFrames = {};

  for (const file of files) {
    const match = file.match(pattern);
    if (!match) continue;

    const [, animName, frameStr, partName] = match;
    const frameNum = parseInt(frameStr, 10);

    if (!partAnimFrames[partName]) partAnimFrames[partName] = {};
    if (!partAnimFrames[partName][animName])
      partAnimFrames[partName][animName] = [];
    partAnimFrames[partName][animName].push(frameNum);
  }

  // Sort frame numbers
  for (const part of Object.values(partAnimFrames)) {
    for (const anim of Object.keys(part)) {
      part[anim].sort((a, b) => a - b);
    }
  }

  return partAnimFrames;
}

async function buildSpriteSheet(characterName, partName, animations) {
  const animNames = Object.keys(animations).sort();
  const maxFrames = Math.max(...animNames.map((a) => animations[a].length));
  const rows = animNames.length;

  const sheetWidth = maxFrames * FRAME_SIZE;
  const sheetHeight = rows * FRAME_SIZE;

  const composites = [];

  for (let row = 0; row < animNames.length; row++) {
    const animName = animNames[row];
    const frames = animations[animName];

    for (let col = 0; col < frames.length; col++) {
      const frameNum = String(frames[col]).padStart(2, "0");
      const filePath = path.join(
        partsDir,
        `${characterName}_${animName}_${frameNum}_${partName}.png`
      );

      if (!fs.existsSync(filePath)) {
        console.warn(`Missing frame: ${filePath}`);
        continue;
      }

      composites.push({
        input: filePath,
        left: col * FRAME_SIZE,
        top: row * FRAME_SIZE,
      });
    }
  }

  if (composites.length === 0) {
    console.warn(`No frames found for ${characterName}/${partName}, skipping.`);
    return null;
  }

  fs.mkdirSync(sheetsDir, { recursive: true });

  const sheetPath = path.join(sheetsDir, `${characterName}_${partName}.png`);
  await sharp({
    create: {
      width: sheetWidth,
      height: sheetHeight,
      channels: 4,
      background: { r: 0, g: 0, b: 0, alpha: 0 },
    },
  })
    .composite(composites)
    .png()
    .toFile(sheetPath);

  console.log(`[sheet] ${sheetPath} (${maxFrames}x${rows} frames)`);

  // Generate manifest
  const manifest = {
    character: characterName,
    part: partName,
    frameSize: FRAME_SIZE,
    animations: {},
  };

  for (let row = 0; row < animNames.length; row++) {
    const animName = animNames[row];
    manifest.animations[animName] = {
      row,
      frameCount: animations[animName].length,
      fps: 8,
    };
  }

  const manifestPath = path.join(
    sheetsDir,
    `${characterName}_${partName}.json`
  );
  fs.writeFileSync(manifestPath, JSON.stringify(manifest, null, 2));
  console.log(`[manifest] ${manifestPath}`);

  return { sheetPath, manifestPath };
}

// --- CLI ---
const args = process.argv.slice(2);

if (args.length === 0) {
  console.error("Usage: npm run sheet -- <character-name>");
  console.error("Example: npm run sheet -- poland");
  process.exit(1);
}

const characterName = args[0];
const partAnimFrames = discoverAnimations(characterName);

if (Object.keys(partAnimFrames).length === 0) {
  console.error(`No animation frames found for character "${characterName}".`);
  console.error(
    `Expected files matching: ${characterName}_<anim>_<frame>_<part>.png`
  );
  console.error(`in directory: ${partsDir}`);
  process.exit(1);
}

console.log(
  `Building sprite sheets for "${characterName}" with parts: ${Object.keys(partAnimFrames).join(", ")}`
);

for (const [partName, animations] of Object.entries(partAnimFrames)) {
  await buildSpriteSheet(characterName, partName, animations);
}

console.log("Done!");
