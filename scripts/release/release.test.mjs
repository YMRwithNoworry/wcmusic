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
  mkdirSync(join(cwd, 'site'), { recursive: true });
  try { run(cwd); } finally { rmSync(cwd, { recursive: true, force: true }); }
}
function execute(cwd, script, args, expected = 0) {
  const result = spawnSync(process.execPath, [resolve(root, script), ...args], {
    cwd, encoding: 'utf8', env: { ...process.env, GITHUB_OUTPUT: join(cwd, 'outputs') },
  });
  assert.equal(result.status, expected, result.stdout + result.stderr);
  return result;
}
function manifest(cwd, version, build = 46) {
  writeFileSync(join(cwd, 'pubspec.yaml'), `name: wcmusic\nversion: ${version}+${build}\n`);
}
function assertVersion(cwd, version) {
  assert.match(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), new RegExp(`version: ${version.replaceAll('.', '\\.')}\\+47`));
}

test('old branches advance past the latest release and update Flutter build number', () => fixture(cwd => {
  manifest(cwd, '1.2.9');
  execute(cwd, 'scripts/release/bump-version.mjs', ['patch', 'v1.2.46']);
  assertVersion(cwd, '1.2.47');
  assert.match(readFileSync(join(cwd, 'outputs'), 'utf8'), /previous=1\.2\.46/);
}));

test('a branch ahead of latest keeps its version ordering', () => fixture(cwd => {
  manifest(cwd, '1.3.0');
  execute(cwd, 'scripts/release/bump-version.mjs', ['minor', 'v1.2.99']);
  assertVersion(cwd, '1.4.0');
}));

test('none rebuilds the current version without changing Android build number', () => fixture(cwd => {
  manifest(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', ['none', 'v1.2.47']);
  assert.equal(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), 'name: wcmusic\nversion: 1.2.46+46\n');
}));

test('invalid release tags fail before modifying the Flutter manifest', () => fixture(cwd => {
  manifest(cwd, '1.2.46');
  execute(cwd, 'scripts/release/bump-version.mjs', ['patch', 'invalid'], 1);
  assert.equal(readFileSync(join(cwd, 'pubspec.yaml'), 'utf8'), 'name: wcmusic\nversion: 1.2.46+46\n');
}));
function site(cwd) {
  for (const file of ['site/index.html', 'site/app.js']) writeFileSync(join(cwd, file), readFileSync(resolve(root, file)));
}

test('site and notes target both Flutter artifacts from the current release', () => fixture(cwd => {
  site(cwd);
  execute(cwd, 'scripts/release/sync-site.mjs', ['--version', '1.2.47', '--notes', join(cwd, 'notes.md')]);
  const html = readFileSync(join(cwd, 'site/index.html'), 'utf8');
  const android = 'https://github.com/YMRwithNoworry/wcmusic/releases/download/v1.2.47/wcmusic-android-arm64.apk';
  assert.ok(html.includes('v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(html.includes('Windows 1.2.47 · Android 1.2.47'));
  assert.ok(html.includes('WCMusic 1.2.47</p>'));
  assert.ok(html.includes(android));
  assert.ok(readFileSync(join(cwd, 'site/app.js'), 'utf8').includes(android));
  const notes = readFileSync(join(cwd, 'notes.md'), 'utf8');
  assert.ok(notes.includes('v1.2.47/wcmusic-windows-x64.zip'));
  assert.ok(notes.includes('v1.2.47/wcmusic-android-arm64.apk'));
  // 发布同步不能改写旧截图上实际拍摄的版本号或移除软件图标。
  assert.ok(html.includes('assets/wcmusic-desktop-v1.2.46.png'));
  assert.ok(html.includes('img class="brand-mark"'));
}));
