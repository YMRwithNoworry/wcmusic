import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const root = fileURLToPath(new URL('../../', import.meta.url));
function fixture(run) {
  const cwd = mkdtempSync(join(tmpdir(), 'wcmusic-release-test-'));
  for (const dir of ['desktop/src-tauri', 'site']) mkdirSync(join(cwd, dir), { recursive: true });
  try { run(cwd); } finally { rmSync(cwd, { recursive: true, force: true }); }
}
function execute(cwd, script, args, expected = 0) {
  const result = spawnSync(process.execPath, [resolve(root, script), ...args], {
    cwd, encoding: 'utf8', env: { ...process.env, GITHUB_OUTPUT: join(cwd, 'outputs') },
  });
  assert.equal(result.status, expected, result.stdout + result.stderr);
  return result;
}
function manifests(cwd, version) {
  writeFileSync(join(cwd, 'desktop/src-tauri/Cargo.toml'), `[package]\nname = "wcmusic-desktop"\nversion = "${version}"\n`);
  writeFileSync(join(cwd, 'desktop/src-tauri/Cargo.lock'), `[[package]]\nname = "wcmusic-desktop"\nversion = "${version}"\n`);
  writeFileSync(join(cwd, 'desktop/package.json'), JSON.stringify({ name: 'wcmusic-desktop', version }, null, 2));
  writeFileSync(join(cwd, 'desktop/package-lock.json'), JSON.stringify({ version, packages: { '': { version }, 'node_modules/test': { version: '9.0.0' } } }));
  writeFileSync(join(cwd, 'pubspec.yaml'), `name: wcmusic\nversion: ${version}+46\n`);
}
function outputs(cwd) {
  return Object.fromEntries(
    readFileSync(join(cwd, 'outputs'), 'utf8').trim().split('\n').map((line) => line.split('=')),
  );
}
function assertVersion(cwd, version) {
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.toml'), 'utf8').includes(`version = "${version}"`));
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.lock'), 'utf8').includes(`version = "${version}"`));
  assert.equal(JSON.parse(readFileSync(join(cwd, 'desktop/package.json'))).version, version);
  const lock = JSON.parse(readFileSync(join(cwd, 'desktop/package-lock.json')));
  assert.equal(lock.version, version); assert.equal(lock.packages[''].version, version);
  assert.equal(lock.packages['node_modules/test'].version, '9.0.0');
  assert.match(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), new RegExp(`version: ${version.replaceAll('.', '\\.')}\\+47`));
}

const merged = ["--tag-prefix", "v"];

test("merged release advances past the latest published vX.Y.Z and synchronizes all manifests", () => fixture(cwd => {
  manifests(cwd, "1.2.9");
  execute(cwd, "scripts/release/bump-version.mjs", [...merged, "patch", "v1.2.46"]);
  assertVersion(cwd, "1.2.47");
  const out = outputs(cwd);
  assert.equal(out.previous, "1.2.46");
  assert.equal(out.version, "1.2.47");
  assert.equal(out.tag, "v1.2.47");
  assert.equal(out.build, "true");
}));

test("first merged release works without any previous tag", () => fixture(cwd => {
  manifests(cwd, "1.2.46");
  execute(cwd, "scripts/release/bump-version.mjs", [...merged, "patch", ""]);
  assertVersion(cwd, "1.2.47");
  assert.equal(outputs(cwd).tag, "v1.2.47");
}));

test("merged release bumps both windows and android manifests together", () => fixture(cwd => {
  manifests(cwd, "1.2.46");
  // Android 构建号来自 pubspec 的 +N，Windows 版本来自同一行 version:
  execute(cwd, "scripts/release/bump-version.mjs", [...merged, "minor", "v1.2.46"]);
  const out = outputs(cwd);
  assert.equal(out.version, "1.3.0");
  assert.equal(out.tag, "v1.3.0");
  // 两端共用同一个版本号：旧 Tauri 清单也一起更新
  assertVersion(cwd, "1.3.0");
}));

test("a branch ahead of latest keeps its version ordering", () => fixture(cwd => {
  manifests(cwd, "1.3.0");
  execute(cwd, "scripts/release/bump-version.mjs", [...merged, "minor", "v1.2.99"]);
  assertVersion(cwd, "1.4.0");
}));

test("none rebuilds the current version without changing the build number", () => fixture(cwd => {
  manifests(cwd, "1.2.46");
  execute(cwd, "scripts/release/bump-version.mjs", [...merged, "none", "v1.2.46"]);
  assert.equal(readFileSync(join(cwd, "pubspec.yaml"), "utf8"), "name: wcmusic\nversion: 1.2.46+46\n");
}));


