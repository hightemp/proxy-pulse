import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import {
  parseVersion,
  projectRoot,
  readVersion,
  synchronizeVersion,
} from "./version.mjs";

export function command(program, args, root = projectRoot) {
  const result = spawnSync(program, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error)
    throw new Error(`${program} is required: ${result.error.code}`);
  return result;
}

function git(root, args) {
  const result = command("git", args, root);
  if (result.status !== 0)
    throw new Error(
      `Git ${args[0]} failed. Check repository state, remote access and authentication.`,
    );
  return result.stdout.trim();
}

export function repositoryName(value) {
  const match = /^([A-Za-z0-9][A-Za-z0-9-]*)\/([A-Za-z0-9_.-]+)$/.exec(
    value || "",
  );
  if (!match || [".", ".."].includes(match[2]))
    throw new Error("Specify a GitHub repository as owner/repository.");
  return value;
}

export function repositoryFromRemote(url) {
  const ssh = /^git@github\.com:([^\s]+)$/.exec(url);
  let name = ssh?.[1];
  if (!name) {
    let parsed;
    try {
      parsed = new URL(url);
    } catch {
      throw new Error(
        "Cannot infer the GitHub repository. Set RELEASE_REPOSITORY=owner/repository.",
      );
    }
    if (
      parsed.hostname !== "github.com" ||
      !["https:", "ssh:"].includes(parsed.protocol)
    )
      throw new Error(
        "Remote must be on GitHub, or set RELEASE_REPOSITORY=owner/repository for an SSH alias.",
      );
    name = parsed.pathname.replace(/^\//, "");
  }
  return repositoryName(name.replace(/\.git$/, ""));
}

function remoteName(value) {
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value))
    throw new Error("Invalid remote name.");
  return value;
}

function getRepository(root, remote, repository) {
  if (repository) return repositoryName(repository);
  const result = command("git", ["remote", "get-url", remote], root);
  if (result.status !== 0)
    throw new Error(
      `Remote ${remote} is not configured. Add the GitHub remote before releasing.`,
    );
  return repositoryFromRemote(result.stdout.trim());
}

export function validateTag(root, tag) {
  if (!tag?.startsWith("v") || `v${parseVersion(tag.slice(1))}` !== tag)
    throw new Error("Release tags must use v followed by a SemVer version.");
  synchronizeVersion(root, true);
  if (tag !== `v${readVersion(root)}`)
    throw new Error("The release tag does not match VERSION.");
  const commit = git(root, ["rev-parse", `refs/tags/${tag}^{commit}`]);
  if (commit !== git(root, ["rev-parse", "HEAD"]))
    throw new Error("The checked-out commit does not match the release tag.");
  return readVersion(root);
}

function previousTag(root, tag, ref) {
  const tags = git(root, ["tag", "--merged", ref, "--list", "v*"])
    .split("\n")
    .filter(Boolean)
    .filter((candidate) => {
      if (candidate === tag) return false;
      try {
        return candidate === `v${parseVersion(candidate.slice(1))}`;
      } catch {
        return false;
      }
    });
  return tags
    .map((name) => ({
      name,
      distance: Number(git(root, ["rev-list", "--count", `${name}..${ref}`])),
    }))
    .sort(
      (a, b) =>
        a.distance - b.distance ||
        b.name.localeCompare(a.name, "en", { numeric: true }),
    )[0]?.name;
}

