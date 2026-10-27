import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  createReleaseNotes,
  previousReleaseTag,
  readReleaseCommits,
  renderReleaseNotes,
} from "./release-notes.mjs";

const catalog = {
  revision: 50,
  repository: "owner/market",
  schemaVersion: 2,
  issuedAt: 1_700_000_000,
  expiresAt: 1_700_003_600,
  entries: [
    {
      package: {
        manifest: {
          name: "计算器",
          id: "example.calculator",
          version: "1.2.0",
          minHostVersion: "0.1.7",
        },
      },
    },
  ],
};
const sha = "a".repeat(40);

test("release baseline is the highest public stable market revision", () => {
  assert.equal(
    previousReleaseTag([
      { tag_name: "market-99", draft: true },
      { tag_name: "market-98", draft: false, prerelease: true },
      { tag_name: "market-10", draft: false, prerelease: false },
      { tag_name: "v100", draft: false },
      { tag_name: "market-9", draft: false },
    ]),
    "market-10",
  );
  assert.equal(previousReleaseTag([]), undefined);
});

test("fixed release sections classify commits and surface breaking changes", () => {
  const commits = [
    "feat(calc): 添加日期计算",
    "fix(lan): 修复回执竞态",
    "perf(calc): 减少解析分配",
    "refactor(ui): 简化布局",
    "docs: 补充插件开发指南",
    "build(deps): 更新锁文件",
    "ci: 固定宿主来源",
    "test: 覆盖未来状态",
    "chore: 更新版本",
    "style: 调整格式",
    "revert: 回退错误变更",
    "无类型的历史提交",
    "feat(protocol)!: 调整会话协议\n\nBREAKING CHANGE: 导出旧会话后再升级\n  保留原始数据\nRefs: #42",
    "fix(storage): 变更存储协议\n\nBREAKING-CHANGE: 需要显式重新导入",
  ].map((message, index) => ({
    sha: String(index).padStart(40, "0"),
    message,
  }));
  const notes = renderReleaseNotes(catalog, sha, "market-10", commits);
  for (const heading of [
    "发布信息",
    "破坏性变更",
    "更新内容",
    "插件版本",
    "使用与验证",
    "完整变更",
  ])
    assert.ok(notes.includes(`## ${heading}\n`));
  for (const type of [
    "feat",
    "fix",
    "perf",
    "refactor",
    "docs",
    "build",
    "ci",
    "test",
    "chore",
    "style",
    "revert",
    "other",
  ])
    assert.ok(notes.includes(`（${type}）`), type);
  assert.ok(
    notes.indexOf("导出旧会话后再升级 保留原始数据") <
      notes.indexOf("## 更新内容"),
  );
  assert.ok(notes.includes("需要显式重新导入"));
  assert.ok(!notes.includes("Refs: #42"));
  assert.ok(notes.includes("| 计算器（example.calculator） | 1.2.0 | 0.1.7 |"));
  assert.ok(notes.includes("2023-11-14T22:13:20.000Z"));
  assert.ok(notes.includes(`/compare/market-10...${sha}`));
  assert.ok(notes.includes(`/commit/${commits[0].sha}`));
});

test("renewals, initial releases and Markdown content retain clear meaning", () => {
  const notes = renderReleaseNotes(catalog, sha, "market-10", []);
  assert.ok(notes.includes("本次范围没有新增的非合并提交"));
  assert.ok(notes.includes("本次提交未声明破坏性变更"));
  const initial = renderReleaseNotes(
    { ...catalog, entries: [] },
    sha,
    undefined,
    [
      {
        sha,
        message: "feat(ui): 显示 [链接](https://example.test) <script> |",
      },
    ],
  );
  assert.ok(initial.includes("首次正式发布"));
  assert.ok(initial.includes(`/commits/${sha}`));
  assert.ok(initial.includes("本次目录不收录插件"));
  assert.ok(
    initial.includes("\\[链接\\]\\(https://example.test\\) \\<script\\> \\|"),
  );
  assert.throws(
    () =>
      renderReleaseNotes(catalog, sha, undefined, [
        {
          sha,
          message: "feat: " + "x".repeat(120_000),
        },
      ]),
    /Release notes exceed/,
  );
});

test("real git history includes underlying commits once and supports annotated tags and renewals", async (t) => {
  const directory = await mkdtemp(join(tmpdir(), "starter-market-notes-test-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const git = (args) =>
    execFileSync("git", args, {
      cwd: directory,
      encoding: "utf8",
      windowsHide: true,
    });
  git(["init", "--initial-branch=main"]);
  git(["config", "user.name", "Release fixture"]);
  git(["config", "user.email", "release@example.test"]);
  git(["commit", "--allow-empty", "-m", "feat: 初始版本"]);
  const first = git(["rev-parse", "HEAD"]).trim();
  git(["tag", "-a", "market-10", "-m", "目录 10"]);
  git(["switch", "-c", "feature/calc"]);
  git(["commit", "--allow-empty", "-m", "feat(calc): 添加日期计算"]);
  git(["commit", "--allow-empty", "-m", "fix(calc): 修复闰年"]);
  git(["switch", "main"]);
  git(["merge", "--no-ff", "feature/calc", "-m", "Merge pull request #2"]);
  const head = git(["rev-parse", "HEAD"]).trim();
  assert.deepEqual(
    readReleaseCommits(head, "market-10", git).map((commit) => commit.message),
    ["feat(calc): 添加日期计算", "fix(calc): 修复闰年"],
  );
  assert.equal(readReleaseCommits(head, undefined, git).length, 3);
  git(["tag", "market-20"]);
  assert.deepEqual(readReleaseCommits(head, "market-20", git), []);
  assert.throws(() => readReleaseCommits(first, "market-20", git));
  assert.throws(() => readReleaseCommits(head, "market-999", git));
  assert.throws(
    () =>
      readReleaseCommits(head, undefined, (args) =>
        args[1] === "--is-shallow-repository" ? "true" : git(args),
      ),
    /complete Git history/,
  );
  const notes = createReleaseNotes(
    catalog,
    head,
    [{ tag_name: "market-10", draft: false }],
    git,
  );
  assert.ok(notes.includes("添加日期计算"));
  assert.ok(!notes.includes("Merge pull request"));
  assert.ok(!notes.includes("初始版本"));
});
