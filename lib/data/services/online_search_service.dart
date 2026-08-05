import 'dart:convert';
import 'dart:io';

import '../../domain/models/track.dart';

abstract interface class OnlineSearchService {
  Future<List<Track>> search(String query, {int limit = 30});
  Future<List<PlatformPlaylist>> discoverPlaylists();
}

class AppleOnlineSearchService implements OnlineSearchService {
  AppleOnlineSearchService({
    HttpClient Function()? clientFactory,
    Uri? endpoint,
    Uri? playlistEndpoint,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _endpoint = endpoint ?? Uri.https('itunes.apple.com', '/search'),
       _playlistEndpoint =
           playlistEndpoint ??
           Uri.https(
             'rss.marketingtools.apple.com',
             '/api/v2/cn/music/most-played/20/playlists.json',
           );

  final HttpClient Function() _clientFactory;
  final Uri _endpoint;
  final Uri _playlistEndpoint;

  @override
  Future<List<Track>> search(String query, {int limit = 30}) async {
    final keyword = query.trim();
    if (keyword.isEmpty) return const [];

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
    return _parseResults(decoded['results'] as List);
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

  Future<dynamic> _getJson(Uri uri) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 10);
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

  List<Track> _parseResults(List<dynamic> results) {
    final tracks = <Track>[];
    for (final value in results) {
      if (value is! Map) continue;
      final item = Map<String, dynamic>.from(value);
      final id = item['trackId']?.toString();
      final title = _text(item['trackName']);
      final artist = _text(item['artistName']);
      if (id == null || title == null || artist == null) continue;
      final durationMs = item['trackTimeMillis'];
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
          quality: '试听',
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
