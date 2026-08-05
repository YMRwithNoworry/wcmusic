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

  test('streams a remote track into the cache directory', () async {
    final bytes = List<int>.generate(512, (index) => index % 251);
    final server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
    addTearDown(() => server.close(force: true));
    server.listen((request) async {
      request.response.headers.contentType = ContentType.binary;
      request.response.contentLength = bytes.length;
      request.response.add(bytes);
      await request.response.close();
    });
    final directory = await Directory.systemTemp.createTemp('wcmusic-cache-');
    addTearDown(() => directory.delete(recursive: true));
    final service = TrackDownloadService(
      cacheDirectoryProvider: () async => directory,
    );

    final path = await service.downloadToCache(
      Track(
        id: 'cache-song',
        title: '缓存歌曲',
        artist: '缓存歌手',
        album: '缓存专辑',
        duration: const Duration(minutes: 3),
        uri: 'http://127.0.0.1:${server.port}/song.mp3',
      ),
      onProgress: (_) {},
    );

    expect(await File(path).readAsBytes(), bytes);
  });

  test('copies a cached local track to the chosen path', () async {
    final directory = await Directory.systemTemp.createTemp('wcmusic-copy-');
    addTearDown(() => directory.delete(recursive: true));
    final source = File(
      '${directory.path}${Platform.pathSeparator}cached.flac',
    );
    final target = '${directory.path}${Platform.pathSeparator}download.flac';
    final bytes = List<int>.generate(768, (index) => index % 251);
    await source.writeAsBytes(bytes);
    String? capturedExtension;
    final service = TrackDownloadService(
      pathResolver: (track, extension) async {
        capturedExtension = extension;
        return target;
      },
    );
    final progress = <double>[];

    final path = await service.download(
      Track(
        id: 'cached-song',
        title: '缓存歌曲',
        artist: '缓存歌手',
        album: '缓存专辑',
        duration: const Duration(minutes: 3),
        uri: source.path,
      ),
      onProgress: progress.add,
    );

    expect(path, target);
    expect(capturedExtension, '.flac');
    expect(await File(path).readAsBytes(), bytes);
    expect(progress, [1.0]);
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

  test('uses the fallback extension for php-like audio urls', () async {
    String? capturedExtension;
    final service = TrackDownloadService(
      pathResolver: (track, extension) async {
        capturedExtension = extension;
        return null;
      },
    );

    await expectLater(
      service.download(
        Track(
          id: 'php-url',
          title: 'PHP 地址',
          artist: '歌手',
          album: '专辑',
          duration: const Duration(minutes: 3),
          uri: 'https://audio.example/wy.php?type=mp3&id=42',
        ),
        onProgress: (_) {},
      ),
      throwsA(isA<StateError>()),
    );

    expect(capturedExtension, '.mp3');
  });
}
