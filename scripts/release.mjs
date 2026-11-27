#!/usr/bin/env node
// Prepares and verifies a release.
//
//   node scripts/release.mjs 1.0.1        bump version everywhere + CHANGELOG stub + build check
//   node scripts/release.mjs --check      verify versions/CHANGELOG/tag agree (also runs in CI)
//
// No dependencies on purpose, matching scripts/make-icons.mjs.

import { execSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const file = (name) => path.join(root, name);

const PACKAGE_JSON = "package.json";
const TAURI_CONF = "src-tauri/tauri.conf.json";
const CARGO_TOML = "src-tauri/Cargo.toml";
const CARGO_LOCK = "src-tauri/Cargo.lock";
const CHANGELOG = "CHANGELOG.md";

const read = (name) => readFileSync(file(name), "utf8");
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/;
// Windows CI can check files out with CRLF, so nothing here may assume LF.
const eolOf = (text) => (text.includes("\r\n") ? "\r\n" : "\n");

function fail(message) {
  console.error(`\x1b[31m✖\x1b[0m ${message}`);
  process.exit(1);
}

function versions() {
  const cargoToml = read(CARGO_TOML);
  const cargoLock = read(CARGO_LOCK);
  const lockBlock = cargoLock.match(/^name = "worktime"\r?\nversion = "([^"]+)"/m);
  return {
    [PACKAGE_JSON]: JSON.parse(read(PACKAGE_JSON)).version,
    [TAURI_CONF]: JSON.parse(read(TAURI_CONF)).version,
    [CARGO_TOML]: (cargoToml.match(/^version = "([^"]+)"/m) || [])[1],
    [CARGO_LOCK]: lockBlock ? lockBlock[1] : undefined,
  };
}

function changelogSection(markdown, version) {
  const lines = markdown.split(/\r?\n/);
  const start = lines.findIndex((line) => line.startsWith(`## [${version}]`));
  if (start === -1) return null;
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => line.startsWith("## ["));
  return (end === -1 ? rest : rest.slice(0, end)).join("\n").trim();
}

function compare(a, b) {
  const parse = (value) => value.split("-")[0].split(".").map(Number);
  const [left, right] = [parse(a), parse(b)];
  for (let i = 0; i < 3; i += 1) {
    if (left[i] !== right[i]) return left[i] - right[i];
  }
  return 0;
}

function check({ tagName, requireChangelog = true } = {}) {
  const found = versions();
  const problems = [];

  for (const [name, version] of Object.entries(found)) {
    if (!version) problems.push(`${name}: לא נמצא מספר גרסה`);
  }

  const unique = [...new Set(Object.values(found))];
  if (unique.length > 1) {
    const detail = Object.entries(found)
      .map(([name, version]) => `  ${name}: ${version}`)
      .join("\n");
    problems.push(`הגרסאות אינן זהות בין הקבצים:\n${detail}`);
  }

  const version = found[TAURI_CONF];
  if (version) {
    if (requireChangelog) {
      const section = changelogSection(read(CHANGELOG), version);
      if (section === null) {
        problems.push(`${CHANGELOG}: חסר קטע "## [${version}]" — הריצו npm run release:prepare`);
      } else if (/^-\s*TODO/m.test(section)) {
        problems.push(`${CHANGELOG}: הקטע של ${version} עדיין מכיל TODO — כתבו את השינויים`);
      } else if (!/^[-*] /m.test(section)) {
        problems.push(`${CHANGELOG}: הקטע של ${version} ריק — הוסיפו לפחות שורה אחת של שינויים`);
      }
    }

    if (tagName && tagName.startsWith("v") && tagName !== `v${version}`) {
      problems.push(`התג ${tagName} אינו תואם לגרסה ${version} — עדכנו את הגרסה או את התג`);
    }
  }

  if (problems.length) {
    fail(`בדיקת השחרור נכשלה:\n- ${problems.join("\n- ")}`);
  }
  console.log(`\x1b[32m✔\x1b[0m גרסה ${version} עקבית בכל הקבצים, והקטע ב-${CHANGELOG} מלא.`);
}

