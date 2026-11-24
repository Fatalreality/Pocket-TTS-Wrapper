#!/usr/bin/env node
// Reports how far the vendored pieces are behind upstream, so a wrapper fix never depends on
// someone remembering to look.
//
//   node scripts/check-upstream.mjs [--out report.md]
//
// Compares the model configs in `configs/` with `pocket_tts/config/` on kyutai-labs/pocket-tts and
// the vendored `vendor/ptts` revision with the head of gabrieldemarmiesse/xn-ptts. It also lists
// the latest upstream commits, so a human can judge whether anything else needs porting.
// Exits 0 either way; the report says whether there is drift, and `GITHUB_TOKEN` is used when
// present (the workflow passes it, which avoids the anonymous rate limit).

import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const UPSTREAM = "kyutai-labs/pocket-tts";
const XN = "gabrieldemarmiesse/xn-ptts";
const CONFIG_DIR = "pocket_tts/config";
const args = process.argv.slice(2);
const outIndex = args.indexOf("--out");
const outPath = outIndex >= 0 ? args[outIndex + 1] : null;

const token = process.env.GITHUB_TOKEN || process.env.GH_TOKEN || "";
const headers = {
  Accept: "application/vnd.github+json",
  "User-Agent": "pocket-tts-upstream-check",
  ...(token ? { Authorization: `Bearer ${token}` } : {}),
};

async function api(path) {
  const res = await fetch(`https://api.github.com${path}`, { headers });
  if (!res.ok) {
    throw new Error(`GET ${path} failed: ${res.status} ${await res.text()}`);
  }
  return res.json();
}

async function raw(url) {
  const res = await fetch(url, { headers: { "User-Agent": "pocket-tts-upstream-check" } });
  if (!res.ok) throw new Error(`GET ${url} failed: ${res.status}`);
  return Buffer.from(await res.arrayBuffer());
}

/// The contents with CRLF folded to LF: this checkout is on Windows, upstream is not, and a
/// line-ending difference is not an upstream change.
function normalized(bytes) {
  return bytes.toString("utf8").replace(/\r\n/g, "\n");
}

const lines = [];
const log = (line = "") => lines.push(line);
let drift = false;

function noteDrift() {
  drift = true;
}

try {
  // ---- model configs ----
  const remoteConfigs = await api(`/repos/${UPSTREAM}/contents/${CONFIG_DIR}?ref=main`);
  const remoteNames = remoteConfigs
    .filter((entry) => entry.type === "file" && entry.name.endsWith(".yaml"))
    .map((entry) => entry.name)
    .sort();
  const localNames = readdirSync("configs")
    .filter((name) => name.endsWith(".yaml"))
    .sort();

  const changed = [];
  const missing = remoteNames.filter((name) => !localNames.includes(name));
  const extra = localNames.filter((name) => !remoteNames.includes(name));
  for (const entry of remoteConfigs.filter(
    (e) => e.type === "file" && e.name.endsWith(".yaml")
  )) {
    const local = readFileSync(join("configs", entry.name));
    const remote = await raw(entry.download_url);
    if (normalized(local) !== normalized(remote)) changed.push(entry.name);
  }
  if (changed.length || missing.length || extra.length) noteDrift();

  log("## Model configs");
  log();
  if (changed.length || missing.length || extra.length) {
    if (changed.length) log(`- **Changed upstream:** ${changed.join(", ")}`);
    if (missing.length) log(`- **New upstream configs:** ${missing.join(", ")}`);
    if (extra.length) log(`- **Only in this repository:** ${extra.join(", ")}`);
    log();
    log("Vendored configs live in `configs/`; copy the changed files in and rebuild.");
  } else {
    log(`All ${localNames.length} vendored configs are byte-identical to upstream \`main\`.`);
  }
  log();

  // ---- vendored ptts ----
  const vendored = readFileSync(join("vendor", "ptts", "VENDORED.md"), "utf8");
  const vendoredRev = (vendored.match(/\b([0-9a-f]{40})\b/) || [])[1] || "unknown";
  const xnHead = await api(`/repos/${XN}/commits/main`);
  // `ahead_by` counts what `main` has that the vendored revision does not; a vendored revision
  // from a later commit than `main` is fine, and must not raise an issue every week.
  const relation =
    vendoredRev === "unknown"
      ? null
      : await api(`/repos/${XN}/compare/${vendoredRev}...main`);

  log("## Vendored inference crate (`vendor/ptts`, from xn-ptts)");
  log();
  log(`- Vendored revision: \`${vendoredRev.slice(0, 7)}\``);
  log(`- xn-ptts \`main\`: [\`${xnHead.sha.slice(0, 7)}\`](${xnHead.html_url})`);
  if (!relation) {
    log("- Could not read the vendored revision from `vendor/ptts/VENDORED.md`.");
  } else if (relation.ahead_by > 0) {
    noteDrift();
    log(
      `- \`main\` is **${relation.ahead_by} commit(s) ahead** of the vendored revision; check ` +
        "whether they need porting into `vendor/ptts`."
    );
  } else if (relation.status === "behind") {
    log("- The vendored revision is ahead of `main`; nothing newer to take.");
  } else {
    log("- The vendored revision is current.");
  }
  log();

  // ---- recent upstream work ----
  const commits = await api(`/repos/${UPSTREAM}/commits?per_page=10`);
  log("## Latest upstream commits (`kyutai-labs/pocket-tts`)");
  log();
  for (const commit of commits) {
    const message = commit.commit.message.split("\n")[0];
    const date = commit.commit.committer.date.slice(0, 10);
    log(`- ${date} [\`${commit.sha.slice(0, 7)}\`](${commit.html_url}) ${message}`);
  }
  log();
  log(
    "Triage: config or voice-catalog changes belong in the app; server, training and Python-only " +
      "fixes usually do not."
  );
} catch (e) {
  noteDrift();
  log("## Upstream check failed");
  log();
  log(`\`${e.message}\``);
  log();
  log("The GitHub API may be rate limited without `GITHUB_TOKEN`; re-run the workflow by hand.");
}

const report = lines.join("\n");
console.log(report);
if (outPath) writeFileSync(outPath, `${report}\n`);
if (process.env.GITHUB_OUTPUT) {
  writeFileSync(process.env.GITHUB_OUTPUT, `drift=${drift ? "yes" : "no"}\n`, { flag: "a" });
}