test("invalid release tags fail before modifying files", () => fixture(cwd => {
  manifests(cwd, "1.2.46");
  execute(cwd, "scripts/release/bump-version.mjs", [...merged, "patch", "invalid"], 1);
  assert.ok(readFileSync(join(cwd, "desktop/src-tauri/Cargo.toml"), "utf8").includes("1.2.46"));
}));

function site(cwd) {
  for (const file of ['site/index.html', 'site/app.js']) writeFileSync(join(cwd, file), readFileSync(resolve(root, file)));
}

test('site points both platforms at the shared release tag', () => fixture(cwd => {
  site(cwd);
  execute(cwd, 'scripts/release/sync-site.mjs', [
    '--platform', 'site',
    '--version', '1.2.47', '--previous', '1.2.46',
    '--notes', join(cwd, 'notes.md'),
  ]);
  const html = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  const app = readFileSync(join(cwd, 'site/app.js'), 'utf8');
  const base = 'https://github.com/YMRwithNoworry/wcmusic/releases/download/';
  // 必须断言完整地址：只比后半个路径会漏掉「把域名整段删掉」的坏链。
  // 合并发布：两端都指向同一个 vX.Y.Z 的 Release。
  assert.ok(html.includes(base + 'v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(html.includes(base + 'v1.2.47/wcmusic-android-arm64.apk'));
  assert.ok(app.includes(base + 'v1.2.47/wcmusic-android-arm64.apk'));
  assert.ok(!html.includes('android-v1.2.47'));
  assert.ok(!html.includes('href="/releases/download/'));
  assert.ok(!app.includes("= '/releases/download/"));
  assert.ok(html.includes('Windows 1.2.47 · Android 1.2.47'));
  assert.ok(html.includes('WCMusic 1.2.47</p>'));
  // 把历史 android-v 链接一并迁移到合并发布
  assert.ok(!app.includes('android-v'));
  // 发布同步不能改写旧截图上实际拍摄的版本号或移除软件图标。
  assert.ok(html.includes('assets/wcmusic-desktop-v1.2.46.png'));
  assert.ok(html.includes('img class="brand-mark"'));
}));

test('merged release notes list both downloads and leave the site untouched', () => fixture(cwd => {
  site(cwd);
  const before = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  execute(cwd, 'scripts/release/sync-site.mjs', [
    '--version', '1.2.47', '--previous', '1.2.46', '--notes', join(cwd, 'notes.md'),
  ]);
  const notes = readFileSync(join(cwd, 'notes.md'), 'utf8');
  assert.ok(notes.includes('## WCMusic 1.2.47（Windows + Android）'));
  assert.ok(notes.includes('releases/download/v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(notes.includes('releases/download/v1.2.47/wcmusic-android-arm64.apk'));
  // 只生成说明时不能改动官网。
  assert.equal(readFileSync(join(cwd, 'site/index.html'), 'utf8'), before);
}));

test('legacy per-platform versions still resolve to their own tags', () => fixture(cwd => {
  site(cwd);
  execute(cwd, 'scripts/release/sync-site.mjs', [
    '--platform', 'site',
    '--windows-version', '1.2.47', '--android-version', '1.2.40',
  ]);
  const html = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  assert.ok(html.includes('releases/download/v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(html.includes('releases/download/android-v1.2.40/wcmusic-android-arm64.apk'));
}));

test('windows build records the release version inside the package', () => {
  // 应用「检查更新」靠包内的 rebuild_meta.json 认出真实安装版本：
  // 本地直接跑 pubspec 1.2.82，而 CI 用 --build-name 打出 1.2.83。
  const workflow = readFileSync(resolve(root, ".github/workflows/release.yml"), "utf8");
  assert.match(
    workflow,
    /flutter build windows --release --build-name=\$\{\{ needs\.version\.outputs\.version \}\}/,
    "Windows 构建必须把发布版本写进产物",
  );
  const release = readFileSync(resolve(root, "lib/data/services/release_service.dart"), "utf8");
  assert.ok(release.includes("rebuild_meta.json"));
  assert.ok(release.includes("--build-name"));
});

test('workflow publishes one combined release for both platforms', () => {
  const workflow = readFileSync(resolve(root, ".github/workflows/release.yml"), "utf8");
  const names = [...workflow.matchAll(/^\s{2}([a-z][a-z-]*):\s*$/gm)].map((m) => m[1]);
  // 只有一个 version / publish job，两端产物挂在同一个 Release 上
  assert.deepEqual(
    names.filter((name) => name === 'version' || name.startsWith('publish') || name.startsWith('version-')),
    ['version', 'publish'],
  );
  assert.ok(!workflow.includes('android-v'));
  assert.match(workflow, /gh release upload "[^"]*TAG[^"]*" \$assets/);
});