function bumpVersion(next, { dryRun, verify }) {
  const current = JSON.parse(read(TAURI_CONF)).version;
  if (!SEMVER.test(next)) fail(`"${next}" אינו מספר גרסה תקין (למשל 1.0.1)`);
  if (compare(next, current) <= 0) fail(`הגרסה ${next} אינה חדשה מהגרסה הנוכחית ${current}`);

  const edits = [];
  const replace = (name, pattern, replacement) => {
    const text = read(name);
    if (!pattern.test(text)) fail(`${name}: לא נמצאה שורת גרסה לעדכון`);
    edits.push([name, text.replace(pattern, replacement)]);
  };

  replace(PACKAGE_JSON, /("version"\s*:\s*")[^"]+(")/, `$1${next}$2`);
  replace(TAURI_CONF, /("version"\s*:\s*")[^"]+(")/, `$1${next}$2`);
  replace(CARGO_TOML, /^(version = ")[^"]+(")/m, `$1${next}$2`);
  replace(CARGO_LOCK, /(^name = "worktime"\r?\nversion = ")[^"]+(")/m, `$1${next}$2`);

  const changelog = read(CHANGELOG);
  const nl = eolOf(changelog);
  let updated = changelog;
  let sectionNote;
  if (changelogSection(changelog, next) === null) {
    const date = new Date().toISOString().slice(0, 10);
    const heading = `## [${next}] — ${date}`;
    const entry = [
      heading,
      "",
      "### נוסף",
      "- TODO: תארו את השינויים בגרסה הזו — הטקסט הזה הוא הערות הגרסה ב-GitHub ובתוך התוכנה.",
      "",
    ].join(nl);
    const lines = changelog.split(/\r?\n/);
    const firstSection = lines.findIndex((line) => line.startsWith("## ["));
    if (firstSection === -1) {
      updated = `${changelog.trimEnd()}${nl}${nl}${entry}`;
    } else {
      updated = [...lines.slice(0, firstSection), ...entry.split(nl), ...lines.slice(firstSection)].join(nl);
    }
    sectionNote = `${CHANGELOG}: נוסף קטע "${heading}"`;
  } else {
    sectionNote = `${CHANGELOG}: הקטע של ${next} כבר קיים — לא נכתב שוב`;
  }

  if (dryRun) {
    for (const [name] of edits) console.log(`(dry-run) יעודכן: ${name}`);
    console.log(`(dry-run) ${sectionNote}`);
    return;
  }

  for (const [name, text] of edits) writeFileSync(file(name), text, "utf8");
  if (updated !== changelog) writeFileSync(file(CHANGELOG), updated, "utf8");

  console.log(`\x1b[32m✔\x1b[0m הגרסה עודכנה ל-${next} ב-${edits.length} קבצים`);
  for (const [name] of edits) console.log(`   · ${name}`);
  console.log(`   · ${sectionNote}`);

  if (verify) {
    console.log("\n▸ בדיקת בנייה: npm run build");
    execSync("npm run build", { cwd: root, stdio: "inherit" });
  }

  check({ requireChangelog: false });

  console.log(`
השלבים הבאים:
  1. כתבו את השינויים ב-${CHANGELOG} והחליפו את שורת ה-TODO
     (npm run release:check נכשל עד שתעשו זאת — וכך גם ה-workflow)
  2. git add -A && git commit -m "Release v${next}"
  3. git tag v${next}
  4. git push origin main v${next}
`);
}

const args = process.argv.slice(2);
if (args.includes("--check")) {
  check({ tagName: process.env.GITHUB_REF_NAME });
} else {
  const target = args.find((arg) => !arg.startsWith("-"));
  if (!target) {
    fail("שימוש: node scripts/release.mjs <גרסה> [--no-verify] [--dry-run]  |  --check");
  }
  bumpVersion(target, {
    dryRun: args.includes("--dry-run"),
    verify: !args.includes("--no-verify"),
  });
}