function markdownText(value) {
  return value.replace(/[\\`*_\[\]<>|]/g, "\\$&");
}

export function releaseNotes(
  root,
  repository,
  tag = `v${readVersion(root)}`,
  ref = "HEAD",
) {
  repository = repositoryName(repository);
  const previous = previousTag(root, tag, ref);
  const range = previous ? `${previous}..${ref}` : ref;
  const output = git(root, [
    "log",
    "--reverse",
    "--format=%H%x00%s%x00",
    range,
  ]);
  const fields = output.split("\0");
  const commits = [];
  for (let index = 0; index + 1 < fields.length; index += 2) {
    const hash = fields[index].trim();
    if (!hash) continue;
    commits.push(
      `- [${hash.slice(0, 7)}](https://github.com/${repository}/commit/${hash}) ${markdownText(fields[index + 1])}`,
    );
  }
  const comparison = previous
    ? `\n[Full changelog](https://github.com/${repository}/compare/${encodeURIComponent(previous)}...${encodeURIComponent(tag)})\n`
    : "\nInitial release.\n";
  return `## Commits\n\n${commits.length ? commits.join("\n") : "No additional commits since the previous release."}\n${comparison}`;
}

export function prepareRelease(root, { remote = "origin", repository } = {}) {
  remote = remoteName(remote);
  const version = synchronizeVersion(root, true).version;
  if (git(root, ["status", "--porcelain"]))
    throw new Error(
      "The working tree must be clean. Commit VERSION and generated metadata before releasing.",
    );
  const branch = git(root, ["symbolic-ref", "--quiet", "--short", "HEAD"]);
  const head = git(root, ["rev-parse", "HEAD"]);
  repository = getRepository(root, remote, repository);
  const publishedHead = git(root, [
    "ls-remote",
    "--heads",
    remote,
    `refs/heads/${branch}`,
  ]).split(/\s/)[0];
  if (publishedHead !== head)
    throw new Error(
      `Push the current ${branch} commit to ${remote} before releasing. The remote branch must match HEAD.`,
    );
  const tag = `v${version}`;
  if (
    git(root, [
      "ls-remote",
      "--tags",
      remote,
      `refs/tags/${tag}`,
      `refs/tags/${tag}^{}`,
    ])
  ) {
    throw new Error(
      `${tag} is already on ${remote}. Rerun its GitHub Actions workflow to retry a failed build; do not move the tag.`,
    );
  }
  const localTag = command(
    "git",
    ["rev-parse", "--verify", `refs/tags/${tag}^{commit}`],
    root,
  );
  if (localTag.status === 0 && localTag.stdout.trim() !== head)
    throw new Error(`Local tag ${tag} points to a different commit.`);
  return {
    version,
    tag,
    head,
    branch,
    remote,
    repository,
    existingLocalTag: localTag.status === 0,
    notes: releaseNotes(root, repository, tag),
  };
}

export function publishTag(root, options = {}) {
  const plan = prepareRelease(root, options);
  if (options.dryRun) return { ...plan, pushed: false };
  // Detect a local checkout change between preflight and the mutating operation.
  if (
    git(root, ["rev-parse", "HEAD"]) !== plan.head ||
    git(root, ["status", "--porcelain"])
  )
    throw new Error(
      "The checkout changed during release preparation. Retry from a clean checkout.",
    );
  if (!plan.existingLocalTag)
    git(root, ["tag", "-a", plan.tag, plan.head, "-m", `Release ${plan.tag}`]);
  const result = command(
    "git",
    ["push", plan.remote, `refs/tags/${plan.tag}:refs/tags/${plan.tag}`],
    root,
  );
  if (result.status !== 0)
    throw new Error(
      `Tag push failed. Local tag ${plan.tag} was retained. Check the remote state and authentication before retrying.`,
    );
  return { ...plan, pushed: true };
}

function argumentsOf(argv) {
  const options = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--dry-run") options.dryRun = true;
    else if (
      ["--remote", "--repository", "--tag", "--output"].includes(argv[i]) &&
      argv[i + 1]
    )
      options[argv[i].slice(2)] = argv[++i];
    else throw new Error(`Unknown or incomplete option: ${argv[i]}`);
  }
  return options;
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href
) {
  try {
    const action = process.argv[2];
    const options = argumentsOf(process.argv.slice(3));
    options.remote ||= process.env.RELEASE_REMOTE || "origin";
    options.repository ||=
      process.env.RELEASE_REPOSITORY || process.env.GITHUB_REPOSITORY;
    if (action === "publish") {
      const plan = publishTag(projectRoot, options);
      console.log(
        `${plan.pushed ? "Pushed" : "Dry run:"} ${plan.tag} at ${plan.head.slice(0, 12)} to ${plan.remote}.`,
      );
      console.log(plan.notes);
      if (plan.pushed)
        console.log(
          `Follow the release build: https://github.com/${plan.repository}/actions`,
        );
    } else if (action === "notes") {
      const repository = getRepository(
        projectRoot,
        remoteName(options.remote),
        options.repository,
      );
      const notes = releaseNotes(projectRoot, repository, options.tag);
      if (options.output) {
        fs.mkdirSync(path.dirname(options.output), { recursive: true });
        fs.writeFileSync(options.output, notes);
      } else console.log(notes);
    } else if (action === "validate-tag") {
      console.log(validateTag(projectRoot, options.tag));
    } else
      throw new Error(
        "Usage: node scripts/release.mjs publish [--dry-run] | notes [--output file] | validate-tag --tag vX.Y.Z",
      );
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
