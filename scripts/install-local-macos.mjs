#!/usr/bin/env node

import { createHash } from "node:crypto";
import { execFile, spawnSync } from "node:child_process";
import { promises as fs, constants as fsConstants } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { promisify } from "node:util";

const exec = promisify(execFile);
const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const appName = "Coding Tools MCP.app";
const defaultTarget = `/Applications/${appName}`;
const defaultDataDir = path.join(
  homedir(),
  "Library/Application Support/coding-tools-mcp-desktop/data",
);

export function parseArgs(argv) {
  const known = new Set(["--dry-run", "--skip-checks"]);
  for (const arg of argv) {
    if (!known.has(arg)) throw new Error(`未知参数: ${arg}`);
  }
  return { dryRun: argv.includes("--dry-run"), skipChecks: argv.includes("--skip-checks") };
}

function tomlPackageVersion(text, packageName) {
  if (!packageName) return text.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  const escaped = packageName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  return text.match(new RegExp(`name = "${escaped}"\\nversion = "([^"]+)"`))?.[1];
}

export async function readProjectVersions(root) {
  const [pkgText, lockText, cargoText, cargoLockText, tauriText] = await Promise.all([
    fs.readFile(path.join(root, "package.json"), "utf8"),
    fs.readFile(path.join(root, "package-lock.json"), "utf8"),
    fs.readFile(path.join(root, "src-tauri/Cargo.toml"), "utf8"),
    fs.readFile(path.join(root, "src-tauri/Cargo.lock"), "utf8"),
    fs.readFile(path.join(root, "src-tauri/tauri.conf.json"), "utf8"),
  ]);
  const pkg = JSON.parse(pkgText);
  const lock = JSON.parse(lockText);
  const tauri = JSON.parse(tauriText);
  return {
    packageJson: pkg.version,
    packageLock: lock.version,
    packageLockRoot: lock.packages?.[""]?.version,
    cargoToml: tomlPackageVersion(cargoText),
    cargoLock: tomlPackageVersion(cargoLockText, "coding-tools-mcp-desktop"),
    tauriConfig: tauri.version,
  };
}

export function assertSingleVersion(versions) {
  const values = Object.values(versions);
  if (values.some((value) => !value) || new Set(values).size !== 1) {
    throw new Error(`应用版本不一致: ${JSON.stringify(versions)}`);
  }
  return values[0];
}

export function exactAppPids(psOutput, executable) {
  return psOutput
    .split("\n")
    .map((line) => line.match(/^\s*(\d+)\s+(.+)$/))
    .filter((match) => match && match[2] === executable)
    .map((match) => Number(match[1]));
}

export function selectRestoreServices(profiles, ownedPorts) {
  return profiles.flatMap((profile) => {
    const mcp = ownedPorts.has(Number(profile.runtime?.local_port));
    const actions = ownedPorts.has(Number(profile.actions?.local_port));
    return mcp || actions ? [{ workspaceId: profile.id, mcp, actions }] : [];
  });
}

async function run(command, args, options = {}) {
  console.log(`→ ${command} ${args.join(" ")}`);
  await new Promise((resolve, reject) => {
    const child = spawnSync(command, args, { cwd: projectRoot, stdio: "inherit", ...options });
    if (child.error) reject(child.error);
    else if (child.status !== 0) reject(new Error(`${command} 退出码 ${child.status}`));
    else resolve();
  });
}

async function capture(command, args) {
  const { stdout } = await exec(command, args, { cwd: projectRoot, maxBuffer: 4 * 1024 * 1024 });
  return stdout.trim();
}

async function sha256(file) {
  return createHash("sha256").update(await fs.readFile(file)).digest("hex");
}

async function installedVersion(appPath) {
  return capture("/usr/libexec/PlistBuddy", [
    "-c", "Print :CFBundleShortVersionString", path.join(appPath, "Contents/Info.plist"),
  ]);
}

async function listeningPids(port) {
  if (!Number.isInteger(port) || port < 1 || port > 65535) return [];
  try {
    const stdout = await capture("/usr/sbin/lsof", ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"]);
    return stdout.split(/\s+/).filter(Boolean).map(Number);
  } catch (error) {
    if (error.code === 1) return [];
    throw error;
  }
}

async function discoverState(target, dataDir) {
  const executable = path.join(target, "Contents/MacOS/coding-tools-mcp-desktop");
  const psOutput = await capture("/bin/ps", ["-axo", "pid=,comm="]);
  const appPids = exactAppPids(psOutput, executable);
  const configPath = path.join(dataDir, "profiles.json");
  let config = null;
  try {
    config = JSON.parse(await fs.readFile(configPath, "utf8"));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const ownedPorts = new Set();
  for (const profile of config?.profiles ?? []) {
    for (const port of [Number(profile.runtime?.local_port), Number(profile.actions?.local_port)]) {
      const pids = await listeningPids(port);
      if (pids.some((pid) => appPids.includes(pid))) ownedPorts.add(port);
    }
  }
  return {
    appPids,
    configPath,
    configHash: config ? await sha256(configPath) : null,
    services: selectRestoreServices(config?.profiles ?? [], ownedPorts),
  };
}

async function waitForExit(pids, timeoutMs = 10000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const psOutput = await capture("/bin/ps", ["-axo", "pid=,comm="]);
    const live = new Set(psOutput.split("\n").map((line) => Number(line.trim().split(/\s+/, 1)[0])));
    if (pids.every((pid) => !live.has(pid))) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`应用未在 ${timeoutMs / 1000} 秒内退出`);
}

async function waitForFile(file, timeoutMs = 20000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try { return JSON.parse(await fs.readFile(file, "utf8")); }
    catch (error) { if (error.code !== "ENOENT") throw error; }
    await new Promise((resolve) => setTimeout(resolve, 200));
  }
  throw new Error("等待服务恢复结果超时");
}

