import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/domain/models/track.dart';
import 'package:wcmusic/domain/repositories/source_repository.dart';
import 'package:wcmusic/ui/features/player/player_view_model.dart';

import 'test_support.dart';

void main() {
  test(
    'keeps play and pause state aligned with synchronous player events',
    () async {
      final player = _SynchronousTogglePlayerService();
      final viewModel = PlayerViewModel(
        musicRepository: MemoryMusicRepository(),
        sourceRepository: MemorySourceRepository(),
        onlineSearchService: FakeOnlineSearchService(),
        playerService: player,
      );
      addTearDown(viewModel.dispose);
      const track = Track(
        id: 'online',
        title: '在线歌曲',
        artist: '歌手',
        album: '专辑',
        duration: Duration(minutes: 3),
        uri: 'https://audio.example/song.m4a',
      );

      await viewModel.playTrack(track);
      expect(viewModel.isPlaying, isTrue);

      await viewModel.togglePlayback();
      expect(viewModel.isPlaying, isFalse);

      await viewModel.togglePlayback();
      expect(viewModel.isPlaying, isTrue);
    },
  );

  test('controls playback position, volume, and mute restoration', () async {
    final player = FakePlayerService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(viewModel.tracks.first);

    await viewModel.seek(const Duration(minutes: 20).inMilliseconds.toDouble());
    expect(player.soughtPosition, viewModel.tracks.first.duration);

    await viewModel.setVolume(.35);
    expect(player.setVolumeValue, .35);
    await viewModel.toggleMute();
    expect(player.setVolumeValue, 0);
    await viewModel.toggleMute();
    expect(player.setVolumeValue, .35);
  });

  test('prefers a full track url resolved by an imported source', () async {
    const previewTrack = Track(
      id: 'apple-42',
      title: '目标歌曲',
      artist: '目标歌手',
      album: '测试专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.custom,
      sourceId: '42',
    );
    const matchedTrack = Track(
      id: 'apple-42',
      title: '目标歌曲',
      artist: '目标歌手',
      album: '测试专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.wy,
      sourceId: '123',
    );
    final player = FakePlayerService();
    final sourceRepository = _FullTrackSourceRepository();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: sourceRepository,
      onlineSearchService: FakeOnlineSearchService(
        const [],
        const [],
        const [],
        const {},
        matchedTrack,
      ),
      playerService: player,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    await viewModel.playTrack(previewTrack);

    expect(sourceRepository.resolvedTrack?.source, TrackSource.wy);
    expect(sourceRepository.resolvedTrack?.sourceId, '123');
    expect(player.playedTrack?.uri, 'https://audio.example/full.flac');
    expect(viewModel.current?.quality, '洛雪音源 · 整曲');
  });

  test('randomly selects at most four platform playlists', () async {
    final playlists = List.generate(
      6,
      (index) => PlatformPlaylist(
        id: '$index',
        name: '歌单 $index',
        artworkUri: 'https://image.example/$index.jpg',
        url: 'https://music.example/$index',
        platform: '测试平台',
      ),
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(const [], playlists),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    await viewModel.refreshPlatformPlaylists();

    expect(viewModel.platformPlaylists, hasLength(4));
    expect(
      viewModel.platformPlaylists.map((item) => item.id).toSet(),
      hasLength(4),
    );
  });

  test('favorites and unfavorites a platform playlist locally', () async {
    const platformPlaylist = PlatformPlaylist(
      id: '42',
      name: '今日热门',
      artworkUri: 'https://image.example/playlist.jpg',
      url: 'https://music.example/playlist/42',
      platform: '网易云音乐',
    );
    const playableTrack = Track(
      id: 'apple-7',
      title: '歌单歌曲',
      artist: '歌手',
      album: '专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
    );
    final repository = MemoryMusicRepository();
    final viewModel = PlayerViewModel(
      musicRepository: repository,
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [],
        const [],
        const [],
        const {
          '42': [playableTrack],
        },
      ),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    await viewModel.togglePlatformPlaylistFavorite(platformPlaylist);

    expect(viewModel.isPlatformPlaylistFavorite(platformPlaylist), isTrue);
    expect(viewModel.playlists.single.externalUrl, platformPlaylist.url);
    expect(viewModel.playlists.single.tracks, const [playableTrack]);

    await viewModel.togglePlatformPlaylistFavorite(platformPlaylist);

    expect(viewModel.isPlatformPlaylistFavorite(platformPlaylist), isFalse);
    expect(viewModel.playlists, isEmpty);
  });

  test('randomly selects four unique recently released tracks', () async {
    final newTracks = List.generate(
      6,
      (index) => Track(
        id: 'new-$index',
        title: '新曲 $index',
        artist: '歌手 $index',
        album: '新专辑',
        duration: const Duration(minutes: 3),
        uri: 'https://audio.example/$index.m4a',
        releaseDate: DateTime.now().subtract(Duration(days: index)),
      ),
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [],
        const [],
        newTracks,
      ),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    await viewModel.refreshRecentTracks();

    expect(viewModel.recentTracks, hasLength(4));
    expect(viewModel.recentTracks.map((item) => item.id).toSet(), hasLength(4));
  });

  test('updates the close-to-tray background playback setting', () async {
    final lifecycle = FakeWindowLifecycleService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      windowLifecycleService: lifecycle,
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    expect(viewModel.backgroundPlayback, isTrue);
    await viewModel.setBackgroundPlayback(false);

    expect(viewModel.backgroundPlayback, isFalse);
    expect(lifecycle.closeToTray, isFalse);
  });
}

class _SynchronousTogglePlayerService extends FakePlayerService {
  final _playing = StreamController<bool>.broadcast(sync: true);
  bool _state = false;

  @override
  Stream<bool> get playing => _playing.stream;

  @override
  Future<void> play(Track track) async {
    _state = true;
    _playing.add(_state);
  }

  @override
  Future<void> toggle() async {
    _state = !_state;
    _playing.add(_state);
  }

  @override
  Future<void> dispose() => _playing.close();
}

class _FullTrackSourceRepository implements SourceRepository {
  Track? resolvedTrack;

  @override
  Future<List<SourceScript>> loadSources() async => const [
    SourceScript(
      id: 'source',
      name: '测试音源',
      version: '1.0.0',
      author: 'WCMusic',
      description: '测试',
      sourceKeys: ['wy'],
      rawScript: '',
    ),
  ];

  @override
  Future<String> resolveUrl(Track track, {String quality = '320k'}) async {
    resolvedTrack = track;
    return 'https://audio.example/full.flac';
  }

  @override
  Future<SourceScript> importScript(String rawScript) =>
      throw UnimplementedError();

  @override
  Future<void> deleteSource(String id) => throw UnimplementedError();
}
