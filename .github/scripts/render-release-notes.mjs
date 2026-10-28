#!/usr/bin/env node
/**
 * Renders the body of a published Slate release from a fixed template.
 *
 * The entries are generated from the commits between this tag and the
 * previous one, grouped by conventional-commit type under numbered headings,
 * each entry carrying the commit's own title, its hash link, and the GitHub
 * user that authored it. Hidden types (ci, chore, build, ...) are skipped.
 *
 * Requires a checkout with all tags (fetch-depth: 0, fetch-tags: true), gh
 * with GH_TOKEN set, and node.
 * Usage: node render-release-notes.mjs <tag>
 */

import fs from "node:fs";
import { execFileSync } from "node:child_process";

const SECTIONS = [
  ["feat", "Features"],
  ["fix", "Bug Fixes"],
  ["perf", "Performance"],
  ["refactor", "Refactoring"],
  ["style", "Styling"],
  ["test", "Tests"],
  ["docs", "Documentation"],
];
// Types release-please hides in the changelog go here too.
const HIDDEN = new Set(["build", "ci", "chore", "init"]);
const SUBJECT = /^(\w+)(?:\(([^)]*)\))?!?:\s*(.+)$/;

const repo = process.env.GITHUB_REPOSITORY;
const tag = process.argv[2];
if (!repo || !tag) {
  console.error("usage: node render-release-notes.mjs <tag> (with GITHUB_REPOSITORY set)");
  process.exit(1);
}

const sh = (cmd, ...args) =>
  execFileSync(cmd, args, { encoding: "utf8", maxBuffer: 1 << 26 }).trim().replace(/\r/g, "");

const commitUrl = (sha) => `https://github.com/${repo}/commit/${sha}`;
const commitLink = (sha) => `[\`${sha.slice(0, 7)}\`](${commitUrl(sha)})`;

// ---- previous tag ---------------------------------------------------------

const tags = sh("git", "tag", "--list", "v*", "--sort=-v:refname")
  .split("\n")
  .map((t) => t.trim())
  .filter(Boolean);
const ix = tags.indexOf(tag);
const prev = ix === -1 ? null : (tags[ix + 1] ?? null);

// ---- GitHub handles for commits ------------------------------------------

/**
 * Maps commit sha to the author's GitHub login. `since` stops the walk at
 * that commit so one release only asks the API for its own commits.
 */
function fetchHandles(since) {
  const map = new Map();
  for (let page = 1; page < 20; page += 1) {
    let list;
    try {
      list = JSON.parse(
        sh("gh", "api", `repos/${repo}/commits?sha=${tag}&per_page=100&page=${page}`),
      );
    } catch (err) {
      console.error("gh api failed, bullets fall back to git names:", err.message);
      return map;
    }
    if (!Array.isArray(list) || list.length === 0) break;
    for (const c of list) {
      map.set(c.sha, c.author?.login || "");
      if (since && c.sha === since) return map;
    }
  }
  return map;
}

// ---- entries and sections -------------------------------------------------

/** GitHub login from a `123+name@users.noreply.github.com` address, if present. */
function handleFromEmail(email) {
  const m = /^(?:\d+\+)?([^@]+)@users\.noreply\.github\.com$/.exec(email ?? "");
  return m ? m[1] : "";
}

function entry(entry, number) {
  const login = handles.get(entry.sha) || handleFromEmail(entry.gitEmail);
  const who = login
    ? `by @${login}`
    : `by ${entry.gitAuthor}`;
  return `**\`${number}.\`** ${entry.title} (${commitLink(entry.sha)}) ${who}`;
}

/** `[#123](url)` when the commit came in through a PR, else the commit. */
function contributionLink(sha) {
  try {
    const list = JSON.parse(sh("gh", "api", `repos/${repo}/commits/${sha}/pulls`));
    const pr = Array.isArray(list) ? list.find((p) => p.number) : null;
    if (pr) return `[#${pr.number}](${pr.html_url})`;
  } catch {
    // Direct pushes have no PR; fall through to the commit link.
  }
  return commitLink(sha);
}

// ---- body -----------------------------------------------------------------

const sinceSha = prev ? sh("git", "rev-parse", `${prev}^{commit}`) : null;
const handles = fetchHandles(sinceSha);
if (prev && !handles.has(sinceSha)) {
  console.error("commit list did not reach the previous tag; some handles may be wrong");
}
const raw = sh(
  "git",
  "log",
  "--no-merges",
  "--format=%H%x09%s%x09%an%x09%ae",
  prev ? `${prev}..${tag}` : tag,
);

const grouped = new Map(SECTIONS.map(([, title]) => [title, []]));
const authors = new Map(); // identity -> oldest commit by that author in this release
for (const line of raw.split("\n")) {
  if (!line.trim()) continue;
  const [sha, subject, gitAuthor, gitEmail] = line.split("\t");
  const login = handles.get(sha) || handleFromEmail(gitEmail);
  const key = (login || gitEmail).toLowerCase();
  if (key) authors.set(key, { sha, gitAuthor, gitEmail, login });
  const m = SUBJECT.exec(subject);
  if (!m || HIDDEN.has(m[1].toLowerCase()) || m[3].startsWith("release")) continue;
  const section = SECTIONS.find(([t]) => t === m[1].toLowerCase())?.[1];
  if (!section) {
    console.error(`skipping hidden type: ${subject}`);
    continue;
  }
  // The commit's own title, scope and all, untouched.
  grouped.get(section).push({ title: subject, sha, gitAuthor, gitEmail });
}

const parts = ["## What's changed"];
for (const [, title] of SECTIONS) {
  const items = grouped.get(title);
  if (!items.length) continue;
  parts.push(
    `### ${title}\n\n${items
      .map((item, n) => entry(item, n + 1))
      .join("\n\n")}`,
  );
}

// ---- new contributors -----------------------------------------------------

/** Identities (handles, else emails) that committed before the previous tag. */
function priorIdentities() {
  if (!prev) return new Set();
  const seen = new Set();
  for (const email of sh("git", "log", "--format=%ae", prev).split("\n")) {
    const e = email.trim().toLowerCase();
    if (!e) continue;
    seen.add(handleFromEmail(e).toLowerCase() || e);
  }
  return seen;
}

const known = priorIdentities();
const newcomers = [...authors.entries()]
  .filter(([key, a]) => a.login && !known.has(key) && !a.login.endsWith("[bot]"))
  .map(([, a]) => a);
if (newcomers.length) {
  parts.push(
    `<br>\n\n## New Contributors\n\n${newcomers
      .map((a) => `* @${a.login} made their first contribution in ${contributionLink(a.sha)}`)
      .join("\n")}`,
  );
}

if (prev) {
  const compare = `https://github.com/${repo}/compare/${prev}...${tag}`;
  parts.push(
    `<br>\n\n<code>Full Changelog <a href="${compare}">${prev}...${tag}</a></code>`,
  );
}

const body = parts.join("\n\n") + "\n";
const bodyFile = "release-body.md";
fs.writeFileSync(bodyFile, body);
if (process.env.RELEASE_BODY_OUT) {
  fs.writeFileSync(process.env.RELEASE_BODY_OUT, body);
  console.log(`wrote body to ${process.env.RELEASE_BODY_OUT}`);
}
if (process.env.DRY_RUN) {
  console.log(body);
} else {
  sh("gh", "release", "edit", tag, "--notes-file", bodyFile);
}
