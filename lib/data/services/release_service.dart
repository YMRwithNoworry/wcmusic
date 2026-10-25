import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../../domain/models/release_info.dart';

/// 访问 GitHub Releases 的抽象，测试中可注入假实现。
abstract interface class ReleaseService {
  /// 拉取仓库最新的正式发布（优先使用与当前平台匹配的 tag 前缀）。
  Future<ReleaseInfo> fetchLatestRelease();

  /// 当前运行版本（X.Y.Z），无法确定时返回 null。
  ///
  /// 实现会综合显式版本、pubspec 与上次更新留下的记录，GitHub 上的版本高于它
  /// 就提示更新。
  Future<String?> currentVersionName();

  /// 把 [url] 下载到 [target]；[onProgress] 回报 0~1 的进度（总大小未知时为 null）。
  Future<void> download(
    String url,
    File target, {
    void Function(double? progress)? onProgress,
  });

  /// 「一键更新」：下载发布产物并安装。返回为空表示已在后台开始替换，需要退出当前进程。
  Future<UpdateInstallResult?> installUpdate(
    ReleaseInfo release, {
    void Function(double? progress)? onProgress,
  });

  Future<void> openExternal(String url);
}

class ReleaseServiceException implements Exception {
  ReleaseServiceException(this.message);

  final String message;

  @override
  String toString() => message;
}

/// 通过 GitHub REST API 获取版本、通过 HTTP 下载产物的实现。
class GitHubReleaseService implements ReleaseService {
  GitHubReleaseService({
    this.repository = 'YMRwithNoworry/wcmusic',
    HttpClient Function()? httpClientFactory,
    Future<Directory> Function()? directoryProvider,
    Future<void> Function(String url)? externalLauncher,
    Future<void> Function(String executable, List<String> arguments,
            String scriptPath)?
        updaterLauncher,
    this.currentVersion,
  }) : _httpClientFactory = httpClientFactory ?? HttpClient.new,
       _directoryProvider = directoryProvider,
       _externalLauncher = externalLauncher,
       _updaterLauncher = updaterLauncher;

  final String repository;
  final HttpClient Function() _httpClientFactory;
  final Future<Directory> Function()? _directoryProvider;
  final Future<void> Function(String url)? _externalLauncher;
  final Future<void> Function(
    String executable,
    List<String> arguments,
    String scriptPath,
  )?
  _updaterLauncher;

  /// 当前运行版本（例如 1.2.82）；为空时退回到 pubspec 版本，都没有就按平台查最新发布。
  final String? currentVersion;

  // 发布标签：合并发布后 Windows 与 Android 共用 vX.Y.Z；早期历史上还出现过
  // 独立的 android-vX.Y.Z 版本线，这里仍然识别，避免给老用户报错版本。
  static const windowsTagPrefix = 'v';
  static const androidTagPrefix = 'android-v';

  static final _versionPattern = RegExp(r'^(\d+)\.(\d+)\.(\d+)$');

  /// 构建元数据：CI 用 `--build-name` 打出包，本地可能只是 pubspec 版本。
  static const rebuildMetaName = 'rebuild_meta.json';

  /// 上次一键更新写入的版本记录，文件名固定在 exe 旁边。
  static const updateInfoSuffix = '.update-info.json';

  /// 当前平台能接受的 tag 前缀：合并发布的 vX.Y.Z，以及 Android 历史上单独的
  /// android-vX.Y.Z。Windows 不能把 android-v 的发布当成自己的版本。
  List<String> get _platformTagPrefixes => Platform.isAndroid
      ? const [androidTagPrefix, windowsTagPrefix]
      : const [windowsTagPrefix];

  String get _platformAssetSuffix => Platform.isAndroid ? '.apk' : '.zip';

  /// 当前版本的可信来源：构建元数据 → 上次更新记录 → 显式版本 / pubspec。
  ///
  /// 只信 `--app-version` 或 pubspec 会踩坑：本地源码直接跑是 pubspec 版本，而
  /// 官方包是用 `--build-name` 构建的，两者不一致时会一直提示“有新版本”。
  /// 构建元数据由 CI 写进包内，更新记录由一键更新的脚本写进安装目录，
  /// 它们才是这台机器上真正运行的版本。
  List<Future<String?>> _currentVersionCandidates() => [
    _versionFromRebuildMeta(),
    if (Platform.isWindows) _versionFromUpdateInfo(),
    if (currentVersion != null && currentVersion!.isNotEmpty)
      Future.value(currentVersion),
    _versionFromPubspec(),
  ];

  Future<String?> _versionFromPubspec() async {
    try {
      final file = File(
        '${Directory.current.path}${Platform.pathSeparator}pubspec.yaml',
      );
      if (!await file.exists()) return null;
      final text = await file.readAsString();
      final match = RegExp(
        r'^version:\s*([0-9]+\.[0-9]+\.[0-9]+)',
        multiLine: true,
      ).firstMatch(text);
      return match?.group(1);
    } on Object {
      return null;
    }
  }

