#!/usr/bin/env node
// Release helper for Pocket TTS: one place that knows where the version lives, how the
// changelog becomes the release notes, and which files go on a GitHub release.
//
//   node scripts/release.mjs version
//   node scripts/release.mjs set-version 0.2.2
//   node scripts/release.mjs notes
//   node scripts/release.mjs stage [dir]
//   node scripts/release.mjs publish [dir]
//
// Run it from the repository root. `stage` needs a build in target/release; `publish` needs a
// GITHUB_TOKEN (or cached git credentials) and an existing tag.

import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
} from "node:fs";
import { basename, join } from "node:path";

const REPO = "Fatalreality/Pocket-TTS-Wrapper";
const TAURI_CONF = "tauri.conf.json";
const CARGO_TOML = "Cargo.toml";
const CARGO_LOCK = "Cargo.lock";
const CHANGELOG = "CHANGELOG.md";
const README = "README.md";
const BUNDLE_DIR = join("target", "release", "bundle", "nsis");

const read = (path) => readFileSync(path, "utf8");

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

function version() {
  return JSON.parse(read(TAURI_CONF)).version;
}

/** The changelog section for `v`, without its heading, or an error naming the missing heading. */
function notes(v) {
  const text = read(CHANGELOG);
  const heading = `## [${v}]`;
  const start = text.indexOf(heading);
  if (start < 0) fail(`no "${heading}" section in ${CHANGELOG} - add one before releasing`);
  const bodyStart = text.indexOf("\n", start) + 1;
  const next = text.indexOf("\n## [", bodyStart);
  return text.slice(bodyStart, next < 0 ? text.length : next).trim();
}

