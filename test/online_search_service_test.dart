import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/online_search_service.dart';

void main() {
  test('searches online and maps playable tracks', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final requestFuture = server.first.then((request) async {
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        jsonEncode({
          'resultCount': 1,
          'results': [
            {
              'trackId': 42,
              'trackName': '晴天',
              'artistName': '测试歌手',
              'collectionName': '测试专辑',
              'trackTimeMillis': 245000,
              'previewUrl': 'https://audio.example/preview.m4a',
              'artworkUrl100': 'https://image.example/100x100bb.jpg',
            },
          ],
        }),
      );
      await request.response.close();
      return request.uri;
    });
    final service = AppleOnlineSearchService(
      endpoint: Uri.parse('http://127.0.0.1:${server.port}/search'),
    );

    final tracks = await service.search('  晴天  ', limit: 80);
    final requestUri = await requestFuture;

    expect(requestUri.queryParameters['term'], '晴天');
    expect(requestUri.queryParameters['entity'], 'song');
    expect(requestUri.queryParameters['limit'], '50');
    expect(tracks, hasLength(1));
    expect(tracks.single.title, '晴天');
    expect(tracks.single.duration, const Duration(minutes: 4, seconds: 5));
    expect(tracks.single.uri, 'https://audio.example/preview.m4a');
    expect(tracks.single.artworkUri, 'https://image.example/300x300bb.jpg');
    expect(tracks.single.sourceId, '42');
  });

  test('does not send a request for an empty keyword', () async {
    final service = AppleOnlineSearchService();

    expect(await service.search('   '), isEmpty);
  });

  test('retries directly when the configured proxy is unavailable', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.first.then((request) async {
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        jsonEncode({
          'results': [
            {'trackId': 7, 'trackName': '直连歌曲', 'artistName': '测试歌手'},
          ],
        }),
      );
      await request.response.close();
    });
    final service = AppleOnlineSearchService(
      endpoint: Uri.parse('http://127.0.0.1:${server.port}/search'),
      proxyResolver: (_) => 'PROXY 127.0.0.1:1',
    );

    final tracks = await service.search('直连');

    expect(tracks.single.title, '直连歌曲');
  });

  test('loads platform playlists from the public chart feed', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.first.then((request) async {
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        jsonEncode({
          'feed': {
            'results': [
              {
                'id': 'playlist-42',
                'name': '今日热门',
                'artworkUrl100': 'https://image.example/100x100SC.jpg',
                'url': 'https://music.example/playlist-42',
              },
            ],
          },
        }),
      );
      await request.response.close();
    });
    final service = AppleOnlineSearchService(
      playlistEndpoint: Uri.parse('http://127.0.0.1:${server.port}/playlists'),
    );

    final playlists = await service.discoverPlaylists();

    expect(playlists.single.name, '今日热门');
    expect(playlists.single.platform, 'Apple Music');
    expect(playlists.single.artworkUri, 'https://image.example/600x600SC.jpg');
    expect(playlists.single.url, 'https://music.example/playlist-42');
  });

  test('loads recently released playable tracks from the chart feed', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    final recentDate = DateTime.now().toUtc().subtract(
      const Duration(days: 10),
    );
    final oldDate = DateTime.now().toUtc().subtract(const Duration(days: 500));
    Uri? lookupRequest;
    server.listen((request) async {
      request.response.headers.contentType = ContentType.json;
      if (request.uri.path == '/feed') {
        request.response.write(
          jsonEncode({
            'feed': {
              'results': [
                {
                  'id': 'recent-42',
                  'releaseDate': recentDate.toIso8601String(),
                },
                {'id': 'old-7', 'releaseDate': oldDate.toIso8601String()},
              ],
            },
          }),
        );
      } else if (request.uri.path == '/lookup') {
        lookupRequest = request.uri;
        request.response.write(
          jsonEncode({
            'results': [
              {
                'trackId': 42,
                'trackName': '十天前的新歌',
                'artistName': '新歌手',
                'collectionName': '新专辑',
                'trackTimeMillis': 180000,
                'previewUrl': 'https://audio.example/new.m4a',
                'artworkUrl100': 'https://image.example/100x100bb.jpg',
                'releaseDate': recentDate.toIso8601String(),
              },
            ],
          }),
        );
      }
      await request.response.close();
    });
    final service = AppleOnlineSearchService(
      newTracksEndpoint: Uri.parse('http://127.0.0.1:${server.port}/feed'),
      lookupEndpoint: Uri.parse('http://127.0.0.1:${server.port}/lookup'),
    );

    final tracks = await service.discoverNewTracks();

    expect(lookupRequest?.queryParameters['id'], 'recent-42');
    expect(lookupRequest?.queryParameters['country'], 'CN');
    expect(tracks, hasLength(1));
    expect(tracks.single.title, '十天前的新歌');
    expect(tracks.single.uri, 'https://audio.example/new.m4a');
    expect(tracks.single.releaseDate, recentDate);
  });

  test('maps Deezer search results into playable tracks', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.first.then((request) async {
      request.response.headers.contentType = ContentType.json;
      request.response.write(
        jsonEncode({
          'data': [
            {
              'id': 9,
              'title': '另一首歌',
              'duration': 188,
              'preview': 'https://audio.example/deezer.mp3',
              'artist': {'name': '另一位歌手'},
              'album': {
                'title': '另一张专辑',
                'cover_big': 'https://image.example/deezer.jpg',
              },
            },
          ],
        }),
      );
      await request.response.close();
    });
    final service = AppleOnlineSearchService(
      deezerEndpoint: Uri.parse('http://127.0.0.1:${server.port}/deezer'),
    );

    final tracks = await service.search(
      '另一首歌',
      channel: OnlineSearchChannel.deezer,
    );

    expect(tracks.single.id, 'deezer-9');
    expect(tracks.single.artist, '另一位歌手');
    expect(tracks.single.uri, 'https://audio.example/deezer.mp3');
    expect(tracks.single.quality, 'Deezer · 试听');
  });

  test('aggregates channels and removes duplicate songs', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      request.response.headers.contentType = ContentType.json;
      if (request.uri.path == '/apple') {
        request.response.write(
          jsonEncode({
            'results': [
              {'trackId': 1, 'trackName': '同一首歌', 'artistName': '同一位歌手'},
            ],
          }),
        );
      } else {
        request.response.write(
          jsonEncode({
            'data': [
              {
                'id': 2,
                'title': '同一首歌',
                'artist': {'name': '同一位歌手'},
                'album': {'title': '聚合专辑'},
              },
            ],
          }),
        );
      }
      await request.response.close();
    });
    final service = AppleOnlineSearchService(
      endpoint: Uri.parse('http://127.0.0.1:${server.port}/apple'),
      deezerEndpoint: Uri.parse('http://127.0.0.1:${server.port}/deezer'),
    );

    final tracks = await service.search(
      '同一首歌',
      channel: OnlineSearchChannel.aggregate,
    );

    expect(tracks, hasLength(1));
    expect(tracks.single.title, '同一首歌');
  });
}
