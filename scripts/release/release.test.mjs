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

const windows = ['--platform', 'windows', '--tag-prefix', 'v'];
const android = ['--platform', 'android', '--tag-prefix', 'android-v'];

test('windows advances past its own latest release and synchronizes all manifests', () => fixture(cwd => {
  manifests(cwd, '1.2.9');
  execute(cwd, 'scripts/release/bump-version.mjs', [...windows, 'patch', 'v1.2.46']);
  assertVersion(cwd, '1.2.47');
  const out = outputs(cwd);
  assert.equal(out.previous, '1.2.46');
  assert.equal(out.tag, 'v1.2.47');
  assert.equal(out.build, 'true');
}));

test('windows only compares against the windows tag line, never the android one', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  // 传进来的 latest 已由 workflow 按前缀筛过，这里模拟只有 android 线的情况（空串）。
  execute(cwd, 'scripts/release/bump-version.mjs', [...windows, 'patch', '']);
  assertVersion(cwd, '1.2.47');
  assert.equal(outputs(cwd).tag, 'v1.2.47');
}));

test('android bumps its own line with the android-v tag prefix', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', [...android, 'minor', 'android-v1.2.90']);
  const out = outputs(cwd);
  assert.equal(out.version, '1.3.0');
  assert.equal(out.previous, '1.2.90');
  assert.equal(out.tag, 'android-v1.3.0');
  assert.equal(out.build, 'true');
  assert.equal(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), 'name: wcmusic\nversion: 1.3.0+47\n');
  // Android 线不改动旧 Tauri 清单。
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.toml'), 'utf8').includes('1.2.46'));
}));

test('a branch ahead of latest keeps its version ordering', () => fixture(cwd => {
  manifests(cwd, '1.3.0');
  execute(cwd, 'scripts/release/bump-version.mjs', [...windows, 'minor', 'v1.2.99']);
  assertVersion(cwd, '1.4.0');
}));

test('none rebuilds the current version without changing the Android build number', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', [...windows, 'none', 'v1.2.47']);
  assert.equal(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), 'name: wcmusic\nversion: 1.2.46+46\n');
}));

test('an unselected platform reports its current version and changes nothing', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', [...android, '--target', 'windows', 'patch', 'android-v1.2.46']);
  const out = outputs(cwd);
  assert.equal(out.build, 'false');
  assert.equal(out.version, '1.2.46');
  assert.equal(out.tag, 'android-v1.2.46');
  assert.equal(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), 'name: wcmusic\nversion: 1.2.46+46\n');
}));

test('invalid release tags fail before modifying files', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', [...windows, 'patch', 'invalid'], 1);
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.toml'), 'utf8').includes('1.2.46'));
}));

function site(cwd) {
  for (const file of ['site/index.html', 'site/app.js']) writeFileSync(join(cwd, file), readFileSync(resolve(root, file)));
}

test('site points each platform at its own version line', () => fixture(cwd => {
  site(cwd);
  execute(cwd, 'scripts/release/sync-site.mjs', [
    '--platform', 'site',
    '--windows-version', '1.2.47', '--windows-previous', '1.2.46',
    '--android-version', '1.2.40', '--android-previous', '1.2.39',
    '--notes', join(cwd, 'notes.md'),
  ]);
  const html = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  const app = readFileSync(join(cwd, 'site/app.js'), 'utf8');
  const base = 'https://github.com/YMRwithNoworry/wcmusic/releases/download/';
  // 必须断言完整地址：只比后半个路径会漏掉「把域名整段删掉」的坏链。
  assert.ok(html.includes(base + 'v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(html.includes(base + 'android-v1.2.40/wcmusic-android-arm64.apk'));
  assert.ok(app.includes(base + 'android-v1.2.40/wcmusic-android-arm64.apk'));
  assert.ok(!html.includes('href="/releases/download/'));
  assert.ok(!app.includes("= '/releases/download/"));
  assert.ok(html.includes('Windows 1.2.47 · Android 1.2.40'));
  assert.ok(html.includes('WCMusic 1.2.47</p>'));
  // 发布同步不能改写旧截图上实际拍摄的版本号或移除软件图标。
  assert.ok(html.includes('assets/wcmusic-desktop-v1.2.46.png'));
  assert.ok(html.includes('img class="brand-mark"'));
}));

test('android release notes only list the apk', () => fixture(cwd => {
  site(cwd);
  const before = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  execute(cwd, 'scripts/release/sync-site.mjs', [
    '--platform', 'android', '--version', '1.2.40', '--previous', '1.2.39', '--notes', join(cwd, 'notes.md'),
  ]);
  const notes = readFileSync(join(cwd, 'notes.md'), 'utf8');
  assert.ok(notes.includes('android-v1.2.40/wcmusic-android-arm64.apk'));
  assert.ok(!notes.includes('wcmusic-windows-x64.zip'));
  // 只生成说明时不能改动官网。
  assert.equal(readFileSync(join(cwd, 'site/index.html'), 'utf8'), before);
}));