function setVersion(v) {
  if (!/^\d+\.\d+\.\d+$/.test(v)) fail(`"${v}" is not a x.y.z version`);

  const conf = JSON.parse(read(TAURI_CONF));
  conf.version = v;
  writeFileSync(TAURI_CONF, `${JSON.stringify(conf, null, 2)}\n`);

  writeFileSync(
    CARGO_TOML,
    read(CARGO_TOML).replace(/^version = "[^"]*"/m, `version = "${v}"`)
  );

  // Cargo.lock carries the root package version too, and CI builds with --locked.
  writeFileSync(
    CARGO_LOCK,
    read(CARGO_LOCK).replace(
      /(name = "pocket-tts-app"\nversion = ")[^"]*(")/,
      `$1${v}$2`
    )
  );

  writeFileSync(
    README,
    read(README).replace(/Pocket-TTS-[\d.]+-x64-setup\.exe/g, `Pocket-TTS-${v}-x64-setup.exe`)
  );

  if (!read(CHANGELOG).includes(`## [${v}]`)) {
    console.log(`note: ${CHANGELOG} has no "## [${v}]" section yet`);
  }
  console.log(`version set to ${v} in ${TAURI_CONF}, ${CARGO_TOML}, ${CARGO_LOCK} and ${README}`);
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

/** Copies the build output into `dir` under the names the README and updater use. */
function stage(dir = "github-release") {
  const v = version();
  const installer = readdirSync(BUNDLE_DIR).find(
    (name) => name.endsWith(`_${v}_x64-setup.exe`) && !name.endsWith(".sig")
  );
  if (!installer) fail(`no installer for ${v} in ${BUNDLE_DIR} - run the tauri build first`);

  const binary = join("target", "release", "pocket-tts-app.exe");
  if (!existsSync(binary)) fail(`no ${binary} - run the tauri build first`);

  mkdirSync(dir, { recursive: true });

  const setupName = `Pocket-TTS-${v}-x64-setup.exe`;
  const portableName = `Pocket-TTS-${v}-portable.exe`;
  copyFileSync(join(BUNDLE_DIR, installer), join(dir, setupName));
  copyFileSync(binary, join(dir, portableName));

  for (const doc of ["README.txt", "HF_TOKEN_GUIDE.txt", "app-icon.webp"]) {
    copyFileSync(join("docs", doc), join(dir, doc));
  }

  const checksummed = [setupName, portableName, "README.txt", "HF_TOKEN_GUIDE.txt", "app-icon.webp"];
  const sums = checksummed.map((name) => `${sha256(join(dir, name))}  ${name}`).join("\n");
  writeFileSync(join(dir, "SHA256SUMS.txt"), `${sums}\n`);

  // The updater manifest: the signature of the installer it points at, so the running app can
  // verify the download before installing it.
  const sigPath = join(BUNDLE_DIR, `${installer}.sig`);
  if (!existsSync(sigPath)) {
    fail(`no ${sigPath}: build with TAURI_SIGNING_PRIVATE_KEY set to sign the update`);
  }
  const manifest = {
    version: v,
    notes: notes(v),
    pub_date: new Date().toISOString(),
    platforms: {
      "windows-x86_64": {
        signature: read(sigPath).trim(),
        url: `https://github.com/${REPO}/releases/download/v${v}/${setupName}`,
      },
    },
  };
  writeFileSync(join(dir, "latest.json"), `${JSON.stringify(manifest, null, 2)}\n`);

  console.log(`staged ${v} in ${dir}:`);
  for (const name of [...checksummed, "SHA256SUMS.txt", "latest.json"]) {
    console.log(`  ${name}`);
  }
}

function token() {
  if (process.env.GITHUB_TOKEN) return process.env.GITHUB_TOKEN;
  try {
    const out = execFileSync(
      "git",
      ["credential", "fill"],
      { input: "protocol=https\nhost=github.com\n\n", encoding: "utf8" }
    );
    const match = out.match(/^password=(.*)$/m);
    if (match) return match[1].trim();
  } catch {
    // fall through to the error below
  }
  return fail("no GITHUB_TOKEN and no cached git credentials for github.com");
}

async function api(path, init = {}) {
  const res = await fetch(`https://api.github.com${path}`, {
    ...init,
    headers: {
      Authorization: `Bearer ${token()}`,
      Accept: "application/vnd.github+json",
      "X-GitHub-Api-Version": "2022-11-28",
      "User-Agent": "pocket-tts-release",
      ...(init.headers || {}),
    },
  });
  const body = res.status === 204 ? null : await res.json().catch(() => null);
  if (!res.ok) fail(`${init.method || "GET"} ${path} failed: ${res.status} ${JSON.stringify(body)}`);
  return body;
}

async function publish(dir = "github-release") {
  const v = version();
  const tag = `v${v}`;
  const body = notes(v);

  let release;
  const existing = await fetch(`https://api.github.com/repos/${REPO}/releases/tags/${tag}`, {
    headers: { "User-Agent": "pocket-tts-release" },
  });
  if (existing.ok) {
    release = await existing.json();
    release = await api(`/repos/${REPO}/releases/${release.id}`, {
      method: "PATCH",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ name: `Pocket TTS ${v}`, body, draft: false, prerelease: false }),
    });
    console.log(`updating existing release ${tag} (${release.id})`);
  } else {
    release = await api(`/repos/${REPO}/releases`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        tag_name: tag,
        target_commitish: "main",
        name: `Pocket TTS ${v}`,
        body,
        draft: false,
        prerelease: false,
      }),
    });
    console.log(`created release ${tag} (${release.id})`);
  }

  const upload = async (name) => {
    const current = await api(`/repos/${REPO}/releases/${release.id}/assets`);
    for (const asset of current.filter((a) => a.name === name)) {
      await api(`/repos/${REPO}/releases/assets/${asset.id}`, { method: "DELETE" });
    }
    const bytes = readFileSync(join(dir, name));
    const asset = await api(
      `/repos/${REPO}/releases/${release.id}/assets?name=${encodeURIComponent(name)}`,
      {
        method: "POST",
        headers: { "Content-Type": "application/octet-stream" },
        body: bytes,
      }
    );
    console.log(`  uploaded ${name} (${asset.size} bytes)`);
  };

  for (const name of readdirSync(dir).sort()) {
    await upload(name);
  }
  console.log(`published ${release.html_url}`);
}

const [command, argument] = process.argv.slice(2);
switch (command) {
  case "version":
    console.log(version());
    break;
  case "set-version":
    if (!argument) fail("usage: node scripts/release.mjs set-version 0.2.2");
    setVersion(argument);
    break;
  case "notes":
    console.log(notes(argument || version()));
    break;
  case "stage":
    stage(argument);
    break;
  case "publish":
    await publish(argument);
    break;
  default:
    console.log(
      [
        "usage: node scripts/release.mjs <command>",
        "",
        "  version              print the current version",
        "  set-version x.y.z    update every place the version lives",
        "  notes                print the changelog section for the current version",
        "  stage [dir]          rename the build output, write SHA256SUMS.txt and latest.json",
        "  publish [dir]        create or update the GitHub release from [dir]",
      ].join("\n")
    );
    process.exit(command ? 1 : 0);
}
