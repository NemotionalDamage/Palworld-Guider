import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";

const SOURCE_ID = "SRC-PALWORLD-GG-BREEDING-1_0-20260911";
const SOURCE_ID_PREFIX = "SRC-PALWORLD-GG-BREEDING";
const APPLICABLE_GAME_VERSION = "1.0";
const SOURCE_URL = "https://palworld.gg/breeding-calculator";
const ENGLISH_DATA_URL = "https://palworld.gg/_nuxt/BXj4Ki76.js";
const CHINESE_DATA_URL = "https://palworld.gg/_nuxt/CyxJ3Vfp.js";

function parseArguments(argv) {
  const argumentsByName = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    argumentsByName.set(argv[index], argv[index + 1]);
  }
  const root = argumentsByName.get("--root") ?? process.cwd();
  return {
    root,
    englishInput: argumentsByName.get("--english-input") ?? path.join(root, ".local", "palworldgg-pals-en.js"),
    chineseInput: argumentsByName.get("--chinese-input") ?? path.join(root, ".local", "palworldgg-pals-zh.js"),
  };
}

function breedingRuleId(parentA, parentB, child) {
  return `BREED_${parentA}_${parentB}_${child}`.replaceAll("PAL_", "");
}

function breedingProvenance() {
  return {
    source_id: SOURCE_ID,
    applicable_game_version: APPLICABLE_GAME_VERSION,
    retrieved_on: "2026-09-11",
    reviewer: "Codex",
    review_status: "reviewed",
    confidence: "reviewed_secondary",
    change_risk: "Breeding ranks and explicit combinations can change in game patches.",
    corroborating_source_ids: [],
  };
}

function setBreedingFields(record, webPal) {
  record.breeding_combi_rank = webPal.combiRank;
  record.breeding_combi_priority = webPal.combiPriority;
  if (webPal.ignoreCombi) {
    record.breeding_ignore_combi = true;
    if (webPal.combos.length === 0) {
      record.breeding_self_only = true;
    }
  } else {
    delete record.breeding_ignore_combi;
    delete record.breeding_self_only;
  }
}

async function loadWebPals(filePath) {
  const module = await import(pathToFileURL(path.resolve(filePath)).href);
  return new Map(Object.values(module.default).map((pal) => [pal.id, pal]));
}

async function readJsonLines(filePath) {
  return (await readFile(filePath, "utf8"))
    .trim()
    .split(/\r?\n/)
    .map((line) => JSON.parse(line));
}

async function writeJsonLines(filePath, records) {
  await writeFile(filePath, `${records.map((record) => JSON.stringify(record)).join("\n")}\n`, "utf8");
}

const { root, englishInput, chineseInput } = parseArguments(process.argv.slice(2));
const englishPals = await loadWebPals(englishInput);
const chinesePals = await loadWebPals(chineseInput);
if (englishPals.size !== chinesePals.size) {
  throw new Error(`English and Chinese Pal datasets have different counts: ${englishPals.size} vs ${chinesePals.size}`);
}

const reviewedPath = path.join(root, "data", "reviewed");
const palRecords = await readJsonLines(path.join(reviewedPath, "pals.jsonl"));
const factRecords = await readJsonLines(path.join(reviewedPath, "facts.jsonl"));
const sourceRecords = (await readJsonLines(path.join(reviewedPath, "sources.jsonl"))).filter(
  (record) => !record.id?.startsWith(SOURCE_ID_PREFIX),
);
const knownPalRecords = [...palRecords, ...factRecords].filter(
  (record) => record.record_type === "pal",
);
const palByNativeId = new Map(
  knownPalRecords.map((record) => [record.native_row_id?.toLowerCase(), record]),
);
const repositoryIdByWebId = new Map();
const skippedWebPals = [];

for (const [webPalId, englishPal] of englishPals) {
  const record = palByNativeId.get(englishPal.key.toLowerCase());
  if (!record) {
    skippedWebPals.push(`${englishPal.name} (${englishPal.key})`);
    continue;
  }
  repositoryIdByWebId.set(webPalId, record.id);
  setBreedingFields(record, englishPal);
  const corroboratingSourceIds = new Set(
    (record.provenance.corroborating_source_ids ?? []).filter(
      (sourceId) => !sourceId.startsWith(SOURCE_ID_PREFIX),
    ),
  );
  corroboratingSourceIds.add(SOURCE_ID);
  record.provenance.corroborating_source_ids = [...corroboratingSourceIds].sort();
}

const explicitRules = [];
const skippedCombinations = [];
const uniqueCombinations = new Map();
for (const englishPal of englishPals.values()) {
  for (const combination of englishPal.combos) {
    const combinationKey = [...[combination.a, combination.b]].sort().join("|") + ">" + combination.child;
    if (!uniqueCombinations.has(combinationKey)) {
      uniqueCombinations.set(combinationKey, combination);
    }
  }
}
for (const combination of uniqueCombinations.values()) {
  const parentAId = repositoryIdByWebId.get(combination.a);
  const parentBId = repositoryIdByWebId.get(combination.b);
  const childId = repositoryIdByWebId.get(combination.child);
  if (!parentAId || !parentBId || !childId) {
    skippedCombinations.push(JSON.stringify(combination));
    continue;
  }
  explicitRules.push({
    record_type: "breeding_rule",
    id: breedingRuleId(parentAId, parentBId, childId),
    parent_a_id: parentAId,
    parent_b_id: parentBId,
    child_id: childId,
    notes: "Explicit Palworld.gg combination; checked before the combi-rank formula.",
    provenance: breedingProvenance(),
  });
}
explicitRules.sort((left, right) => left.id.localeCompare(right.id));

sourceRecords.push({
  record_type: "source",
  id: SOURCE_ID,
  title: "Palworld.gg breeding calculator dataset",
  supplier: "Palworld.gg",
  retrieved_on: "2026-09-11",
  evidence_urls: [SOURCE_URL, ENGLISH_DATA_URL, CHINESE_DATA_URL],
  applicable_game_version: APPLICABLE_GAME_VERSION,
  reviewer: "Codex",
  review_status: "reviewed",
  confidence: "reviewed_secondary",
  notes: "Extracted per-Pal combiRank, combiPriority, ignoreCombi, and explicit combinations from the calculator's English and Chinese data modules. The site publishes no game-version marker, so the reviewer aligned this source with the 1.0 target build; ranks can drift if the live version moves.",
});

await writeJsonLines(path.join(reviewedPath, "pals.jsonl"), palRecords);
await writeJsonLines(path.join(reviewedPath, "facts.jsonl"), factRecords);
await writeJsonLines(path.join(reviewedPath, "breeding_rules.jsonl"), explicitRules);
await writeJsonLines(path.join(reviewedPath, "sources.jsonl"), sourceRecords);

console.log(`Breeding data written: ${repositoryIdByWebId.size} matched Pals, ${explicitRules.length} explicit combinations.`);
if (skippedWebPals.length > 0) {
  console.log(`Skipped ${skippedWebPals.length} Pals outside the canonical roster: ${skippedWebPals.join(", ")}`);
}
if (skippedCombinations.length > 0) {
  console.log(`Skipped ${skippedCombinations.length} combinations that reference those Pals.`);
}
