import { Resvg } from "@resvg/resvg-js";
import fs from "node:fs";
import path from "node:path";

const PART_GROUPS = {
  body: ["layer-body", "layer-flag", "layer-outline"],
  eyes: ["layer-eyes"],
  eyebrows: ["layer-eyebrows"],
  mouth: ["layer-mouth"],
  accessories: ["layer-accessories"],
  effects: ["layer-effects"],
};

const ALL_LAYER_IDS = Object.values(PART_GROUPS).flat();

function hideLayers(svgText, visibleLayerIds) {
  let result = svgText;
  for (const layerId of ALL_LAYER_IDS) {
    const regex = new RegExp(`(<g\\s+id="${layerId}")`, "g");
    if (visibleLayerIds.includes(layerId)) {
      result = result.replace(regex, `$1 style="display:inline"`);
    } else {
      result = result.replace(regex, `$1 style="display:none"`);
    }
  }
  return result;
}

function renderSvgToPng(svgText, width = 256) {
  const resvg = new Resvg(svgText, {
    fitTo: { mode: "width", value: width },
    background: "rgba(0, 0, 0, 0)",
  });
  const pngData = resvg.render();
  return pngData.asPng();
}

function renderComposite(svgPath, outputDir) {
  const svgText = fs.readFileSync(svgPath, "utf-8");
  const pngBuffer = renderSvgToPng(svgText);

  const baseName = path.basename(svgPath, ".svg");
  const outPath = path.join(outputDir, `${baseName}.png`);
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, pngBuffer);
  console.log(`[composite] ${outPath}`);
  return outPath;
}

function renderParts(svgPath, outputDir) {
  const svgText = fs.readFileSync(svgPath, "utf-8");
  const baseName = path.basename(svgPath, ".svg");
  const outputs = [];

  for (const [partName, layerIds] of Object.entries(PART_GROUPS)) {
    const modified = hideLayers(svgText, layerIds);
    const pngBuffer = renderSvgToPng(modified);

    const outPath = path.join(outputDir, `${baseName}_${partName}.png`);
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, pngBuffer);
    console.log(`[part] ${outPath}`);
    outputs.push(outPath);
  }

  return outputs;
}

function renderAll(svgPath, compositeDir, partsDir) {
  renderComposite(svgPath, compositeDir);
  renderParts(svgPath, partsDir);
}

// --- CLI ---
const args = process.argv.slice(2);
const partsMode = args.includes("--parts");
const allMode = args.includes("--all");
const svgFiles = args.filter((a) => !a.startsWith("--"));

if (svgFiles.length === 0) {
  console.error("Usage:");
  console.error("  npm run render -- <svg-file>           # composite render");
  console.error("  npm run render:parts -- <svg-file>     # parts render");
  console.error("  npm run render:all -- <svg-file>       # both modes");
  process.exit(1);
}

const scriptDir = path.dirname(new URL(import.meta.url).pathname);
const compositeDir = path.join(scriptDir, "output", "composite");
const partsDir = path.join(scriptDir, "output", "parts");

for (const svgFile of svgFiles) {
  const svgPath = path.resolve(svgFile);

  if (!fs.existsSync(svgPath)) {
    console.error(`File not found: ${svgPath}`);
    process.exit(1);
  }

  if (allMode) {
    renderAll(svgPath, compositeDir, partsDir);
  } else if (partsMode) {
    renderParts(svgPath, partsDir);
  } else {
    renderComposite(svgPath, compositeDir);
  }
}