  /// 包内 `data/rebuild_meta.json` 记录的 `--build-name`（版本）。
  Future<String?> _versionFromRebuildMeta() async {
    try {
      final exe = File(Platform.resolvedExecutable).parent;
      final file = File(
        '${exe.path}${Platform.pathSeparator}data'
        '${Platform.pathSeparator}$rebuildMetaName',
      );
      if (!await file.exists()) return null;
      final decoded = jsonDecode(await file.readAsString());
      final version = decoded is Map ? decoded['version'] as String? : null;
      return version == null ? null : _normalizeVersion(version);
    } on Object {
      return null;
    }
  }

  /// 上次一键更新写下的版本记录（`<exe>.update-info.json`）。
  ///
  /// 也顺便清理安装目录里残留的更新包。
  Future<String?> _versionFromUpdateInfo() async {
    try {
      final exe = File(Platform.resolvedExecutable);
      final info = File('${exe.path}$updateInfoSuffix');
      if (await info.exists()) {
        final decoded = jsonDecode(await info.readAsString());
        final version = decoded is Map ? decoded['version'] as String? : null;
        if (version != null) return _normalizeVersion(version);
      }
      await for (final entity in exe.parent.list()) {
        if (entity is File && entity.path.endsWith('.zip')) {
          // 更新完成后残留的安装包，删掉省空间（失败也无所谓）。
          await entity.delete().catchError((_) => entity);
        }
      }
    } on Object {
      // 读不到就用其它来源判断
    }
    return null;
  }

  /// 能解析成三段数字的版本号（GitHub 有的 tag 是 v1.2 这种）。
  static String? _normalizeVersion(String raw) {
    final match = _versionPattern.firstMatch(raw.trim());
    return match == null ? null : '${match[1]}.${match[2]}.${match[3]}';
  }

  @override
  Future<String?> currentVersionName() async {
    for (final candidate in _currentVersionCandidates()) {
      final version = _normalizeVersion(await candidate ?? '');
      if (version != null) return version;
    }
    return null;
  }

  @override
  Future<ReleaseInfo> fetchLatestRelease() async {
    final client = _httpClientFactory()
      ..connectionTimeout = const Duration(seconds: 15)
      ..userAgent = 'WCMusic';
    try {
      // 每页多取一些，跳过该平台的旧版本发布（早期 Release 两端共用 vX.Y.Z）
      final request = await client.getUrl(
        Uri.parse(
          'https://api.github.com/repos/$repository/releases?per_page=30',
        ),
      );
      request.headers.set(
        HttpHeaders.acceptHeader,
        'application/vnd.github+json',
      );
      final response = await request.close().timeout(
        const Duration(seconds: 20),
      );
      final body = await response.transform(utf8.decoder).join();
      if (response.statusCode != HttpStatus.ok) {
        throw ReleaseServiceException('获取版本信息失败（HTTP ${response.statusCode}）');
      }
      final decoded = jsonDecode(body);
      if (decoded is! List) {
        throw ReleaseServiceException('版本信息格式不正确');
      }
      final assetSuffix = _platformAssetSuffix;
      for (final item in decoded) {
        if (item is! Map<String, dynamic>) continue;
        if (item['draft'] == true || item['prerelease'] == true) continue;
        final tag = item['tag_name'] as String? ?? '';
        String? version;
        for (final prefix in _platformTagPrefixes) {
          if (!tag.startsWith(prefix)) continue;
          version = _normalizeVersion(tag.substring(prefix.length));
          if (version != null) break;
        }
        if (version == null) continue;
        final assets = item['assets'];
        if (assets is! List || assets.isEmpty) continue;
        final asset = assets.whereType<Map<String, dynamic>>().where((asset) {
          final name = asset['name'] as String? ?? '';
          return name.toLowerCase().endsWith(assetSuffix);
        }).firstOrNull;
        if (asset == null) continue;
        return ReleaseInfo(
          tag: tag,
          version: version,
          name: item['name'] as String? ?? tag,
          notes: item['body'] as String? ?? '',
          publishedAt: DateTime.tryParse(item['published_at'] as String? ?? ''),
          assetName: asset['name'] as String? ?? '',
          assetUrl: asset['browser_download_url'] as String? ?? '',
          assetSize: (asset['size'] as num?)?.toInt() ?? 0,
          pageUrl: item['html_url'] as String? ?? '',
        );
      }
      throw ReleaseServiceException('没有找到可用的发布版本');
    } on SocketException catch (error) {
      throw ReleaseServiceException('网络连接失败：${error.message}');
    } on HttpException catch (error) {
      throw ReleaseServiceException('网络请求失败：${error.message}');
    } finally {
      client.close(force: true);
    }
  }

