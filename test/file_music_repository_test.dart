import 'dart:io';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:path/path.dart' as path;
import 'package:wcmusic/data/repositories/file_music_repository.dart';
import 'package:wcmusic/domain/models/track.dart';

void main() {
  test('starts with an empty library instead of demo tracks', () async {
    final root = await Directory.systemTemp.createTemp('wcmusic-library-');
    addTearDown(() => root.delete(recursive: true));
    final repository = FileMusicRepository(directoryProvider: () async => root);

    expect(await repository.loadTracks(), isEmpty);
    expect(await repository.loadPlaylists(), isEmpty);
  });

  test('copies imported audio and reloads it after source deletion', () async {
    final root = await Directory.systemTemp.createTemp('wcmusic-library-');
    final sourceRoot = await Directory.systemTemp.createTemp('wcmusic-source-');
    addTearDown(() => root.delete(recursive: true));
    addTearDown(() => sourceRoot.delete(recursive: true));
    final source = File(path.join(sourceRoot.path, '一首歌.mp3'));
    await source.writeAsBytes([1, 2, 3, 4], flush: true);
    final repository = FileMusicRepository(directoryProvider: () async => root);

    final imported = await repository.importAudioFiles([source.path]);
    final managedPath = imported.single.uri;
    await source.delete();

    expect(path.dirname(managedPath), path.join(root.path, 'music'));
    expect(await File(managedPath).readAsBytes(), [1, 2, 3, 4]);

    final reloaded = FileMusicRepository(directoryProvider: () async => root);
    final tracks = await reloaded.loadTracks();
    expect(tracks, hasLength(1));
    expect(tracks.single.title, '一首歌');
    expect(tracks.single.uri, managedPath);
    expect(await File(tracks.single.uri).exists(), isTrue);
  });

  test('copies local files referenced by an imported M3U playlist', () async {
    final root = await Directory.systemTemp.createTemp('wcmusic-library-');
    final sourceRoot = await Directory.systemTemp.createTemp('wcmusic-source-');
    addTearDown(() => root.delete(recursive: true));
    addTearDown(() => sourceRoot.delete(recursive: true));
    final source = File(path.join(sourceRoot.path, 'relative.mp3'));
    await source.writeAsBytes([5, 6, 7], flush: true);
    final playlist = File(path.join(sourceRoot.path, 'songs.m3u'));
    final repository = FileMusicRepository(directoryProvider: () async => root);

    await repository.importFile(
      playlist.path,
      utf8.encode('#EXTM3U\n#EXTINF:12,歌手 - 歌名\nrelative.mp3'),
    );
    await source.delete();

    final tracks = await repository.loadTracks();
    expect(tracks.single.title, '歌名');
    expect(path.dirname(tracks.single.uri), path.join(root.path, 'music'));
    expect(await File(tracks.single.uri).readAsBytes(), [5, 6, 7]);
  });

  test('persists and removes a favorite platform playlist', () async {
    final root = await Directory.systemTemp.createTemp('wcmusic-library-');
    addTearDown(() => root.delete(recursive: true));
    final repository = FileMusicRepository(directoryProvider: () async => root);
    const favorite = Playlist(
      id: 'platform-Apple Music-42',
      name: '今日热门',
      tracks: [],
      artworkUri: 'https://image.example/playlist.jpg',
      externalUrl: 'https://music.example/playlist/42',
      platform: 'Apple Music',
    );

    await repository.savePlaylist(favorite);

    final reloaded = FileMusicRepository(directoryProvider: () async => root);
    final playlists = await reloaded.loadPlaylists();
    expect(playlists.single.name, '今日热门');
    expect(playlists.single.artworkUri, favorite.artworkUri);
    expect(playlists.single.externalUrl, favorite.externalUrl);
    expect(playlists.single.isPlatformFavorite, isTrue);

    await reloaded.deletePlaylist(favorite.id);
    final afterDeletion = FileMusicRepository(
      directoryProvider: () async => root,
    );
    expect(await afterDeletion.loadPlaylists(), isEmpty);
  });
}
