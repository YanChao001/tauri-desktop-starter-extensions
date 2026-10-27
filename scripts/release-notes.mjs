import { execFileSync } from "node:child_process";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const validSha = (value) => /^[a-f0-9]{40}$/i.test(value);
const headings = new Map([
  ["feat", "新功能"],
  ["fix", "问题修复"],
  ["perf", "性能优化"],
  ["refactor", "代码重构"],
  ["docs", "文档"],
  ["build", "构建与依赖"],
  ["ci", "持续集成"],
  ["test", "测试"],
  ["style", "代码风格"],
  ["chore", "维护"],
  ["revert", "回退"],
  ["other", "其他变更"],
]);
const markdown = (value) =>
  String(value)
    .replace(/\s+/g, " ")
    .replace(/[\\`*_{}[\]()<>|]/g, "\\$&");

export const runGit = (args) =>
  execFileSync("git", args, {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
    timeout: 60_000,
    maxBuffer: 32 * 1024 * 1024,
  });

/** Only public, non-prerelease market catalogs form a release-notes baseline. */
export function previousReleaseTag(releases) {
  let previous = 0;
  for (const release of releases) {
    if (release.draft || release.prerelease === true) continue;
    const match = /^market-([1-9]\d*)$/.exec(release.tag_name);
    if (match && Number(match[1]) > previous) previous = Number(match[1]);
  }
  return previous ? `market-${previous}` : undefined;
}

export function readReleaseCommits(sourceSha, previousTag, git = runGit) {
  if (!validSha(sourceSha)) throw new Error("Invalid release notes source SHA");
  if (git(["rev-parse", "--is-shallow-repository"]).trim() !== "false")
    throw new Error(
      "Release notes require complete Git history; fetch with --unshallow first",
    );
  const source = git(["rev-parse", "--verify", `${sourceSha}^{commit}`]).trim();
  if (source.toLowerCase() !== sourceSha.toLowerCase())
    throw new Error("Release notes source commit mismatch");
  let range = source;
  if (previousTag) {
    if (!/^market-[1-9]\d*$/.test(previousTag))
      throw new Error("Invalid release notes baseline tag");
    const previous = git([
      "rev-parse",
      "--verify",
      `refs/tags/${previousTag}^{commit}`,
    ]).trim();
    if (!validSha(previous))
      throw new Error("Invalid release notes baseline SHA");
    // Missing history or a rewritten release lineage must not silently omit changes.
    git(["merge-base", "--is-ancestor", previous, source]);
    range = `${previous}..${source}`;
  }
  const fields = git([
    "log",
    "--no-merges",
    "--reverse",
    "--format=%H%x00%B%x00",
    range,
    "--",
  ])
    .trim()
    .split("\0");
  if (fields.length === 1 && fields[0] === "") return [];
  if (fields.pop() !== "" || fields.length % 2 !== 0)
    throw new Error("Invalid release commit history");
  const commits = [];
  for (let index = 0; index < fields.length; index += 2) {
    const sha = fields[index].trim();
    const message = fields[index + 1].trim();
    if (!validSha(sha) || !message) throw new Error("Invalid release commit");
    commits.push({ sha, message });
  }
  return commits;
}

export function renderReleaseNotes(catalog, sourceSha, previousTag, commits) {
  const base = `https://github.com/${catalog.repository}`;
  const groups = new Map([...headings.keys()].map((type) => [type, []]));
  const breaking = [];
  for (const { sha, message } of commits) {
    const [subject] = message.split(/\r?\n/);
    const match = /^([a-z]+)(?:\(([^)\r\n]+)\))?(!)?:[ \t]+(.+)$/.exec(subject);
    const type = match && headings.has(match[1]) ? match[1] : "other";
    const scope = match?.[2] ? `**${markdown(match[2])}：** ` : "";
    const link = `([${sha.slice(0, 7)}](${base}/commit/${sha}))`;
    const item = `${scope}${markdown(match?.[4] ?? subject)} ${link}`;
    groups.get(type).push(item);
    const footer = /(?:^|\n)BREAKING[ -]CHANGE:[ \t]*([\s\S]+)/.exec(message);
    if (match?.[3] || footer) {
      const details = footer?.[1].split(/\n[A-Za-z][A-Za-z-]*:[ \t]/)[0].trim();
      breaking.push(details ? `${item} — ${markdown(details)}` : item);
    }
  }
  const lines = [
    "## 发布信息",
    "",
    `- 目录修订：\`market-${catalog.revision}\`（schema ${catalog.schemaVersion}）`,
    `- 市场源码：[${sourceSha.slice(0, 7)}](${base}/commit/${sourceSha})`,
    `- 变更基线：${previousTag ? `[${previousTag}](${base}/releases/tag/${previousTag})` : "首次正式发布，包含全部源码历史"}`,
    `- 签名有效期：${new Date(catalog.issuedAt * 1000).toISOString()} 至 ${new Date(catalog.expiresAt * 1000).toISOString()}（UTC）`,
    "",
    "## 破坏性变更",
    "",
    ...(breaking.length
      ? breaking.map((item) => `- ${item}`)
      : ["本次提交未声明破坏性变更。"]),
    "",
    "## 更新内容",
    "",
  ];
  for (const [type, items] of groups) {
    if (items.length)
      lines.push(
        `### ${headings.get(type)}（${type}）`,
        "",
        ...items.map((item) => `- ${item}`),
        "",
      );
  }
  if (!commits.length)
    lines.push(
      "本次范围没有新增的非合并提交；目录已重新签发，源码差异见完整变更。",
      "",
    );
  lines.push("## 插件版本", "");
  if (catalog.entries.length) {
    lines.push("| 插件 | 版本 | 最低宿主版本 |", "| --- | --- | --- |");
    for (const {
      package: { manifest },
    } of catalog.entries)
      lines.push(
        `| ${markdown(manifest.name)}（${markdown(manifest.id)}） | ${manifest.version} | ${manifest.minHostVersion} |`,
      );
  } else lines.push("本次目录不收录插件。");
  lines.push(
    "",
    "## 使用与验证",
    "",
    "在配套原生宿主中刷新扩展市场，审阅版本与权限后安装。目录签名与包完整性由宿主核验，安装不会自动授予新增权限。最低版本相同的开发快照仍须确认包含插件所需能力。",
    "",
    `[构建与离线检查流程](${base}/blob/${sourceSha}/.github/workflows/publish.yml)与[发布验收边界](${base}/blob/${sourceSha}/docs/publishing.md#发布前验收)可供核对；自动生成的变更说明不代表已完成真实桌面、多机网络或全部目标平台验收。`,
    "",
    "## 完整变更",
    "",
    previousTag
      ? `[${previousTag}...market-${catalog.revision}](${base}/compare/${previousTag}...${sourceSha})`
      : `[全部提交](${base}/commits/${sourceSha})`,
    "",
  );
  const notes = lines.join("\n");
  if (Buffer.byteLength(notes) > 120_000)
    throw new Error(
      "Release notes exceed 120,000 bytes; review and reduce the release scope",
    );
  return notes;
}

export function createReleaseNotes(catalog, sourceSha, releases, git = runGit) {
  const previousTag = previousReleaseTag(releases);
  return renderReleaseNotes(
    catalog,
    sourceSha,
    previousTag,
    readReleaseCommits(sourceSha, previousTag, git),
  );
}
