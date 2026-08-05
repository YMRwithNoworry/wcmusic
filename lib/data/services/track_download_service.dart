import 'dart:io';

import 'package:file_selector/file_selector.dart';
import 'package:path_provider/path_provider.dart';

import '../../domain/models/track.dart';

typedef DownloadPathResolver =
    Future<String?> Function(Track track, String extension);
typedef DownloadProxyResolver = String Function(Uri uri);

const _audioExtensions = {
  '.mp3',
  '.flac',
  '.m4a',
  '.wav',
  '.ogg',
  '.aac',
  '.opus',
  '.ape',
  '.wma',
};

class TrackDownloadService {
  TrackDownloadService({
    HttpClient Function()? clientFactory,
    this._pathResolver,
    Future<Directory> Function()? cacheDirectoryProvider,
    DownloadProxyResolver? proxyResolver,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _cacheDirectoryProvider =
           cacheDirectoryProvider ?? getTemporaryDirectory,
       _proxyResolver =
           proxyResolver ??
           ((uri) => HttpClient.findProxyFromEnvironment(
             uri,
             environment: Platform.environment,
           ));

  final HttpClient Function() _clientFactory;
  final DownloadPathResolver? _pathResolver;
  final Future<Directory> Function() _cacheDirectoryProvider;
  final DownloadProxyResolver _proxyResolver;

  Future<String> download(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
  }) async {
    if (track.uri.isEmpty) throw StateError('当前歌曲没有可下载的音频地址');
    final uri = Uri.parse(track.uri);
    final extension = _extensionFor(uri, fallbackExtension);
    final path = _pathResolver != null
        ? await _pathResolver(track, extension)
        : await _defaultPath(track, extension);
    if (path == null) throw StateError('已取消下载');
    if (_isNetworkUri(uri)) {
      return _downloadTo(track, uri, path, onProgress, null);
    }
    return _copyLocalTo(track.uri, uri, path, onProgress);
  }

  Future<String> downloadToCache(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
    bool Function()? shouldCancel,
  }) async {
    if (track.uri.isEmpty) throw StateError('当前歌曲没有可下载的音频地址');
    final uri = Uri.parse(track.uri);
    final extension = _extensionFor(uri, fallbackExtension);
    final directory = await _cacheDirectoryProvider();
    final path =
        '${directory.path}${Platform.pathSeparator}${_safeName('wcmusic-${track.id}$extension')}';
    final file = File(path);
    if (await file.exists() && await file.length() > 1024) return path;
    return _downloadTo(track, uri, path, onProgress, shouldCancel);
  }

  Future<String> _downloadTo(
    Track track,
    Uri uri,
    String path,
    void Function(double progress) onProgress,
    bool Function()? shouldCancel,
  ) async {
    final proxy = _proxyResolver(uri);
    try {
      return await _downloadToOnce(
        track,
        uri,
        path,
        onProgress,
        shouldCancel,
        proxy,
      );
    } on Object {
      if (proxy == 'DIRECT') rethrow;
      return _downloadToOnce(
        track,
        uri,
        path,
        onProgress,
        shouldCancel,
        'DIRECT',
      );
    }
  }

  Future<String> _downloadToOnce(
    Track track,
    Uri uri,
    String path,
    void Function(double progress) onProgress,
    bool Function()? shouldCancel,
    String proxy,
  ) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 20)
      ..findProxy = (_) => proxy;
    final target = File(path);
    final partial = File('$path.part');
    try {
      final request = await client.getUrl(uri);
      request.headers.set(
        HttpHeaders.userAgentHeader,
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
      );
      request.headers.set(HttpHeaders.acceptHeader, '*/*');
      request.headers.set(HttpHeaders.refererHeader, _refererFor(track.source));
      final response = await request.close();
      if (response.statusCode < HttpStatus.ok ||
          response.statusCode >= HttpStatus.multipleChoices) {
        await response.drain<void>();
        throw HttpException('下载服务返回 ${response.statusCode}', uri: uri);
      }
      final total = response.contentLength;
      var received = 0;
      await target.parent.create(recursive: true);
      if (await partial.exists()) await partial.delete();
      final sink = partial.openWrite();
      try {
        await for (final chunk in response) {
          if (shouldCancel?.call() ?? false) {
            throw StateError('播放已切换，下载已取消');
          }
          sink.add(chunk);
          received += chunk.length;
          if (total > 0) {
            onProgress((received / total).clamp(0.0, 1.0).toDouble());
          }
        }
        await sink.flush();
      } finally {
        await sink.close();
      }
      if (received == 0) throw StateError('下载服务返回了空文件');
      if (await target.exists()) await target.delete();
      await partial.rename(target.path);
      return path;
    } on Object {
      if (await partial.exists()) await partial.delete();
      rethrow;
    } finally {
      client.close(force: true);
    }
  }

  Future<String> _copyLocalTo(
    String sourcePath,
    Uri uri,
    String targetPath,
    void Function(double progress) onProgress,
  ) async {
    final source = uri.scheme == 'file' ? File.fromUri(uri) : File(sourcePath);
    if (!await source.exists()) {
      throw StateError('本地音频文件不存在');
    }
    final target = File(targetPath);
    if (source.absolute.path != target.absolute.path) {
      await target.parent.create(recursive: true);
      await source.copy(target.path);
    }
    onProgress(1);
    return target.path;
  }

  bool _isNetworkUri(Uri uri) {
    final scheme = uri.scheme.toLowerCase();
    return scheme == 'http' || scheme == 'https';
  }

  String _refererFor(TrackSource source) => switch (source) {
    TrackSource.kw => 'https://www.kuwo.cn/',
    TrackSource.kg => 'https://www.kugou.com/',
    TrackSource.tx => 'https://y.qq.com/',
    TrackSource.wy => 'https://music.163.com/',
    TrackSource.mg => 'https://music.migu.cn/',
    _ => 'https://music.163.com/',
  };

  String _extensionFor(Uri uri, String fallback) {
    final segments = uri.pathSegments;
    if (segments.isNotEmpty) {
      final last = segments.last;
      final dot = last.lastIndexOf('.');
      if (dot > 0 && last.length - dot <= 5) {
        final extension = last.substring(dot).toLowerCase();
        if (_audioExtensions.contains(extension)) return extension;
      }
    }
    return fallback;
  }

  Future<String?> _defaultPath(Track track, String extension) async {
    final name = _safeName('${track.title} - ${track.artist}$extension');
    if (Platform.isAndroid) {
      final downloads = await getDownloadsDirectory();
      final base = downloads ?? await getApplicationDocumentsDirectory();
      return '${base.path}${Platform.pathSeparator}$name';
    }
    final location = await getSaveLocation(
      suggestedName: name,
      acceptedTypeGroups: const [
        XTypeGroup(
          label: '音频',
          extensions: ['mp3', 'flac', 'm4a', 'wav', 'ogg'],
        ),
      ],
    );
    return location?.path;
  }

  String _safeName(String value) => value
      .replaceAll(RegExp(r'[<>:"/\\|?*\x00-\x1f]'), '_')
      .trim()
      .replaceAll(RegExp(r'\s+'), ' ');
}
