import 'dart:convert';
import 'dart:io';

import 'package:html/parser.dart' as html_parser;

import '../../domain/models/track.dart';

enum OnlineSearchChannel { appleMusic, deezer, aggregate }

extension OnlineSearchChannelLabel on OnlineSearchChannel {
  String get label => switch (this) {
    OnlineSearchChannel.appleMusic => 'Apple Music',
    OnlineSearchChannel.deezer => 'Deezer',
    OnlineSearchChannel.aggregate => '聚合搜索',
  };
}

abstract interface class OnlineSearchService {
  Future<List<Track>> search(
    String query, {
    int limit = 30,
    OnlineSearchChannel channel = OnlineSearchChannel.appleMusic,
  });
  Future<List<PlatformPlaylist>> discoverPlaylists();
  Future<List<Track>> discoverNewTracks();
  Future<List<Track>> discoverPlaylistTracks(PlatformPlaylist playlist);
}

class AppleOnlineSearchService implements OnlineSearchService {
  AppleOnlineSearchService({
    HttpClient Function()? clientFactory,
    String Function(Uri)? proxyResolver,
    Uri? endpoint,
    Uri? deezerEndpoint,
    Uri? playlistEndpoint,
    Uri? newTracksEndpoint,
    Uri? lookupEndpoint,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _proxyResolver =
           proxyResolver ??
           ((uri) => HttpClient.findProxyFromEnvironment(
             uri,
             environment: Platform.environment,
           )),
       _endpoint = endpoint ?? Uri.https('itunes.apple.com', '/search'),
       _deezerEndpoint =
           deezerEndpoint ?? Uri.https('api.deezer.com', '/search'),
       _playlistEndpoint =
           playlistEndpoint ??
           Uri.https(
             'rss.marketingtools.apple.com',
             '/api/v2/cn/music/most-played/20/playlists.json',
           ),
       _newTracksEndpoint =
           newTracksEndpoint ??
           Uri.https(
             'rss.marketingtools.apple.com',
             '/api/v2/cn/music/most-played/100/songs.json',
           ),
       _lookupEndpoint =
           lookupEndpoint ?? Uri.https('itunes.apple.com', '/lookup');

  final HttpClient Function() _clientFactory;
  final String Function(Uri) _proxyResolver;
  final Uri _endpoint;
  final Uri _deezerEndpoint;
  final Uri _playlistEndpoint;
  final Uri _newTracksEndpoint;
  final Uri _lookupEndpoint;

  @override
  Future<List<Track>> search(
    String query, {
    int limit = 30,
    OnlineSearchChannel channel = OnlineSearchChannel.appleMusic,
  }) async {
    final keyword = query.trim();
    if (keyword.isEmpty) return const [];

    return switch (channel) {
      OnlineSearchChannel.appleMusic => _searchApple(keyword, limit),
      OnlineSearchChannel.deezer => _searchDeezer(keyword, limit),
      OnlineSearchChannel.aggregate => _searchAggregate(keyword, limit),
    };
  }

  Future<List<Track>> _searchApple(String keyword, int limit) async {
    final uri = _endpoint.replace(
      queryParameters: {
        ..._endpoint.queryParameters,
        'term': keyword,
        'media': 'music',
        'entity': 'song',
        'country': 'CN',
        'limit': limit.clamp(1, 50).toString(),
      },
    );
    final decoded = await _getJson(uri);
    if (decoded is! Map<String, dynamic> || decoded['results'] is! List) {
      throw const FormatException('在线搜索返回了无法识别的数据');
    }
    return _parseAppleResults(decoded['results'] as List);
  }

  Future<List<Track>> _searchDeezer(String keyword, int limit) async {
    final uri = _deezerEndpoint.replace(
      queryParameters: {
        ..._deezerEndpoint.queryParameters,
        'q': keyword,
        'limit': limit.clamp(1, 50).toString(),
      },
    );
    final decoded = await _getJson(uri);
    if (decoded is! Map<String, dynamic> || decoded['data'] is! List) {
      throw const FormatException('Deezer 返回了无法识别的数据');
    }
    return _parseDeezerResults(decoded['data'] as List);
  }

  Future<List<Track>> _searchAggregate(String keyword, int limit) async {
    Object? appleError;
    Object? deezerError;
    final results = await Future.wait([
      _searchApple(keyword, limit).onError((error, _) {
        appleError = error;
        return const [];
      }),
      _searchDeezer(keyword, limit).onError((error, _) {
        deezerError = error;
        return const [];
      }),
    ]);
    if (appleError != null && deezerError != null) {
      throw StateError('所有搜索渠道均不可用');
    }
    final unique = <String, Track>{};
    for (final track in results.expand((items) => items)) {
      final key = '${track.title}\u0000${track.artist}'.toLowerCase();
      unique.putIfAbsent(key, () => track);
    }
    return unique.values.take(limit).toList(growable: false);
  }

  @override
  Future<List<PlatformPlaylist>> discoverPlaylists() async {
    final decoded = await _getJson(_playlistEndpoint);
    if (decoded is! Map<String, dynamic> || decoded['feed'] is! Map) {
      throw const FormatException('平台歌单返回了无法识别的数据');
    }
    final feed = Map<String, dynamic>.from(decoded['feed'] as Map);
    final results = feed['results'];
    if (results is! List) {
      throw const FormatException('平台歌单缺少结果列表');
    }
    final playlists = <PlatformPlaylist>[];
    for (final value in results) {
      if (value is! Map) continue;
      final item = Map<String, dynamic>.from(value);
      final id = _text(item['id']);
      final name = _text(item['name']);
      final artwork = _text(item['artworkUrl100']);
      final url = _text(item['url']);
      if (id == null || name == null || artwork == null || url == null) {
        continue;
      }
      playlists.add(
        PlatformPlaylist(
          id: id,
          name: name,
          artworkUri: artwork.replaceFirst('100x100', '600x600'),
          url: url,
          platform: 'Apple Music',
        ),
      );
    }
    return playlists;
  }

  @override
  Future<List<Track>> discoverNewTracks() async {
    final decoded = await _getJson(_newTracksEndpoint);
    if (decoded is! Map<String, dynamic> || decoded['feed'] is! Map) {
      throw const FormatException('新曲推荐返回了无法识别的数据');
    }
    final feed = Map<String, dynamic>.from(decoded['feed'] as Map);
    final results = feed['results'];
    if (results is! List) {
      throw const FormatException('新曲推荐缺少结果列表');
    }

    final cutoff = DateTime.now().toUtc().subtract(const Duration(days: 365));
    final ids = <String>[];
    for (final value in results) {
      if (value is! Map) continue;
      final item = Map<String, dynamic>.from(value);
      final id = _text(item['id']);
      final releaseDate = DateTime.tryParse(_text(item['releaseDate']) ?? '');
      if (id != null &&
          releaseDate != null &&
          !releaseDate.toUtc().isBefore(cutoff)) {
        ids.add(id);
      }
      if (ids.length == 50) break;
    }
    if (ids.isEmpty) return const [];

    return _lookupAppleTracks(ids);
  }

  @override
  Future<List<Track>> discoverPlaylistTracks(PlatformPlaylist playlist) async {
    final document = html_parser.parse(await _getText(Uri.parse(playlist.url)));
    List<dynamic>? entries;
    for (final script in document.querySelectorAll(
      'script[type="application/ld+json"]',
    )) {
      try {
        final decoded = jsonDecode(script.text);
        if (decoded is Map && decoded['@type'] == 'MusicPlaylist') {
          final trackValue = decoded['track'];
          if (trackValue is List) entries = trackValue;
        }
      } on FormatException {
        continue;
      }
    }
    if (entries == null) {
      throw const FormatException('平台歌单页面缺少曲目数据');
    }
    final ids = <String>[];
    for (final value in entries) {
      if (value is! Map) continue;
      final url = _text(value['url']);
      final segments = url == null
          ? const <String>[]
          : Uri.parse(url).pathSegments;
      if (segments.isNotEmpty) ids.add(segments.last);
    }
    if (ids.isEmpty) throw const FormatException('平台歌单没有可识别的曲目');
    return _lookupAppleTracks(ids);
  }

  Future<List<Track>> _lookupAppleTracks(List<String> ids) async {
    final tracksById = <String, Track>{};
    for (var start = 0; start < ids.length; start += 50) {
      final end = (start + 50).clamp(0, ids.length);
      final uri = _lookupEndpoint.replace(
        queryParameters: {
          ..._lookupEndpoint.queryParameters,
          'id': ids.sublist(start, end).join(','),
          'country': 'CN',
        },
      );
      final lookup = await _getJson(uri);
      if (lookup is! Map<String, dynamic> || lookup['results'] is! List) {
        throw const FormatException('歌曲试听信息返回了无法识别的数据');
      }
      for (final track in _parseAppleResults(lookup['results'] as List)) {
        if (track.uri.isNotEmpty && track.sourceId != null) {
          tracksById[track.sourceId!] = track;
        }
      }
    }
    return ids
        .map((id) => tracksById[id])
        .whereType<Track>()
        .toList(growable: false);
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

  Future<String> _getText(Uri uri) async {
    final proxy = _proxyResolver(uri);
    try {
      return await _getTextOnce(uri, proxy);
    } on Object {
      if (proxy == 'DIRECT') rethrow;
      return _getTextOnce(uri, 'DIRECT');
    }
  }

  Future<String> _getTextOnce(Uri uri, String proxy) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 10)
      ..findProxy = (_) => proxy;
    try {
      final request = await client.getUrl(uri);
      request.headers.set(HttpHeaders.acceptHeader, 'text/html');
      request.headers.set(HttpHeaders.userAgentHeader, 'WCMusic/1.0');
      final response = await request.close().timeout(
        const Duration(seconds: 15),
      );
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('在线服务返回 ${response.statusCode}', uri: uri);
      }
      return await utf8.decoder.bind(response).join();
    } finally {
      client.close(force: true);
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
      final response = await request.close().timeout(
        const Duration(seconds: 15),
      );
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('在线服务返回 ${response.statusCode}', uri: uri);
      }
      return jsonDecode(await utf8.decoder.bind(response).join());
    } finally {
      client.close(force: true);
    }
  }

  List<Track> _parseAppleResults(List<dynamic> results) {
    final tracks = <Track>[];
    for (final value in results) {
      if (value is! Map) continue;
      final item = Map<String, dynamic>.from(value);
      final id = item['trackId']?.toString();
      final title = _text(item['trackName']);
      final artist = _text(item['artistName']);
      if (id == null || title == null || artist == null) continue;
      final durationMs = item['trackTimeMillis'];
      final releaseDate = DateTime.tryParse(_text(item['releaseDate']) ?? '');
      tracks.add(
        Track(
          id: 'apple-$id',
          title: title,
          artist: artist,
          album: _text(item['collectionName']) ?? '单曲',
          duration: Duration(
            milliseconds: durationMs is num ? durationMs.round() : 0,
          ),
          uri: _text(item['previewUrl']) ?? '',
          artworkUri: _largerArtwork(_text(item['artworkUrl100'])),
          source: TrackSource.custom,
          sourceId: id,
          quality: 'Apple Music · 试听',
          releaseDate: releaseDate,
        ),
      );
    }
    return tracks;
  }

  List<Track> _parseDeezerResults(List<dynamic> results) {
    final tracks = <Track>[];
    for (final value in results) {
      if (value is! Map) continue;
      final item = Map<String, dynamic>.from(value);
      final artistValue = item['artist'];
      final albumValue = item['album'];
      final artist = artistValue is Map ? _text(artistValue['name']) : null;
      final album = albumValue is Map ? _text(albumValue['title']) : null;
      final artwork = albumValue is Map
          ? _text(albumValue['cover_big'] ?? albumValue['cover_medium'])
          : null;
      final id = item['id']?.toString();
      final title = _text(item['title']);
      if (id == null || title == null || artist == null) continue;
      final seconds = item['duration'];
      tracks.add(
        Track(
          id: 'deezer-$id',
          title: title,
          artist: artist,
          album: album ?? '单曲',
          duration: Duration(seconds: seconds is num ? seconds.round() : 0),
          uri: _text(item['preview']) ?? '',
          artworkUri: artwork,
          source: TrackSource.custom,
          sourceId: id,
          quality: 'Deezer · 试听',
        ),
      );
    }
    return tracks;
  }

  String? _text(Object? value) {
    if (value is! String) return null;
    final text = value.trim();
    return text.isEmpty ? null : text;
  }

  String? _largerArtwork(String? value) =>
      value?.replaceFirst('100x100bb', '300x300bb');
}
