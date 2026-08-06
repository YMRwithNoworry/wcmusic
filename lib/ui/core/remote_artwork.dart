import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import 'organic_artwork.dart';

typedef ArtworkHttpClientFactory = HttpClient Function();
typedef ArtworkProxyResolver = String Function(Uri uri);

class ArtworkLoader {
  ArtworkLoader({
    ArtworkHttpClientFactory? clientFactory,
    ArtworkProxyResolver? proxyResolver,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _proxyResolver =
           proxyResolver ??
           ((uri) {
             // 国内音乐服务的 CDN 直连，避免代理问题
             if (_shouldBypassProxy(uri)) {
               return 'DIRECT';
             }
             return HttpClient.findProxyFromEnvironment(
               uri,
               environment: Platform.environment,
             );
           });

  final ArtworkHttpClientFactory _clientFactory;
  final ArtworkProxyResolver _proxyResolver;

  // 判断是否应该绕过代理
  static bool _shouldBypassProxy(Uri uri) {
    final host = uri.host.toLowerCase();
    // 网易云音乐 CDN
    if (host.endsWith('.music.126.net')) return true;
    if (host.endsWith('.music.163.com')) return true;
    // QQ音乐 CDN
    if (host.endsWith('.gtimg.cn')) return true;
    if (host.endsWith('.qq.com')) return true;
    // 酷狗音乐 CDN
    if (host.contains('kugou')) return true;
    // 酷我音乐 CDN
    if (host.contains('kuwo')) return true;
    return false;
  }

  Future<Uint8List> load(String rawUrl) async {
    Object? lastError;
    StackTrace? lastStackTrace;
    for (final uri in _candidates(rawUrl)) {
      final proxy = _proxyResolver(uri);
      try {
        return await _loadOnce(uri, proxy);
      } on Object catch (error, stackTrace) {
        lastError = error;
        lastStackTrace = stackTrace;
        // 如果使用了代理失败，尝试直连
        if (proxy != 'DIRECT' && !_shouldBypassProxy(uri)) {
          try {
            return await _loadOnce(uri, 'DIRECT');
          } on Object catch (directError, directStackTrace) {
            lastError = directError;
            lastStackTrace = directStackTrace;
          }
        }
      }
    }
    Error.throwWithStackTrace(
      lastError ?? StateError('没有可用的封面地址'),
      lastStackTrace ?? StackTrace.current,
    );
  }

  Future<Uint8List> _loadOnce(Uri uri, String proxy) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 8)
      ..findProxy = (_) => proxy;
    try {
      final request = await client.getUrl(uri);
      request.headers.set(
        HttpHeaders.userAgentHeader,
        'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
      );
      request.headers.set(HttpHeaders.refererHeader, _refererFor(uri));
      request.headers.set(
        HttpHeaders.acceptHeader,
        'image/avif,image/webp,image/*,*/*;q=0.8',
      );
      final response = await request.close().timeout(
        const Duration(seconds: 12),
      );
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('封面服务返回 ${response.statusCode}', uri: uri);
      }
      return await consolidateHttpClientResponseBytes(response);
    } finally {
      client.close(force: true);
    }
  }

  String _refererFor(Uri uri) {
    if (uri.host.contains('qq.com') || uri.host.contains('gtimg.cn')) {
      return 'https://y.qq.com/';
    }
    if (uri.host.contains('kugou')) return 'https://www.kugou.com/';
    if (uri.host.contains('kuwo')) return 'https://www.kuwo.cn/';
    return 'https://music.163.com/';
  }

  Iterable<Uri> _candidates(String rawUrl) sync* {
    var original = Uri.parse(rawUrl);
    if (original.host.endsWith('.music.126.net') && original.scheme == 'http') {
      original = original.replace(scheme: 'https');
    }
    final hosts = <String>[original.host];
    if (RegExp(r'^p\d+\.music\.126\.net$').hasMatch(original.host)) {
      hosts.addAll(const [
        'p1.music.126.net',
        'p2.music.126.net',
        'p3.music.126.net',
      ]);
    }
    final emitted = <String>{};
    for (final host in hosts) {
      final queryParameters = {
        ...original.queryParameters,
        if (host.endsWith('.music.126.net')) 'param': '600y600',
      };
      final uri = original.replace(
        host: host,
        queryParameters: queryParameters.isEmpty ? null : queryParameters,
      );
      if (emitted.add(uri.toString())) yield uri;
    }
  }
}

class RemoteArtwork extends StatefulWidget {
  const RemoteArtwork({
    super.key,
    required this.url,
    required this.seed,
    this.fit = BoxFit.cover,
  });

  final String url;
  final String seed;
  final BoxFit fit;

  @override
  State<RemoteArtwork> createState() => _RemoteArtworkState();
}

class _RemoteArtworkState extends State<RemoteArtwork> {
  static final _loader = ArtworkLoader();
  static final Map<String, Future<Uint8List>> _cache = {};
  late Future<Uint8List> _bytes;

  @override
  void initState() {
    super.initState();
    _bytes = _load(widget.url);
  }

  @override
  void didUpdateWidget(RemoteArtwork oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.url != oldWidget.url) _bytes = _load(widget.url);
  }

  Future<Uint8List> _load(String url) {
    final cached = _cache[url];
    if (cached != null) return cached;
    if (_cache.length >= 64) _cache.remove(_cache.keys.first);
    late final Future<Uint8List> tracked;
    tracked = _loader
        .load(url)
        .then(
          (bytes) => bytes,
          onError: (Object error, StackTrace stackTrace) {
            if (identical(_cache[url], tracked)) _cache.remove(url);
            Error.throwWithStackTrace(error, stackTrace);
          },
        );
    _cache[url] = tracked;
    return tracked;
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<Uint8List>(
      future: _bytes,
      builder: (context, snapshot) {
        final bytes = snapshot.data;
        if (bytes == null) {
          if (snapshot.hasError) {
            return OrganicArtwork(seed: widget.seed, size: double.infinity);
          }
          return ColoredBox(
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            child: const Center(
              child: SizedBox.square(
                dimension: 22,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
            ),
          );
        }
        return Image.memory(
          bytes,
          width: double.infinity,
          height: double.infinity,
          fit: widget.fit,
          gaplessPlayback: true,
          errorBuilder: (_, _, _) =>
              OrganicArtwork(seed: widget.seed, size: double.infinity),
        );
      },
    );
  }
}
