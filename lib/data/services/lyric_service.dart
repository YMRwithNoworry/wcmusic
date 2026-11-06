import 'dart:convert';
import 'dart:io';

import '../../domain/models/lyric_line.dart';
import '../../domain/models/track.dart';

abstract interface class LyricService {
  Future<List<LyricLine>> loadLyrics(Track track);
}

abstract interface class LyricTranslator {
  Future<List<LyricLine>> translateMissing(List<LyricLine> lines);
}

class OnlineLyricService implements LyricService, LyricTranslator {
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
  List<String>? _bingCredentials;
  DateTime? _bingCredentialsAt;

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
          translation:
              _text(value['translateLyric']) ??
              _text(value['translatedLyric']),
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
    final translation = decoded is Map ? _text(decoded['trans']) : null;
    return lyric == null
        ? const []
        : parseLrcWithTranslation(lyric, translation ?? '');
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
    final tlyric = decoded is Map ? decoded['tlyric'] : null;
    final translation = tlyric is Map ? _text(tlyric['lyric']) : null;
    return lyric == null
        ? const []
        : parseLrcWithTranslation(lyric, translation ?? '');
  }

  @override
  Future<List<LyricLine>> translateMissing(List<LyricLine> lines) async {
    final targets = translationTargets(lines);
    if (targets.isEmpty) return lines;
    try {
      final translated = <String>[];
      for (var start = 0; start < targets.length; start += 40) {
        final batch = targets
            .skip(start)
            .take(40)
            .map((index) => lines[index].text.trim())
            .toList(growable: false);
        try {
          translated.addAll(await _translateBing(batch));
        } on Object {
          try {
            translated.addAll(await _translateGoogle(batch));
          } on Object {
            translated.addAll(await _translateMyMemory(batch));
          }
        }
      }
      return mergeLyricTranslations(lines, targets, translated);
    } on Object {
      return lines;
    }
  }

  Future<List<String>> _translateBing(List<String> texts) async {
    final credentials = await _loadBingCredentials();
    final joined = await _bingTranslate(credentials, texts.join('\n'));
    final lines = const LineSplitter()
        .convert(joined)
        .map((line) => line.trim())
        .toList(growable: false);
    if (lines.length == texts.length) return lines;
    return [
      for (final text in texts) (await _bingTranslate(credentials, text)).trim(),
    ];
  }

  Future<List<String>> _loadBingCredentials() async {
    final loadedAt = _bingCredentialsAt;
    final current = _bingCredentials;
    if (loadedAt != null &&
        current != null &&
        DateTime.now().difference(loadedAt) < const Duration(minutes: 30)) {
      return current;
    }
    final page = await _getText(Uri.https('cn.bing.com', '/translator'));
    final ig = RegExp(r'IG:"([0-9A-F]+)"').firstMatch(page)?.group(1);
    final key = RegExp(
      r'params_AbusePreventionHelper\s*=\s*\[(\d+),',
    ).firstMatch(page)?.group(1);
    final token = RegExp(
      r'params_AbusePreventionHelper\s*=\s*\[\d+,"([^"]+)"',
    ).firstMatch(page)?.group(1);
    if (ig == null || key == null || token == null) {
      throw const FormatException('微软翻译页面缺少必要参数');
    }
    final credentials = [ig, key, token];
    _bingCredentials = credentials;
    _bingCredentialsAt = DateTime.now();
    return credentials;
  }

  Future<String> _bingTranslate(List<String> credentials, String text) async {
    final uri = Uri.https('cn.bing.com', '/ttranslatev3', {
      'isVertical': '1',
      'IG': credentials[0],
      'IID': 'translator.5023.1',
    });
    final form = Uri(queryParameters: {
      'fromLang': 'auto-detect',
      'text': text,
      'to': 'zh-Hans',
      'token': credentials[2],
      'key': credentials[1],
    }).query;
    final body = await _getText(
      uri,
      method: 'POST',
      body: form,
      headers: const {
        HttpHeaders.contentTypeHeader: 'application/x-www-form-urlencoded',
        HttpHeaders.refererHeader: 'https://cn.bing.com/translator',
        HttpHeaders.userAgentHeader:
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36',
      },
    );
    final decoded = jsonDecode(body);
    if (decoded is! List || decoded.isEmpty || decoded.first is! Map) {
      throw const FormatException('微软翻译响应格式无效');
    }
    final translations = (decoded.first as Map)['translations'];
    if (translations is! List || translations.isEmpty || translations.first is! Map) {
      throw const FormatException('微软翻译结果为空');
    }
    final translated = (translations.first as Map)['text'];
    if (translated is! String || translated.isEmpty) {
      throw const FormatException('微软翻译结果为空');
    }
    return translated;
  }

  Future<List<String>> _translateGoogle(List<String> texts) async {
    final uri = Uri.https('translate.googleapis.com', '/translate_a/single', {
      'client': 'gtx',
      'sl': 'auto',
      'tl': 'zh-CN',
      'dt': 't',
      'q': texts.join('\n'),
    });
    final decoded = await _getJson(uri);
    if (decoded is! List || decoded.isEmpty || decoded.first is! List) {
      throw const FormatException('翻译响应格式无效');
    }
    final segments = (decoded.first as List)
        .whereType<List>()
        .where((segment) => segment.isNotEmpty && segment.first is String)
        .map((segment) => segment.first as String)
        .join();
    final lines = const LineSplitter()
        .convert(segments)
        .map((line) => line.trim())
        .toList(growable: false);
    if (lines.length != texts.length) {
      throw const FormatException('翻译行数与原文不匹配');
    }
    return lines;
  }

  Future<List<String>> _translateMyMemory(List<String> texts) async {
    final translated = <String>[];
    for (final text in texts) {
      final source = _guessSourceLanguage(text);
      final decoded = await _getJson(
        Uri.https('api.mymemory.translated.net', '/get', {
          'q': text,
          'langpair': '$source|zh-CN',
        }),
      );
      final data = decoded is Map ? decoded['responseData'] : null;
      translated.add(
        data is Map ? _text(data['translatedText']) ?? '' : '',
      );
    }
    return translated;
  }

  String _guessSourceLanguage(String text) {
    if (text.runes.any(_isKana)) return 'ja';
    if (text.runes.any(_isHangul)) return 'ko';
    return 'en';
  }

  Future<dynamic> _getJson(Uri uri) async => jsonDecode(await _getText(uri));

  Future<String> _getText(
    Uri uri, {
    String method = 'GET',
    String? body,
    Map<String, String> headers = const {},
  }) async {
    final proxy = _proxyResolver(uri);
    try {
      return await _getTextOnce(
        uri,
        proxy,
        method: method,
        body: body,
        headers: headers,
      );
    } on Object {
      if (proxy == 'DIRECT') rethrow;
      return _getTextOnce(
        uri,
        'DIRECT',
        method: method,
        body: body,
        headers: headers,
      );
    }
  }

  Future<String> _getTextOnce(
    Uri uri,
    String proxy, {
    String method = 'GET',
    String? body,
    Map<String, String> headers = const {},
  }) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 10)
      ..findProxy = (_) => proxy;
    try {
      final request = method == 'POST'
          ? await client.postUrl(uri)
          : await client.getUrl(uri);
      request.headers.set(HttpHeaders.acceptHeader, 'application/json');
      request.headers.set(HttpHeaders.userAgentHeader, 'WCMusic/1.0');
      request.headers.set(HttpHeaders.refererHeader, _refererFor(uri));
      for (final entry in headers.entries) {
        request.headers.set(entry.key, entry.value);
      }
      if (body != null) request.write(body);
      final response = await request.close().timeout(
        const Duration(seconds: 15),
      );
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('歌词服务返回 ${response.statusCode}', uri: uri);
      }
      return await utf8.decoder.bind(response).join().timeout(
        const Duration(seconds: 15),
      );
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