async function ensureCleanTrackedTree() {
  const output = await capture("git", ["status", "--porcelain", "--untracked-files=no"]);
  if (output) throw new Error("存在未提交的已跟踪文件；请先提交或还原后再升级。\n" + output);
}

async function main() {
  if (process.platform !== "darwin") throw new Error("install:local 当前只支持 macOS。");
  const options = parseArgs(process.argv.slice(2));
  const target = process.env.CODING_TOOLS_INSTALL_TARGET || defaultTarget;
  const dataDir = process.env.CODING_TOOLS_DATA_DIR || defaultDataDir;
  const versions = await readProjectVersions(projectRoot);
  const version = assertSingleVersion(versions);
  await ensureCleanTrackedTree();
  const before = await discoverState(target, dataDir);
  console.log(`本地升级目标: v${version}`);
  console.log(`当前运行服务: ${before.services.reduce((n, item) => n + Number(item.mcp) + Number(item.actions), 0)} 个`);
  if (options.dryRun) {
    console.log("dry-run 通过：未构建、未停止应用、未修改任何文件。");
    return;
  }

  if (!options.skipChecks) {
    await run("npm", ["run", "check"]);
    await run("cargo", ["check", "--manifest-path", "src-tauri/Cargo.toml"]);
  }
  await run("npm", ["run", "desktop:build", "--", "--bundles", "app"]);

  const source = path.join(projectRoot, "src-tauri/target/release/bundle/macos", appName);
  if (await installedVersion(source) !== version) throw new Error("构建 App 版本与源码版本不一致");
  await run("/usr/bin/codesign", ["--force", "--deep", "--sign", "-", source]);
  await run("/usr/bin/codesign", ["--verify", "--deep", "--strict", source]);
  await fs.access(path.dirname(target), fsConstants.W_OK);

  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const backupRoot = process.env.CODING_TOOLS_BACKUP_ROOT || path.join(dataDir, "update-backups");
  const backupDir = path.join(backupRoot, `${stamp}-to-${version}`);
  const stage = path.join(path.dirname(target), `.Coding Tools MCP-${version}-${process.pid}.staged.app`);
  const previous = path.join(path.dirname(target), `.Coding Tools MCP-${process.pid}.previous.app`);
  const planPath = path.join(dataDir, "local-update-restore.json");
  const resultPath = path.join(dataDir, "local-update-restore-result.json");
  await fs.mkdir(backupDir, { recursive: true, mode: 0o700 });
  if (before.configHash) {
    await fs.copyFile(before.configPath, path.join(backupDir, "profiles.json"));
    await fs.chmod(path.join(backupDir, "profiles.json"), 0o600);
  }
  let swapped = false;
  try {
    await run("/usr/bin/ditto", [source, stage]);
    await fs.rm(resultPath, { force: true });
    for (const pid of before.appPids) process.kill(pid, "SIGTERM");
    await waitForExit(before.appPids);
    if (before.services.length) {
      await fs.writeFile(planPath, JSON.stringify({ targetVersion: version, services: before.services }, null, 2), { mode: 0o600 });
    } else {
      await fs.rm(planPath, { force: true });
    }
    await fs.rename(target, previous);
    try {
      await fs.rename(stage, target);
      swapped = true;
    } catch (error) {
      await fs.rename(previous, target);
      throw error;
    }
    await run("/usr/bin/codesign", ["--verify", "--deep", "--strict", target]);
    if (await installedVersion(target) !== version) throw new Error("安装后的版本校验失败");
    if (before.configHash && await sha256(before.configPath) !== before.configHash) {
      throw new Error("应用配置在升级期间发生变化");
    }
    await run("/usr/bin/ditto", [previous, path.join(backupDir, "previous.app")]);
    await fs.rm(previous, { recursive: true, force: true });
    await run("/usr/bin/open", [target]);
  } catch (error) {
    await fs.rm(planPath, { force: true });
    if (swapped && await fs.stat(previous).then(() => true, () => false)) {
      const failed = path.join(backupDir, "failed-new.app");
      await fs.rename(target, failed);
      await fs.rename(previous, target);
      await run("/usr/bin/open", [target]);
    }
    throw error;
  } finally {
    await fs.rm(stage, { recursive: true, force: true });
  }

  if (before.services.length) {
    const result = await waitForFile(resultPath);
    if (result.targetVersion !== version || !result.success) {
      throw new Error(`服务恢复失败: ${JSON.stringify(result)}`);
    }
    const after = await discoverState(target, dataDir);
    const expected = before.services.reduce((n, item) => n + Number(item.mcp) + Number(item.actions), 0);
    const actual = after.services.reduce((n, item) => n + Number(item.mcp) + Number(item.actions), 0);
    if (actual !== expected) throw new Error(`服务监听恢复不完整: ${actual}/${expected}`);
  }
  if (before.configHash && await sha256(before.configPath) !== before.configHash) {
    throw new Error("服务恢复后应用配置哈希发生变化");
  }

  console.log(`✓ 已安装 Coding Tools MCP v${version}`);
  console.log(`✓ 配置保持不变；备份位于 ${backupDir}`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`本地升级失败: ${error.message}`);
    process.exitCode = 1;
  });
}
