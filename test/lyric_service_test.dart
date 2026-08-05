import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/lyric_service.dart';
import 'package:wcmusic/domain/models/track.dart';

void main() {
  test('parses LRC timestamps and ignores metadata', () {
    final lines = parseLrc('''
[ti:测试歌曲]
[00:01.20]第一句
[00:03.005][00:05.50]重复句
''');

    expect(lines, hasLength(3));
    expect(lines.first.time, const Duration(milliseconds: 1200));
    expect(lines[1].time, const Duration(milliseconds: 3005));
    expect(lines.last.time, const Duration(milliseconds: 5500));
  });

  test('loads lyrics from Kuwo, Kugou, QQ and Netease formats', () async {
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      request.response.headers.contentType = ContentType.json;
      switch (request.uri.path) {
        case '/kuwo':
          request.response.write(
            jsonEncode({
              'data': {
                'lrclist': [
                  {'time': '1.25', 'lineLyric': '酷我歌词'},
                ],
              },
            }),
          );
        case '/kugou-search':
          request.response.write(
            jsonEncode({
              'candidates': [
                {'id': 'lyric-id', 'accesskey': 'key'},
              ],
            }),
          );
        case '/kugou-download':
          request.response.write(
            jsonEncode({
              'content': base64Encode(utf8.encode('[00:02.00]酷狗歌词')),
            }),
          );
        case '/qq':
          request.response.write(jsonEncode({'lyric': '[00:03.00]QQ 歌词'}));
        case '/netease':
          request.response.write(
            jsonEncode({
              'lrc': {'lyric': '[00:04.00]网易歌词'},
            }),
          );
      }
      await request.response.close();
    });
    Uri endpoint(String path) =>
        Uri.parse('http://127.0.0.1:${server.port}$path');
    final service = OnlineLyricService(
      kuwoEndpoint: endpoint('/kuwo'),
      kugouSearchEndpoint: endpoint('/kugou-search'),
      kugouDownloadEndpoint: endpoint('/kugou-download'),
      qqEndpoint: endpoint('/qq'),
      neteaseEndpoint: endpoint('/netease'),
    );

    final kuwo = await service.loadLyrics(_track(TrackSource.kw));
    final kugou = await service.loadLyrics(_track(TrackSource.kg));
    final qq = await service.loadLyrics(_track(TrackSource.tx));
    final netease = await service.loadLyrics(_track(TrackSource.wy));

    expect(kuwo.single.text, '酷我歌词');
    expect(kugou.single.text, '酷狗歌词');
    expect(qq.single.text, 'QQ 歌词');
    expect(netease.single.text, '网易歌词');
  });
}

Track _track(TrackSource source) => Track(
  id: 'track-${source.name}',
  title: '测试歌曲',
  artist: '测试歌手',
  album: '测试专辑',
  duration: const Duration(minutes: 3),
  uri: '',
  source: source,
  sourceId: 'source-id',
);
