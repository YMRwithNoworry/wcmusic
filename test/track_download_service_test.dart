import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/track_download_service.dart';
import 'package:wcmusic/domain/models/track.dart';

void main() {
  test('streams a remote track to the chosen local path', () async {
    final bytes = List<int>.generate(1024, (index) => index % 251);
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      request.response.headers.contentType = ContentType.binary;
      request.response.contentLength = bytes.length;
      request.response.add(bytes);
      await request.response.close();
    });
    final directory = await Directory.systemTemp.createTemp('wcmusic-dl-');
    addTearDown(() => directory.delete(recursive: true));
    final target = '${directory.path}${Platform.pathSeparator}download.mp3';
    final service = TrackDownloadService(
      pathResolver: (track, extension) async => target,
    );
    final progress = <double>[];

    final path = await service.download(
      Track(
        id: 'download',
        title: '下载歌曲',
        artist: '下载歌手',
        album: '下载专辑',
        duration: const Duration(minutes: 3),
        uri: 'http://127.0.0.1:${server.port}/song.mp3',
      ),
      onProgress: progress.add,
    );

    expect(path, target);
    expect(await File(path).readAsBytes(), bytes);
    expect(progress.last, 1.0);
  });

  test('throws when the download is cancelled', () async {
    final service = TrackDownloadService(
      pathResolver: (track, extension) async => null,
    );

    expect(
      () => service.download(
        Track(
          id: 'cancel',
          title: '取消',
          artist: '歌手',
          album: '专辑',
          duration: const Duration(minutes: 3),
          uri: 'https://audio.example/song.mp3',
        ),
        onProgress: (_) {},
      ),
      throwsA(isA<StateError>()),
    );
  });
}