List<LyricLine> parseLrcWithTranslation(
  String content,
  String translationContent,
) {
  final lines = parseLrc(content);
  final translations = parseLrc(translationContent);
  if (lines.isEmpty || translations.isEmpty) return lines;
  const toleranceMs = 500;
  final hasTimestampMatches = lines.any(
    (line) => translations.any(
      (translation) =>
          (translation.time - line.time).inMilliseconds.abs() <= toleranceMs,
    ),
  );
  final pairByOrder = !hasTimestampMatches && lines.length == translations.length;
  final used = <int>{};
  return [
    for (var index = 0; index < lines.length; index++)
      _lineWithTranslation(
        lines[index],
        pairByOrder
            ? translations[index]
            : _nearestTranslation(lines[index], translations, used, toleranceMs),
      ),
  ];
}

LyricLine _lineWithTranslation(LyricLine line, LyricLine? translation) {
  final text = translation?.text.trim();
  if (text == null || text.isEmpty || text == '//' || text == line.text.trim()) {
    return line;
  }
  return LyricLine(time: line.time, text: line.text, translation: text);
}

LyricLine? _nearestTranslation(
  LyricLine line,
  List<LyricLine> translations,
  Set<int> used,
  int toleranceMs,
) {
  var candidateIndex = -1;
  var nearestDistance = toleranceMs + 1;
  for (var index = 0; index < translations.length; index++) {
    if (used.contains(index)) continue;
    final distance = (translations[index].time - line.time).inMilliseconds.abs();
    if (distance < nearestDistance) {
      nearestDistance = distance;
      candidateIndex = index;
    }
  }
  if (candidateIndex < 0 || nearestDistance > toleranceMs) return null;
  used.add(candidateIndex);
  return translations[candidateIndex];
}