  @override
  Future<void> download(
    String url,
    File target, {
    void Function(double? progress)? onProgress,
  }) async {
    final client = _httpClientFactory()
      ..connectionTimeout = const Duration(seconds: 20)
      ..userAgent = 'WCMusic';
    try {
      await target.parent.create(recursive: true);
      final request = await client.getUrl(Uri.parse(url));
      request.followRedirects = true;
      final response = await request.close().timeout(
        const Duration(seconds: 30),
      );
      if (response.statusCode != HttpStatus.ok) {
        throw ReleaseServiceException('下载失败（HTTP ${response.statusCode}）');
      }
      final total = response.contentLength > 0 ? response.contentLength : null;
      var received = 0;
      // 先写入临时文件，成功后再改名，避免留下半截的安装包
      final partial = File('${target.path}.part');
      final sink = partial.openWrite();
      try {
        await for (final chunk in response) {
          sink.add(chunk);
          received += chunk.length;
          onProgress?.call(total == null ? null : received / total);
        }
        await sink.flush();
      } finally {
        await sink.close();
      }
      if (await target.exists()) await target.delete();
      await partial.rename(target.path);
      onProgress?.call(1);
    } on SocketException catch (error) {
      throw ReleaseServiceException('下载中断：${error.message}');
    } finally {
      client.close(force: true);
    }
  }

  @override
  Future<void> openExternal(String url) async {
    final launcher = _externalLauncher;
    if (launcher != null) return launcher(url);
    await launchUrl(Uri.parse(url), mode: LaunchMode.externalApplication);
  }

  @override
  Future<UpdateInstallResult?> installUpdate(
    ReleaseInfo release, {
    void Function(double? progress)? onProgress,
  }) async {
    final directory = _directoryProvider != null
        ? await _directoryProvider()
        : Platform.isAndroid
        ? await _defaultDownloadDirectory()
        : await Directory.systemTemp.createTemp('wcmusic_update');
    final target = File(
      '${directory.path}${Platform.pathSeparator}${release.assetName}',
    );
    await download(release.assetUrl, target, onProgress: onProgress);
    if (Platform.isAndroid) {
      return UpdateInstallResult.apk(target.path);
    }
    if (Platform.isWindows) {
      // 正在运行的 exe 无法原地覆盖：启动脚本等待退出后解压并重启。
      // 脚本与安装包放在同一个临时目录里，脚本结束时整目录清理。
      final script = await _writeWindowsUpdater(target, release.version);
      final launcher = _updaterLauncher;
      if (launcher != null) {
        await launcher('powershell.exe', ['-File', script.path], script.path);
      } else {
        await Process.start('powershell.exe', [
          '-NoProfile',
          '-ExecutionPolicy',
          'Bypass',
          '-WindowStyle',
          'Hidden',
          '-File',
          script.path,
        ], mode: ProcessStartMode.detached);
      }
      return null;
    }
    throw ReleaseServiceException('当前平台不支持一键更新');
  }

  Future<Directory> _defaultDownloadDirectory() async {
    final base = await getTemporaryDirectory();
    return Directory('${base.path}${Platform.pathSeparator}wcmusic_update');
  }

  /// 生成 PowerShell 更新脚本：等待本进程退出 → 解压覆盖 → 启动新版本。
  ///
  /// 解压完成后在 exe 旁边写 `<exe>.update-info.json`，下次启动时据此认出
  /// 真正装上的版本，而不是只相信 `--app-version`。
  Future<File> _writeWindowsUpdater(File archive, String version) async {
    final script = File('${archive.path}.ps1');
    final exe = Platform.resolvedExecutable;
    final installDir = File(exe).parent.path;
    final logFile = '${archive.path}.log';
    final escaped = (String value) => value.replaceAll("'", "''");
    final content =
        '''
\$ErrorActionPreference = 'Stop'
\$pid_ = ${pid}
\$exe = '${escaped(exe)}'
\$dest = '${escaped(installDir)}'
\$archive = '${escaped(archive.path)}'
\$log = '${escaped(logFile)}'
function Write-Log(\$text) { Add-Content -Path \$log -Value ("{0} {1}" -f (Get-Date -Format 'HH:mm:ss'), \$text) }
try {
  Write-Log "等待 WCMusic 退出"
  for (\$i = 0; \$i -lt 120; \$i++) {
    if (-not (Get-Process -Id \$pid_ -ErrorAction SilentlyContinue)) { break }
    Start-Sleep -Milliseconds 500
  }
  Write-Log "解压更新包到 \$dest"
  Expand-Archive -Path \$archive -DestinationPath \$dest -Force
  Set-Content -LiteralPath (\$exe + '$updateInfoSuffix') -Value ('{"version":"${escaped(version)}"}') -Encoding utf8NoBOM
  Write-Log "启动新版本"
  Start-Process -FilePath \$exe -WorkingDirectory \$dest
} catch {
  Write-Log ("更新失败：{0}" -f \$_)
} finally {
  Remove-Item -LiteralPath \$PSScriptRoot -Recurse -Force -ErrorAction SilentlyContinue
}
''';
    await script.writeAsString(content, flush: true);
    return script;
  }
}
