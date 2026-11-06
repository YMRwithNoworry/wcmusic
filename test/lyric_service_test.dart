import 'dart:convert';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/lyric_service.dart';
import 'package:wcmusic/domain/models/lyric_line.dart';
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

  test('matches platform lyric translations by timestamp and skips duplicates', () {
    final lines = parseLrcWithTranslation(
      '''[00:01.00]Hello
[00:03.00]World
[00:05.00]你好''',
      '''[00:01.12]你好
[00:03.08]世界
[00:05.00]你好''',
    );

    expect(lines[0].translation, '你好');
    expect(lines[1].translation, '世界');
    expect(lines[2].translation, isNull);
  });

  test('auto translation targets only missing non-Chinese lyric lines', () {
    final lines = [
      const LyricLine(time: Duration.zero, text: 'Hello'),
      const LyricLine(
        time: Duration(seconds: 1),
        text: 'Already has a translation',
        translation: '已有翻译',
      ),
      const LyricLine(time: Duration(seconds: 2), text: '夜色渐浓'),
      const LyricLine(time: Duration(seconds: 3), text: 'こんにちは'),
      const LyricLine(time: Duration(seconds: 4), text: 'Привет'),
      const LyricLine(time: Duration(seconds: 5), text: '...'),
      const LyricLine(time: Duration(seconds: 6), text: ''),
    ];

    expect(translationTargets(lines), [0, 3, 4]);
    final merged = mergeLyricTranslations(lines, [0, 3, 4], ['你好', '你好', '你好']);
    expect(merged[0].translation, '你好');
    expect(merged[1].translation, '已有翻译');
    expect(merged[2].translation, isNull);
    expect(merged[3].translation, '你好');
    expect(merged[4].translation, '你好');
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
          request.response.write(
            jsonEncode({
              'lyric': '[00:03.00]QQ 歌词',
              'trans': '[00:03.10]QQ 翻译',
            }),
          );
        case '/netease':
          request.response.write(
            jsonEncode({
              'lrc': {'lyric': '[00:04.00]网易歌词'},
              'tlyric': {'lyric': '[00:04.20]网易翻译'},
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
    expect(qq.single.translation, 'QQ 翻译');
    expect(netease.single.text, '网易歌词');
    expect(netease.single.translation, '网易翻译');
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
