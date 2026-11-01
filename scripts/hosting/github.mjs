// Explicit GitHub adapter. Other providers require a reviewed adapter and workflow changes.
import { execFileSync } from "node:child_process";

const runGh = (args) =>
  execFileSync("gh", args, {
    encoding: "utf8",
    windowsHide: true,
    timeout: 600_000,
    maxBuffer: 32 * 1024 * 1024,
  });
const validSha = (value) =>
  typeof value === "string" &&
  value.length === 40 &&
  /^[a-f0-9]{40}$/i.test(value);

export function githubHosting({ run = runGh } = {}) {
  return {
    sourceSha: (environment = process.env) =>
      environment.GITHUB_SHA ?? environment.MARKET_SOURCE_SHA,
    listReleases(repository) {
      const pages = JSON.parse(
        run([
          "api",
          `repos/${repository}/releases?per_page=100`,
          "--paginate",
          "--slurp",
        ]),
      );
      if (!Array.isArray(pages) || pages.some((page) => !Array.isArray(page)))
        throw new Error("Invalid release history response");
      const releases = pages.flat();
      if (
        releases.some(
          (release) =>
            !release ||
            typeof release.tag_name !== "string" ||
            typeof release.draft !== "boolean",
        )
      )
        throw new Error("Invalid release history entry");
      return releases;
    },
    verifyTagSource(repository, tag, sourceSha) {
      const expectedRef = `refs/tags/${tag}`;
      // Absence is [], without conflating a failed request with a missing tag.
      const references = JSON.parse(
        run(["api", `repos/${repository}/git/matching-refs/tags/${tag}`]),
      );
      if (
        !Array.isArray(references) ||
        references.some(
          (entry) =>
            typeof entry?.ref !== "string" ||
            !entry.ref.startsWith(expectedRef),
        )
      )
        throw new Error("Invalid release tag reference response");
      const matches = references.filter((entry) => entry.ref === expectedRef);
      if (matches.length > 1)
        throw new Error("Invalid duplicate release tag references");
      if (matches.length === 0) return;
      let object = matches[0].object;
      const visited = new Set();
      for (let depth = 0; depth <= 8; depth++) {
        if (!validSha(object?.sha))
          throw new Error("Invalid release tag object");
        const sha = object.sha.toLowerCase();
        if (object.type === "commit") {
          if (sha !== sourceSha.toLowerCase())
            throw new Error(
              "Release tag does not match the reviewed source commit",
            );
          return;
        }
        if (object.type !== "tag" || depth === 8 || visited.has(sha))
          throw new Error("Release tag does not resolve to a commit");
        visited.add(sha);
        const annotated = JSON.parse(
          run(["api", `repos/${repository}/git/tags/${sha}`]),
        );
        if (!validSha(annotated?.sha) || annotated.sha.toLowerCase() !== sha)
          throw new Error("Invalid annotated release tag response");
        object = annotated.object;
      }
    },
    createDraft({ repository, tag, sourceSha, title, notesFile }) {
      run([
        "release",
        "create",
        tag,
        "--repo",
        repository,
        "--target",
        sourceSha,
        "--draft",
        "--title",
        title,
        "--notes-file",
        notesFile,
      ]);
    },
    uploadAssets({ repository, tag, assets }) {
      run(["release", "upload", tag, "--repo", repository, ...assets]);
    },
    publishLatest({ repository, tag }) {
      run([
        "release",
        "edit",
        tag,
        "--repo",
        repository,
        "--draft=false",
        "--latest",
      ]);
    },
  };
}