List<int> translationTargets(List<LyricLine> lines) => [
  for (var index = 0; index < lines.length; index++)
    if (_needsTranslation(lines[index])) index,
];

bool _needsTranslation(LyricLine line) {
  final text = line.text.trim();
  if (text.isEmpty || (line.translation?.trim().isNotEmpty ?? false)) {
    return false;
  }
  if (_isChineseLine(text)) return false;
  return text.runes.any(_isWordRune);
}

bool _isWordRune(int value) =>
    value >= 0x30 &&
    value <= 0x39 ||
    value >= 0x41 &&
    value <= 0x5A ||
    value >= 0x61 &&
    value <= 0x7A ||
    value >= 0x00C0 &&
    value <= 0x02AF ||
    value >= 0x0370 &&
    value <= 0x08FF ||
    value >= 0x0900 &&
    value <= 0x0DFF ||
    value >= 0x0E00 &&
    value <= 0x0EFF ||
    value >= 0x10A0 &&
    value <= 0x10FF ||
    value >= 0x1100 &&
    value <= 0x11FF ||
    value >= 0x3040 &&
    value <= 0x30FF ||
    value >= 0x3130 &&
    value <= 0x318F ||
    value >= 0x3400 &&
    value <= 0x9FFF ||
    value >= 0xAC00 &&
    value <= 0xD7AF;
}

bool _isChineseLine(String text) {
  final runes = text.runes;
  return runes.any(_isHan) && !runes.any(_isKana);
}

bool _isHan(int value) =>
    value >= 0x3400 &&
    value <= 0x4DBF ||
    value >= 0x4E00 &&
    value <= 0x9FFF ||
    value >= 0xF900 &&
    value <= 0xFAFF;

bool _isKana(int value) => value >= 0x3040 && value <= 0x30FF;

bool _isHangul(int value) =>
    value >= 0x1100 &&
    value <= 0x11FF ||
    value >= 0x3130 &&
    value <= 0x318F ||
    value >= 0xAC00 &&
    value <= 0xD7AF;

List<LyricLine> mergeLyricTranslations(
  List<LyricLine> lines,
  List<int> targets,
  List<String> translations,
) {
  final merged = [...lines];
  for (var index = 0; index < targets.length && index < translations.length; index++) {
    final target = targets[index];
    if (target < 0 || target >= merged.length) continue;
    merged[target] = _lineWithTranslation(
      merged[target],
      LyricLine(time: merged[target].time, text: translations[index]),
    );
  }
  return merged;
}
