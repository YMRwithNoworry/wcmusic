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
function assertVersion(cwd, version) {
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.toml'), 'utf8').includes(`version = "${version}"`));
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.lock'), 'utf8').includes(`version = "${version}"`));
  assert.equal(JSON.parse(readFileSync(join(cwd, 'desktop/package.json'))).version, version);
  const lock = JSON.parse(readFileSync(join(cwd, 'desktop/package-lock.json')));
  assert.equal(lock.version, version); assert.equal(lock.packages[''].version, version);
  assert.equal(lock.packages['node_modules/test'].version, '9.0.0');
  assert.match(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), new RegExp(`version: ${version.replaceAll('.', '\\.')}\\+47`));
}

test('old branches advance past the latest release and synchronize all manifests', () => fixture(cwd => {
  manifests(cwd, '1.2.9');
  execute(cwd, 'scripts/release/bump-version.mjs', ['patch', 'v1.2.46']);
  assertVersion(cwd, '1.2.47');
  assert.match(readFileSync(join(cwd, 'outputs'), 'utf8'), /previous=1\.2\.46/);
}));

test('a branch ahead of latest keeps its version ordering', () => fixture(cwd => {
  manifests(cwd, '1.3.0');
  execute(cwd, 'scripts/release/bump-version.mjs', ['minor', 'v1.2.99']);
  assertVersion(cwd, '1.4.0');
}));

test('none rebuilds the current version without changing the Android build number', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', ['none', 'v1.2.47']);
  assert.equal(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), 'name: wcmusic\nversion: 1.2.46+46\n');
}));

test('invalid release tags fail before modifying files', () => fixture(cwd => {
  manifests(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', ['patch', 'invalid'], 1);
  assert.ok(readFileSync(join(cwd, 'desktop/src-tauri/Cargo.toml'), 'utf8').includes('1.2.46'));
}));

function site(cwd) {
  for (const file of ['site/index.html', 'site/app.js']) writeFileSync(join(cwd, file), readFileSync(resolve(root, file)));
}

test('site and notes target the new Windows release and retain the last available APK', () => fixture(cwd => {
  site(cwd);
  const apk = 'https://github.com/YMRwithNoworry/wcmusic/releases/download/v1.2.45/wcmusic-android-arm64.apk';
  execute(cwd, 'scripts/release/sync-site.mjs', ['--version', '1.2.47', '--android', 'false', '--android-url', apk, '--notes', join(cwd, 'notes.md')]);
  const html = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  assert.ok(html.includes('v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(html.includes('Windows 1.2.47 · Android 1.2.45'));
  assert.ok(html.includes('WCMusic 1.2.47</p>'));
  assert.ok(html.includes(apk));
  assert.ok(!html.includes('v1.2.47/wcmusic-android-arm64.apk'));
  assert.ok(readFileSync(join(cwd, 'site/app.js'), 'utf8').includes(apk));
  assert.ok(readFileSync(join(cwd, 'notes.md'), 'utf8').includes('v1.2.47/wcmusic-windows-x64.zip'));
  // 发布同步不能改写旧截图上实际拍摄的版本号或移除软件图标。
  assert.ok(html.includes('assets/wcmusic-desktop-v1.2.46.png'));
  assert.ok(html.includes('img class="brand-mark"'));
}));

test('successful Android builds update both platform links', () => fixture(cwd => {
  site(cwd);
  execute(cwd, 'scripts/release/sync-site.mjs', ['--version', '1.2.48', '--android', 'true']);
  const html = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  assert.ok(html.includes('Windows 1.2.48 · Android 1.2.48'));
  assert.ok(html.includes('v1.2.48/wcmusic-android-arm64.apk'));
  assert.ok(readFileSync(join(cwd, 'site/app.js'), 'utf8').includes('v1.2.48/wcmusic-android-arm64.apk'));
}));
