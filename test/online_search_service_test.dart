import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/online_search_service.dart';
import 'package:wcmusic/domain/models/track.dart';

void main() {
  test('searches Kuwo and maps its source id', () async {
    final server = await _jsonServer({
      'abslist': [
        {
          'DC_TARGETID': '42',
          'SONGNAME': '<em>晴天</em>',
          'ARTIST': '测试歌手',
          'ALBUM': '测试专辑',
          'DURATION': '245',
          'web_albumpic_short': '120/85/1/4091887608.jpg',
        },
      ],
    });
    addTearDown(() => server.close(force: true));
    final requestFuture = server.firstRequest;
    final service = MultiSourceOnlineSearchService(
      kuwoSearchEndpoint: server.endpoint,
    );

    final tracks = await service.search('  晴天  ', limit: 80);
    final requestUri = await requestFuture;

    expect(requestUri.queryParameters['all'], '晴天');
    expect(requestUri.queryParameters['rn'], '50');
    expect(tracks.single.title, '晴天');
    expect(tracks.single.duration, const Duration(minutes: 4, seconds: 5));
    expect(tracks.single.source, TrackSource.kw);
    expect(tracks.single.sourceId, '42');
    expect(
      tracks.single.artworkUri,
      'https://img1.kuwo.cn/star/albumcover/120/85/1/4091887608.jpg',
    );
    expect(tracks.single.uri, isEmpty);
  });

  test('searches Kugou and maps its hash', () async {
    final server = await _jsonServer({
      'data': {
        'lists': [
          {
            'FileHash': 'HASH-9',
            'SongName': '另一首歌',
            'SingerName': '另一位歌手',
            'AlbumName': '另一张专辑',
            'Duration': 188,
            'Image': 'https://image.example/{size}.jpg',
          },
        ],
      },
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      kugouSearchEndpoint: server.endpoint,
    );

    final tracks = await service.search(
      '另一首歌',
      channel: OnlineSearchChannel.kugou,
    );

    expect(tracks.single.id, 'kg-HASH-9');
    expect(tracks.single.source, TrackSource.kg);
    expect(tracks.single.sourceId, 'HASH-9');
    expect(tracks.single.artworkUri, 'https://image.example/400.jpg');
  });

  test('searches QQ Music and maps songmid', () async {
    final server = await _jsonServer({
      'data': {
        'song': {
          'list': [
            {
              'songmid': 'MID-7',
              'songname': 'QQ 歌曲',
              'singer': [
                {'name': 'QQ 歌手'},
              ],
              'albumname': 'QQ 专辑',
              'albummid': 'ALBUM-MID',
              'interval': 201,
            },
          ],
        },
      },
    });
    addTearDown(() => server.close(force: true));
    final requestFuture = server.firstRequest;
    final service = MultiSourceOnlineSearchService(
      qqSearchEndpoint: server.endpoint,
    );

    final tracks = await service.search(
      'QQ 歌曲',
      channel: OnlineSearchChannel.qqMusic,
    );
    final requestUri = await requestFuture;

    expect(requestUri.queryParameters['w'], 'QQ 歌曲');
    expect(tracks.single.source, TrackSource.tx);
    expect(tracks.single.sourceId, 'MID-7');
    expect(tracks.single.artist, 'QQ 歌手');
  });

  test('searches Netease and maps its song id', () async {
    final server = await _jsonServer({
      'result': {
        'songs': [_neteaseSong],
      },
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      neteaseSearchEndpoint: server.endpoint,
    );

    final tracks = await service.search(
      '网易歌曲',
      channel: OnlineSearchChannel.netease,
    );

    expect(tracks.single.source, TrackSource.wy);
    expect(tracks.single.sourceId, '123');
    expect(tracks.single.album, '网易专辑');
    expect(tracks.single.duration, const Duration(minutes: 3));
  });

  test('does not send a request for an empty keyword', () async {
    final service = MultiSourceOnlineSearchService();

    expect(await service.search('   '), isEmpty);
  });

  test('matches a legacy online track to Netease', () async {
    final server = await _jsonServer({
      'result': {
        'songs': [_neteaseSong],
      },
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      neteaseSearchEndpoint: server.endpoint,
    );
    const track = Track(
      id: 'legacy-42',
      title: '网易歌曲',
      artist: '网易歌手',
      album: '测试专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.custom,
      sourceId: '42',
    );

    final matched = await service.matchTrackToSources(track, const {'wy'});

    expect(matched?.source, TrackSource.wy);
    expect(matched?.sourceId, '123');
    expect(matched?.quality, '网易云音乐 · 整曲');
  });

  test('retries directly when the configured proxy is unavailable', () async {
    final server = await _jsonServer({
      'abslist': [
        {'DC_TARGETID': '7', 'SONGNAME': '直连歌曲', 'ARTIST': '测试歌手'},
      ],
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      kuwoSearchEndpoint: server.endpoint,
      proxyResolver: (_) => 'PROXY 127.0.0.1:1',
    );

    final tracks = await service.search('直连');

    expect(tracks.single.title, '直连歌曲');
  });

  test('loads hot Netease playlists', () async {
    final server = await _jsonServer({
      'playlists': [
        {
          'id': 42,
          'name': '今日热门',
          'coverImgUrl': 'https://image.example/playlist.jpg',
        },
      ],
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      playlistEndpoint: server.endpoint,
    );

    final playlists = await service.discoverPlaylists();

    expect(playlists.single.name, '今日热门');
    expect(playlists.single.platform, '网易云音乐');
    expect(playlists.single.url, 'https://music.163.com/#/playlist?id=42');
  });

  test('loads Netease playlist tracks with playable source ids', () async {
    final server = await _jsonServer({
      'result': {
        'tracks': [_neteaseSong],
      },
    });
    addTearDown(() => server.close(force: true));
    final requestFuture = server.firstRequest;
    final service = MultiSourceOnlineSearchService(
      playlistDetailEndpoint: server.endpoint,
    );
    const playlist = PlatformPlaylist(
      id: '42',
      name: '今日热门',
      artworkUri: '',
      url: 'https://music.163.com/#/playlist?id=42',
      platform: '网易云音乐',
    );

    final tracks = await service.discoverPlaylistTracks(playlist);
    final requestUri = await requestFuture;

    expect(requestUri.queryParameters['id'], '42');
    expect(tracks.single.source, TrackSource.wy);
    expect(tracks.single.sourceId, '123');
  });

  test('loads recently released Netease tracks', () async {
    final server = await _jsonServer({
      'data': [_neteaseSong],
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      newTracksEndpoint: server.endpoint,
    );

    final tracks = await service.discoverNewTracks();

    expect(tracks.single.title, '网易歌曲');
    expect(tracks.single.releaseDate, isNotNull);
    expect(tracks.single.source, TrackSource.wy);
  });

  test('loads ranking catalogs for Netease, QQ Music, and Kugou', () async {
    final neteaseServer = await _jsonServer({
      'list': [
        {'id': 1, 'name': '网易飙升榜', 'coverImgUrl': 'http://img/wy.jpg'},
      ],
    });
    final qqServer = await _jsonServer({
      'data': {
        'topList': [
          {'id': 2, 'topTitle': 'QQ 热歌榜', 'picUrl': 'http://img/qq.jpg'},
        ],
      },
    });
    final kugouServer = await _jsonServer({
      'data': {
        'info': [
          {
            'rankid': 3,
            'rankname': '酷狗 TOP500',
            'imgurl': 'http://img/{size}.jpg',
          },
        ],
      },
    });
    addTearDown(() => neteaseServer.close(force: true));
    addTearDown(() => qqServer.close(force: true));
    addTearDown(() => kugouServer.close(force: true));
    final service = MultiSourceOnlineSearchService(
      neteaseRankingsEndpoint: neteaseServer.endpoint,
      qqRankingsEndpoint: qqServer.endpoint,
      kugouRankingsEndpoint: kugouServer.endpoint,
    );

    final netease = await service.loadRankings(OnlineSearchChannel.netease);
    final qq = await service.loadRankings(OnlineSearchChannel.qqMusic);
    final kugou = await service.loadRankings(OnlineSearchChannel.kugou);
    final kuwo = await service.loadRankings(OnlineSearchChannel.kuwo);

    expect(netease.single.name, '网易飙升榜');
    expect(qq.single.name, 'QQ 热歌榜');
    expect(kugou.single.artworkUri, 'https://img/400.jpg');
    expect(kuwo, isNotEmpty);
    expect(
      kuwo.every((ranking) => ranking.artworkUri?.isNotEmpty ?? false),
      isTrue,
    );
  });

  test('loads and maps QQ ranking tracks', () async {
    final server = await _jsonServer({
      'songlist': [
        {
          'data': {
            'songmid': 'RANK-MID',
            'songname': '榜单歌曲',
            'singer': [
              {'name': '榜单歌手'},
            ],
            'albumname': '榜单专辑',
            'albummid': 'RANK-ALBUM',
            'interval': 210,
          },
        },
      ],
    });
    addTearDown(() => server.close(force: true));
    final service = MultiSourceOnlineSearchService(
      qqRankingTracksEndpoint: server.endpoint,
    );
    const ranking = PlatformRanking(
      id: '26',
      name: 'QQ 热歌榜',
      channel: OnlineSearchChannel.qqMusic,
    );

    final tracks = await service.loadRankingTracks(ranking);

    expect(tracks.single.id, 'tx-RANK-MID');
    expect(tracks.single.artist, '榜单歌手');
    expect(tracks.single.duration, const Duration(minutes: 3, seconds: 30));
  });

  test('loads Kuwo ranking tracks with per-song covers', () async {
    final rankingServer = await _jsonServer({
      'v9_pic2':
          'http://img4.kuwo.cn/star/albumcover/120/s4s81/95/playlist.jpg',
      'musiclist': [
        {
          'id': '624683929',
          'name': '酷我榜单歌曲',
          'artist': '榜单歌手',
          'album': '榜单专辑',
          'duration': '209',
        },
      ],
    });
    final songInfoServer = await _jsonServer({
      'data': {
        'songinfo': {
          'pic': 'http://img1.kwcdn.kuwo.cn/star/albumcover/240/song.jpg',
        },
      },
    });
    addTearDown(() => rankingServer.close(force: true));
    addTearDown(() => songInfoServer.close(force: true));
    final service = MultiSourceOnlineSearchService(
      kuwoRankingTracksEndpoint: rankingServer.endpoint,
      kuwoSongInfoEndpoint: songInfoServer.endpoint,
    );
    const ranking = PlatformRanking(
      id: '16',
      name: '酷我热歌榜',
      channel: OnlineSearchChannel.kuwo,
    );

    final tracks = await service.loadRankingTracks(ranking);

    expect(tracks.single.id, 'kw-624683929');
    expect(tracks.single.artist, '榜单歌手');
    expect(tracks.single.duration, const Duration(minutes: 3, seconds: 29));
    expect(
      tracks.single.artworkUri,
      'https://img1.kwcdn.kuwo.cn/star/albumcover/240/song.jpg',
    );
  });
}

const _neteaseSong = {
  'id': 123,
  'name': '网易歌曲',
  'artists': [
    {'name': '网易歌手'},
  ],
  'album': {
    'name': '网易专辑',
    'picUrl': 'https://image.example/netease.jpg',
    'publishTime': 1754006400000,
  },
  'duration': 180000,
};

Future<_TestServer> _jsonServer(Object body) async {
  final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
  final firstRequest = Completer<Uri>();
  server.listen((request) async {
    if (!firstRequest.isCompleted) firstRequest.complete(request.uri);
    request.response.headers.contentType = ContentType.json;
    request.response.write(jsonEncode(body));
    await request.response.close();
  });
  return _TestServer(server, firstRequest.future);
}

class _TestServer {
  const _TestServer(this.server, this.firstRequest);

  final HttpServer server;
  final Future<Uri> firstRequest;
  Uri get endpoint => Uri.parse('http://127.0.0.1:${server.port}/api');

  Future<void> close({required bool force}) => server.close(force: force);
}
