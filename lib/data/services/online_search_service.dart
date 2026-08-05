import 'dart:convert';
import 'dart:io';

import '../../domain/models/track.dart';

abstract interface class OnlineSearchService {
  Future<List<Track>> search(String query, {int limit = 30});
}

class AppleOnlineSearchService implements OnlineSearchService {
  AppleOnlineSearchService({
    HttpClient Function()? clientFactory,
    Uri? endpoint,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _endpoint = endpoint ?? Uri.https('itunes.apple.com', '/search');

  final HttpClient Function() _clientFactory;
  final Uri _endpoint;

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
    final client = _clientFactory()..connectionTimeout = const Duration(seconds: 10);
    try {
      final request = await client.getUrl(uri);
      request.headers.set(HttpHeaders.acceptHeader, 'application/json');
      request.headers.set(HttpHeaders.userAgentHeader, 'WCMusic/1.0');
      final response = await request.close().timeout(const Duration(seconds: 15));
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('在线搜索服务返回 ${response.statusCode}', uri: uri);
      }
      final body = await utf8.decoder.bind(response).join();
      final decoded = jsonDecode(body);
      if (decoded is! Map<String, dynamic> || decoded['results'] is! List) {
        throw const FormatException('在线搜索返回了无法识别的数据');
      }
      return _parseResults(decoded['results'] as List);
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
