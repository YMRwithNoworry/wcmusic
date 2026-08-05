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
}
