import 'dart:convert';
import 'dart:io';

import '../../domain/models/lyric_line.dart';
import '../../domain/models/track.dart';

abstract interface class LyricService {
  Future<List<LyricLine>> loadLyrics(Track track);
}

class OnlineLyricService implements LyricService {
  OnlineLyricService({
    HttpClient Function()? clientFactory,
    String Function(Uri)? proxyResolver,
    Uri? kuwoEndpoint,
    Uri? kugouSearchEndpoint,
    Uri? kugouDownloadEndpoint,
    Uri? qqEndpoint,
    Uri? neteaseEndpoint,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _proxyResolver =
           proxyResolver ??
           ((uri) => HttpClient.findProxyFromEnvironment(
             uri,
             environment: Platform.environment,
           )),
       _kuwoEndpoint =
           kuwoEndpoint ??
           Uri.https('www.kuwo.cn', '/openapi/v1/www/lyric/getlyric'),
       _kugouSearchEndpoint =
           kugouSearchEndpoint ?? Uri.https('lyrics.kugou.com', '/search'),
       _kugouDownloadEndpoint =
           kugouDownloadEndpoint ?? Uri.https('lyrics.kugou.com', '/download'),
       _qqEndpoint =
           qqEndpoint ??
           Uri.https('c.y.qq.com', '/lyric/fcgi-bin/fcg_query_lyric_new.fcg'),
       _neteaseEndpoint =
           neteaseEndpoint ?? Uri.https('music.163.com', '/api/song/lyric');

  final HttpClient Function() _clientFactory;
  final String Function(Uri) _proxyResolver;
  final Uri _kuwoEndpoint;
  final Uri _kugouSearchEndpoint;
  final Uri _kugouDownloadEndpoint;
  final Uri _qqEndpoint;
  final Uri _neteaseEndpoint;

  @override
  Future<List<LyricLine>> loadLyrics(Track track) async {
    final sourceId = track.sourceId;
    if (sourceId == null || sourceId.isEmpty) return const [];
    final lines = switch (track.source) {
      TrackSource.kw => await _loadKuwo(sourceId),
      TrackSource.kg => await _loadKugou(sourceId),
      TrackSource.tx => await _loadQq(sourceId),
      TrackSource.wy => await _loadNetease(sourceId),
      TrackSource.local ||
      TrackSource.mg ||
      TrackSource.custom => const <LyricLine>[],
    };
    lines.sort((left, right) => left.time.compareTo(right.time));
    return lines;
  }

  Future<List<LyricLine>> _loadKuwo(String sourceId) async {
    final decoded = await _getJson(
      _kuwoEndpoint.replace(
        queryParameters: {
          ..._kuwoEndpoint.queryParameters,
          'musicId': sourceId,
        },
      ),
    );
    final data = decoded is Map ? decoded['data'] : null;
    final values = data is Map ? data['lrclist'] : null;
    if (values is! List) return const [];
    final lines = <LyricLine>[];
    for (final value in values) {
      if (value is! Map) continue;
      final time = double.tryParse(value['time']?.toString() ?? '');
      final lyric = _text(value['lineLyric']);
      if (time == null || lyric == null) continue;
      lines.add(
        LyricLine(
          time: Duration(milliseconds: (time * 1000).round()),
          text: lyric,
        ),
      );
    }
    return lines;
  }

  Future<List<LyricLine>> _loadKugou(String sourceId) async {
    final search = await _getJson(
      _kugouSearchEndpoint.replace(
        queryParameters: {
          ..._kugouSearchEndpoint.queryParameters,
          'ver': '1',
          'man': 'yes',
          'client': 'pc',
          'hash': sourceId,
        },
      ),
    );
    final candidates = search is Map ? search['candidates'] : null;
    if (candidates is! List || candidates.isEmpty || candidates.first is! Map) {
      return const [];
    }
    final candidate = candidates.first as Map;
    final id = candidate['id']?.toString();
    final accessKey = _text(candidate['accesskey']);
    if (id == null || accessKey == null) return const [];
    final download = await _getJson(
      _kugouDownloadEndpoint.replace(
        queryParameters: {
          ..._kugouDownloadEndpoint.queryParameters,
          'ver': '1',
          'client': 'pc',
          'id': id,
          'accesskey': accessKey,
          'fmt': 'lrc',
          'charset': 'utf8',
        },
      ),
    );
    final content = download is Map ? _text(download['content']) : null;
    if (content == null) return const [];
    return parseLrc(utf8.decode(base64Decode(content)));
  }

  Future<List<LyricLine>> _loadQq(String sourceId) async {
    final decoded = await _getJson(
      _qqEndpoint.replace(
        queryParameters: {
          ..._qqEndpoint.queryParameters,
          'songmid': sourceId,
          'format': 'json',
          'nobase64': '1',
        },
      ),
    );
    final lyric = decoded is Map ? _text(decoded['lyric']) : null;
    return lyric == null ? const [] : parseLrc(lyric);
  }

  Future<List<LyricLine>> _loadNetease(String sourceId) async {
    final decoded = await _getJson(
      _neteaseEndpoint.replace(
        queryParameters: {
          ..._neteaseEndpoint.queryParameters,
          'id': sourceId,
          'lv': '-1',
          'kv': '-1',
          'tv': '-1',
        },
      ),
    );
    final lrc = decoded is Map ? decoded['lrc'] : null;
    final lyric = lrc is Map ? _text(lrc['lyric']) : null;
    return lyric == null ? const [] : parseLrc(lyric);
  }

  Future<dynamic> _getJson(Uri uri) async {
    final proxy = _proxyResolver(uri);
    try {
      return await _getJsonOnce(uri, proxy);
    } on Object {
      if (proxy == 'DIRECT') rethrow;
      return _getJsonOnce(uri, 'DIRECT');
    }
  }

  Future<dynamic> _getJsonOnce(Uri uri, String proxy) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 10)
      ..findProxy = (_) => proxy;
    try {
      final request = await client.getUrl(uri);
      request.headers.set(HttpHeaders.acceptHeader, 'application/json');
      request.headers.set(HttpHeaders.userAgentHeader, 'WCMusic/1.0');
      request.headers.set(HttpHeaders.refererHeader, _refererFor(uri));
      final response = await request.close().timeout(
        const Duration(seconds: 15),
      );
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('歌词服务返回 ${response.statusCode}', uri: uri);
      }
      return jsonDecode(await utf8.decoder.bind(response).join());
    } finally {
      client.close(force: true);
    }
  }

  String _refererFor(Uri uri) {
    if (uri.host.contains('qq.com')) return 'https://y.qq.com/';
    if (uri.host.contains('kuwo')) return 'https://www.kuwo.cn/';
    if (uri.host.contains('kugou')) return 'https://www.kugou.com/';
    return 'https://music.163.com/';
  }

  String? _text(Object? value) {
    if (value is! String) return null;
    final text = value.trim();
    return text.isEmpty ? null : text;
  }
}

List<LyricLine> parseLrc(String content) {
  final timestamp = RegExp(r'\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]');
  final lines = <LyricLine>[];
  for (final rawLine in const LineSplitter().convert(content)) {
    final matches = timestamp.allMatches(rawLine).toList(growable: false);
    if (matches.isEmpty) continue;
    final text = rawLine.replaceAll(timestamp, '').trim();
    if (text.isEmpty) continue;
    for (final match in matches) {
      final minutes = int.parse(match.group(1)!);
      final seconds = int.parse(match.group(2)!);
      final fraction = match.group(3) ?? '';
      final milliseconds = switch (fraction.length) {
        0 => 0,
        1 => int.parse(fraction) * 100,
        2 => int.parse(fraction) * 10,
        _ => int.parse(fraction.substring(0, 3)),
      };
      lines.add(
        LyricLine(
          time: Duration(
            minutes: minutes,
            seconds: seconds,
            milliseconds: milliseconds,
          ),
          text: text,
        ),
      );
    }
  }
  lines.sort((left, right) => left.time.compareTo(right.time));
  return lines;
}
