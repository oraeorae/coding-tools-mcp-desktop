import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import {
  assertSingleVersion,
  exactAppPids,
  parseArgs,
  readProjectVersions,
  selectRestoreServices,
} from "./install-local-macos.mjs";

test("parseArgs accepts only supported flags", () => {
  assert.deepEqual(parseArgs(["--dry-run"]), { dryRun: true, skipChecks: false });
  assert.throws(() => parseArgs(["--replace-anything"]), /未知参数/);
});

test("assertSingleVersion rejects drift", () => {
  assert.equal(assertSingleVersion({ a: "1.2.3", b: "1.2.3" }), "1.2.3");
  assert.throws(() => assertSingleVersion({ a: "1.2.3", b: "1.2.4" }), /版本不一致/);
});

test("exactAppPids requires an exact executable path", () => {
  const executable = "/Applications/Coding Tools MCP.app/Contents/MacOS/coding-tools-mcp-desktop";
  const ps = ` 123 ${executable}\n 124 ${executable}-helper\n 125 /tmp/coding-tools-mcp-desktop`;
  assert.deepEqual(exactAppPids(ps, executable), [123]);
});

test("selectRestoreServices includes only owned configured ports", () => {
  const profiles = [
    { id: "one", runtime: { local_port: 28766 }, actions: { local_port: 8788 } },
    { id: "two", runtime: { local_port: 28767 }, actions: { local_port: 8789 } },
  ];
  assert.deepEqual(selectRestoreServices(profiles, new Set([28766, 8789])), [
    { workspaceId: "one", mcp: true, actions: false },
    { workspaceId: "two", mcp: false, actions: true },
  ]);
});

test("readProjectVersions reads all five version sources", async () => {
  const root = await mkdtemp(path.join(tmpdir(), "coding-tools-version-"));
  try {
    await mkdir(path.join(root, "src-tauri"));
    await writeFile(path.join(root, "package.json"), '{"version":"1.2.3"}');
    await writeFile(path.join(root, "package-lock.json"), '{"version":"1.2.3","packages":{"":{"version":"1.2.3"}}}');
    await writeFile(path.join(root, "src-tauri/Cargo.toml"), '[package]\nversion = "1.2.3"\n');
    await writeFile(path.join(root, "src-tauri/Cargo.lock"), 'name = "coding-tools-mcp-desktop"\nversion = "1.2.3"\n');
    await writeFile(path.join(root, "src-tauri/tauri.conf.json"), '{"version":"1.2.3"}');
    assert.deepEqual(await readProjectVersions(root), {
      packageJson: "1.2.3",
      packageLock: "1.2.3",
      packageLockRoot: "1.2.3",
      cargoToml: "1.2.3",
      cargoLock: "1.2.3",
      tauriConfig: "1.2.3",
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
