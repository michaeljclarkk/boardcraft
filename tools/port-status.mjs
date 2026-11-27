#!/usr/bin/env node
// Generate the port-status table and chart from areas/*.md.
// Usage: node tools/port-status.mjs [--check]
import { readdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { join } from "node:path";

const root = new URL("..", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const areasDir = join(root, "areas");
const readmePath = join(root, "README.md");
const svgPath = join(root, "docs", "port-status.svg");
const check = process.argv.includes("--check");

function parseArea(file) {
  const text = readFileSync(join(areasDir, file), "utf8");
  const match = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n/);
  if (!match) throw new Error(`${file}: missing frontmatter`);
  const meta = {};
  for (const line of match[1].split(/\r?\n/)) {
    const at = line.indexOf(":");
    if (at < 0) continue;
    meta[line.slice(0, at).trim()] = line.slice(at + 1).trim();
  }
  for (const key of ["id", "name", "order", "percent", "code"]) {
    if (!meta[key]) throw new Error(`${file}: missing ${key}`);
  }
  const percent = Number(meta.percent);
  if (!Number.isFinite(percent) || percent < 0 || percent > 100) {
    throw new Error(`${file}: percent must be 0-100`);
  }
  const body = text.slice(match[0].length);
  const title = body.match(/^#\s+(.+)$/m);
  if (!title) throw new Error(`${file}: missing title`);
  const afterTitle = body.slice(title.index + title[0].length);
  const scope = (afterTitle.split(/^##\s+/m)[0] ?? "").trim().replace(/\s+/g, " ");
  if (!scope) throw new Error(`${file}: missing scope paragraph`);
  return { ...meta, percent, scope };
}

const areas = readdirSync(areasDir)
  .filter((name) => name.endsWith(".md") && name !== "README.md")
  .map(parseArea)
  .sort((a, b) => Number(a.order) - Number(b.order));

const overall = Math.round(areas.reduce((sum, a) => sum + a.percent, 0) / areas.length);

const table = [
  "## Port map",
  "",
  `**Overall: ${overall}%** — mean across ${areas.length} areas. A handoff names one area; it includes its Rust rules, UI and desktop/browser/Android behaviour.`,
  "",
  "| Area | What it covers | Code home | Ported |",
  "|---|---|---|---|",
  ...areas.map(
    (a) =>
      `| [${a.name}](areas/${a.id}.md) | ${a.scope} | \`${a.code.split(",").map((s) => s.trim()).join("`, `")}\` | ${a.percent}% |`,
  ),
  "",
  "![Port status](docs/port-status.svg)",
  "",
  "**How the percentage works.** 0% is not started. 100% means every workflow in that area matches the internal tool on desktop, browser and Android — behaviour and UI, observed in the running app. Count verified user-visible behaviour only, never code, tests or scaffolding. When unsure, lower it. Change an area's `percent` in `areas/<id>.md`; the table and chart regenerate on commit.",
].join("\n");

function replaceBlock(text, body) {
  const start = "<!-- PORT-STATUS:START -->";
  const end = "<!-- PORT-STATUS:END -->";
  const a = text.indexOf(start);
  const b = text.indexOf(end);
  if (a < 0 || b < 0 || b < a) throw new Error("README.md: missing PORT-STATUS markers");
  return text.slice(0, a + start.length) + "\n" + body + "\n" + text.slice(b);
}

const svg = renderSvg(areas, overall);
const readme = readFileSync(readmePath, "utf8");
const nextReadme = replaceBlock(readme, table);

function renderSvg(rows, total) {
  const width = 980;
  const rowHeight = 26;
  const top = 96;
  const labelWidth = 250;
  const barWidth = 580;
  const height = top + rows.length * rowHeight + 24;
  const radius = 62;
  const circumference = 2 * Math.PI * radius;
  const dash = (total / 100) * circumference;

  const bars = rows
    .map((a, i) => {
      const y = top + i * rowHeight;
      const w = Math.round((a.percent / 100) * barWidth);
      const color = a.percent === 0 ? "#c9d3e0" : "#4f8cff";
      return [
        `<text x="24" y="${y + 15}" class="label">${escapeXml(a.name)}</text>`,
        `<rect x="${labelWidth}" y="${y + 4}" width="${barWidth}" height="12" rx="6" class="track"/>`,
        `<rect x="${labelWidth}" y="${y + 4}" width="${w}" height="12" rx="6" fill="${color}"/>`,
        `<text x="${width - 24}" y="${y + 15}" text-anchor="end" class="pct">${a.percent}%</text>`,
      ].join("");
    })
    .join("");

  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" role="img" aria-label="Port status: ${total}% overall">
<style>
  .bg{fill:#ffffff}.label{font:13px -apple-system,Segoe UI,Roboto,sans-serif;fill:#1f2937}
  .pct{font:600 13px -apple-system,Segoe UI,Roboto,sans-serif;fill:#1f2937}
  .track{fill:#e8edf5}.title{font:600 18px -apple-system,Segoe UI,Roboto,sans-serif;fill:#111827}
  .sub{font:13px -apple-system,Segoe UI,Roboto,sans-serif;fill:#6b7280}
  .donut{fill:none;stroke:#e8edf5;stroke-width:16}.arc{fill:none;stroke:#4f8cff;stroke-width:16;stroke-linecap:round}
  .big{font:600 28px -apple-system,Segoe UI,Roboto,sans-serif;fill:#111827}
  @media (prefers-color-scheme: dark){
    .bg{fill:#0d1117}.label,.pct{fill:#e6edf3}.title,.big{fill:#f0f6fc}.sub{fill:#8b949e}
    .track,.donut{stroke:#22272e;fill:none}.track{fill:#21262d}
  }
</style>
<rect class="bg" width="${width}" height="${height}" rx="12"/>
<text x="24" y="34" class="title">Port status</text>
<text x="24" y="56" class="sub">Mean across ${rows.length} areas; verified behaviour and UI only.</text>
<g transform="translate(24,${top - 10})">
  <circle class="donut" cx="${radius}" cy="${radius}" r="${radius}"/>
  <circle class="arc" cx="${radius}" cy="${radius}" r="${radius}" stroke-dasharray="${dash.toFixed(1)} ${(circumference - dash).toFixed(1)}" transform="rotate(-90 ${radius} ${radius})"/>
  <text x="${radius}" y="${radius + 4}" text-anchor="middle" class="big">${total}%</text>
  <text x="${radius}" y="${radius + 26}" text-anchor="middle" class="sub">overall</text>
</g>
${bars}
</svg>`;
}

function escapeXml(value) {
  return value.replace(/[<>&"]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;", '"': "&quot;" }[c]));
}

if (process.argv.includes("--summary")) {
  console.log(table);
  process.exit(0);
}

if (check) {
  const currentSvg = existsSync(svgPath) ? readFileSync(svgPath, "utf8") : "";
  const problems = [];
  if (nextReadme !== readme) problems.push("README.md");
  if (currentSvg !== svg) problems.push("docs/port-status.svg");
  if (problems.length) {
    console.error(`Port status is stale: ${problems.join(", ")}. Run: node tools/port-status.mjs`);
    process.exit(1);
  }
  console.log(`Port status is current (${overall}%).`);
} else {
  writeFileSync(readmePath, nextReadme);
  writeFileSync(svgPath, svg);
  console.log(`Port status: ${overall}% across ${areas.length} areas.`);
}
