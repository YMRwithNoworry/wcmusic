import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/data/services/floating_lyrics_service.dart';
import 'package:wcmusic/data/services/lyric_service.dart';
import 'package:wcmusic/data/services/track_download_service.dart';
import 'package:wcmusic/domain/models/lyric_line.dart';
import 'package:wcmusic/domain/models/lyrics_overlay_style.dart';
import 'package:wcmusic/domain/models/playback_mode.dart';
import 'package:wcmusic/domain/models/playback_quality.dart';
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

  test('synchronizes floating lyrics with playback position', () async {
    final player = _PositionPlayerService();
    final overlay = _FakeFloatingLyricsService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
      lyricService: _FakeLyricService(),
      floatingLyricsService: overlay,
    );
    addTearDown(viewModel.dispose);
    const track = Track(
      id: 'lyric-track',
      title: '歌词歌曲',
      artist: '歌词歌手',
      album: '歌词专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/song.mp3',
      source: TrackSource.wy,
      sourceId: '123',
    );

    await viewModel.playTrack(track);
    await viewModel.setFloatingLyrics(true);
    expect(overlay.currentLine, '第一句');
    expect(overlay.nextLine, '第二句');
    expect(overlay.lines, containsAll(['第一句', '第二句']));
    expect(overlay.currentIndex, 0);

    player.emitPosition(const Duration(seconds: 6));
    await Future<void>.delayed(Duration.zero);

    expect(overlay.currentLine, '第二句');
    expect(overlay.nextLine, isEmpty);
    expect(overlay.currentIndex, 1);
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
    expect(viewModel.current?.quality, '洛雪音源 · 高品 320k');
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

  test('selects a loaded source for full-track resolution', () async {
    final sourceRepository = _FullTrackSourceRepository();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: sourceRepository,
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    await viewModel.selectSource('source');

    expect(viewModel.selectedSourceId, 'source');
    expect(viewModel.selectedSource?.name, '测试音源');
    expect(sourceRepository.selectedId, 'source');

    await viewModel.selectSource(null);

    expect(viewModel.selectedSourceId, isNull);
    expect(sourceRepository.selectedId, isNull);
  });

  test('cycles playback modes in a fixed order', () async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);

    expect(viewModel.playbackMode, PlaybackMode.listLoop);

    viewModel.cyclePlaybackMode();
    expect(viewModel.playbackMode, PlaybackMode.shuffle);

    viewModel.cyclePlaybackMode();
    expect(viewModel.playbackMode, PlaybackMode.singleLoop);

    viewModel.cyclePlaybackMode();
    expect(viewModel.playbackMode, PlaybackMode.sequence);

    viewModel.cyclePlaybackMode();
    expect(viewModel.playbackMode, PlaybackMode.listLoop);
  });

  test('list loop advances and wraps when a track completes', () async {
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
    player.emitCompleted();
    await Future<void>.delayed(Duration.zero);
    expect(viewModel.current?.id, testLibraryTracks[1].id);

    for (var index = 1; index < testLibraryTracks.length; index++) {
      player.emitCompleted();
      await Future<void>.delayed(Duration.zero);
    }
    expect(viewModel.current?.id, testLibraryTracks.first.id);
  });

  test('sequence playback stops after the last track', () async {
    final player = FakePlayerService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    while (viewModel.playbackMode != PlaybackMode.sequence) {
      viewModel.cyclePlaybackMode();
    }

    await viewModel.playTrack(viewModel.tracks.first);
    for (var index = 1; index < testLibraryTracks.length; index++) {
      player.emitCompleted();
      await Future<void>.delayed(Duration.zero);
    }
    expect(viewModel.current?.id, testLibraryTracks.last.id);

    player.emitCompleted();
    await Future<void>.delayed(Duration.zero);

    expect(viewModel.isPlaying, isFalse);
    expect(player.stopped, isTrue);
  });

  test('single loop replays the current track', () async {
    const track = Track(
      id: 'single',
      title: '单曲',
      artist: '歌手',
      album: '专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/single.mp3',
    );
    final player = FakePlayerService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: const [track]),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    while (viewModel.playbackMode != PlaybackMode.singleLoop) {
      viewModel.cyclePlaybackMode();
    }

    await viewModel.playTrack(viewModel.tracks.single);
    player.emitCompleted();
    await Future<void>.delayed(Duration.zero);

    expect(viewModel.current?.id, 'single');
    expect(player.playedTrack?.id, 'single');
  });

  test('shuffle advances to a different track', () async {
    final player = FakePlayerService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    while (viewModel.playbackMode != PlaybackMode.shuffle) {
      viewModel.cyclePlaybackMode();
    }

    await viewModel.playTrack(viewModel.tracks.first);
    player.emitCompleted();
    await Future<void>.delayed(Duration.zero);

    expect(viewModel.current?.id, isNot(testLibraryTracks.first.id));
  });

  test('requests the selected quality when resolving full tracks', () async {
    const previewTrack = Track(
      id: 'quality-preview',
      title: '音质歌曲',
      artist: '音质歌手',
      album: '音质专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.custom,
      sourceId: '42',
    );
    const matchedTrack = Track(
      id: 'quality-preview',
      title: '音质歌曲',
      artist: '音质歌手',
      album: '音质专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.wy,
      sourceId: '123',
    );
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
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    await viewModel.setPlaybackQuality(PlaybackQuality.lossless);
    await viewModel.playTrack(previewTrack);

    expect(sourceRepository.resolvedQuality, 'flac');
    expect(viewModel.current?.quality, contains('无损 flac'));
  });

  test('downloads the current online track to a local file', () async {
    const track = Track(
      id: 'download',
      title: '下载歌曲',
      artist: '下载歌手',
      album: '下载专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/song.mp3',
    );
    final downloader = _FakeTrackDownloadService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: const [track]),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      downloadService: downloader,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    await viewModel.playTrack(track);
    await viewModel.downloadCurrentTrack();

    expect(downloader.downloadedTrack?.id, 'download');
    expect(downloader.downloadedTrack?.uri, downloader.cachedPath);
    expect(viewModel.message, contains('已下载到'));
  });

  test('resolves the selected quality before downloading', () async {
    const track = Track(
      id: 'quality-download',
      title: '无损下载',
      artist: '下载歌手',
      album: '下载专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.wy,
      sourceId: 'quality-42',
    );
    final sourceRepository = _FullTrackSourceRepository();
    final downloader = _FakeTrackDownloadService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: const [track]),
      sourceRepository: sourceRepository,
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      downloadService: downloader,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(track);

    await viewModel.downloadCurrentTrack(PlaybackQuality.lossless);

    expect(sourceRepository.resolvedQuality, 'flac');
    expect(downloader.downloadedTrack?.uri, 'https://audio.example/full.flac');
    expect(downloader.downloadedTrack?.quality, contains('无损 flac'));
    expect(viewModel.message, contains('无损 flac 已下载到'));
  });

  test('downloads network tracks to cache before playing', () async {
    const track = Track(
      id: 'network-play',
      title: '网络歌曲',
      artist: '网络歌手',
      album: '网络专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/song.mp3',
    );
    final downloader = _FakeTrackDownloadService();
    final player = FakePlayerService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: const [track]),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: player,
      downloadService: downloader,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    await viewModel.playTrack(track);

    expect(downloader.downloadedTrack?.id, 'network-play');
    expect(player.playedTrack?.uri, downloader.cachedPath);
    expect(viewModel.current?.uri, downloader.cachedPath);
  });

  test('updates and persists the desktop lyrics style', () async {
    final overlay = _FakeFloatingLyricsService();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      floatingLyricsService: overlay,
    );
    addTearDown(viewModel.dispose);
    const style = LyricsOverlayStyle(
      fontFamily: 'SimHei',
      fontSize: 36,
      alignment: 'right',
      textColor: 0xFFFF0000,
      backgroundColor: 0xFF000000,
      opacity: 0.5,
      cornerRadius: 8,
      locked: false,
    );

    await viewModel.updateLyricsStyle(style);

    expect(overlay.style?.fontFamily, 'SimHei');
    expect(overlay.savedStyle?.locked, isFalse);
    expect(viewModel.lyricsStyle.fontSize, 36);
  });

  test('creates folders and favorites tracks into them', () async {
    final repository = MemoryMusicRepository(initialTracks: testLibraryTracks);
    final viewModel = PlayerViewModel(
      musicRepository: repository,
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    final folder = await viewModel.createFolder('  我的收藏  ');
    expect(folder?.name, '我的收藏');
    await viewModel.addTrackToFolder(folder!.id, testLibraryTracks.first.id);

    expect(
      viewModel.folders.single.trackIds,
      contains(testLibraryTracks.first.id),
    );
    viewModel.selectFolder(folder.id);
    expect(viewModel.folderTracks.single.id, testLibraryTracks.first.id);
  });

  test('persists an online track before adding it to a folder', () async {
    final repository = MemoryMusicRepository();
    final viewModel = PlayerViewModel(
      musicRepository: repository,
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    final folder = await viewModel.createFolder('在线收藏');
    const onlineTrack = Track(
      id: 'online-persisted',
      title: '在线歌曲',
      artist: '歌手',
      album: '专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/song.mp3',
      source: TrackSource.custom,
    );

    await viewModel.favoriteTrack(folder!.id, onlineTrack);

    final reloaded = PlayerViewModel(
      musicRepository: repository,
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(reloaded.dispose);
    await reloaded.load();
    reloaded.selectFolder(folder.id);
    expect(reloaded.folderTracks, const [onlineTrack]);
  });

  test('imports a folder of sources and reports failures', () async {
    final repository = _BulkSourceRepository();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: repository,
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    final result = await viewModel.importSources([
      (name: 'a.js', script: 'ok-1'),
      (name: 'b.js', script: 'bad'),
      (name: 'c.js', script: 'ok-2'),
    ]);

    expect(result.imported, 2);
    expect(result.failures.single, contains('b.js'));
    expect(repository.imported, 2);
    expect(viewModel.isImportingSources, isFalse);
  });

  test(
    'plays a searched track through the selected source on its platform',
    () async {
      const kwTrack = Track(
        id: 'kw-42',
        title: '酷我歌曲',
        artist: '酷我歌手',
        album: '酷我专辑',
        duration: Duration(minutes: 3),
        uri: '',
        source: TrackSource.kw,
        sourceId: '42',
      );
      final sourceRepository = _FullTrackSourceRepository(sourceKeys: ['kw']);
      final viewModel = PlayerViewModel(
        musicRepository: MemoryMusicRepository(),
        sourceRepository: sourceRepository,
        onlineSearchService: FakeOnlineSearchService(),
        playerService: FakePlayerService(),
      );
      addTearDown(viewModel.dispose);
      await viewModel.load();
      await viewModel.selectSource('source');

      await viewModel.playTrack(kwTrack);

      expect(sourceRepository.resolvedTrack?.source, TrackSource.kw);
      expect(sourceRepository.resolvedTrack?.sourceId, '42');
      expect(viewModel.current?.uri, 'https://audio.example/full.flac');
    },
  );

  test('keeps the selected platform when the chosen source lacks it', () async {
    const kwTrack = Track(
      id: 'kw-42',
      title: '酷我歌曲',
      artist: '酷我歌手',
      album: '酷我专辑',
      duration: Duration(minutes: 3),
      uri: '',
      source: TrackSource.kw,
      sourceId: '42',
    );
    final sourceRepository = _FullTrackSourceRepository(sourceKeys: ['wy']);
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: sourceRepository,
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.selectSource('source');

    await viewModel.playTrack(kwTrack);

    expect(viewModel.current?.uri, isEmpty);
    expect(viewModel.message, contains('不支持'));
    expect(sourceRepository.resolvedTrack, isNull);
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

class _PositionPlayerService extends FakePlayerService {
  final _positions = StreamController<Duration>.broadcast(sync: true);

  @override
  Stream<Duration> get position => _positions.stream;

  void emitPosition(Duration position) => _positions.add(position);

  @override
  Future<void> dispose() => _positions.close();
}

class _FakeLyricService implements LyricService {
  @override
  Future<List<LyricLine>> loadLyrics(Track track) async => const [
    LyricLine(time: Duration.zero, text: '第一句'),
    LyricLine(time: Duration(seconds: 5), text: '第二句'),
  ];
}

class _FakeFloatingLyricsService implements FloatingLyricsService {
  String currentLine = '';
  String nextLine = '';
  List<String> lines = const [];
  int currentIndex = -1;
  LyricsOverlayStyle? style;
  LyricsOverlayStyle? savedStyle;

  @override
  bool get isSupported => true;

  @override
  Future<bool> setEnabled(bool enabled) async => enabled;

  @override
  Future<void> update({
    required String title,
    required String currentLine,
    required String nextLine,
    List<String> lines = const [],
    int currentIndex = -1,
  }) async {
    this.currentLine = currentLine;
    this.nextLine = nextLine;
    this.lines = lines;
    this.currentIndex = currentIndex;
  }

  @override
  Future<void> setStyle(LyricsOverlayStyle style) async {
    this.style = style;
  }

  @override
  Future<LyricsOverlayStyle> loadStyle() async =>
      style ?? const LyricsOverlayStyle();

  @override
  Future<void> saveStyle(LyricsOverlayStyle style) async {
    this.style = style;
    savedStyle = style;
  }

  @override
  Future<void> dispose() async {}
}

class _FullTrackSourceRepository implements SourceRepository {
  _FullTrackSourceRepository({this.sourceKeys = const ['wy']});

  final List<String> sourceKeys;
  Track? resolvedTrack;
  String? selectedId;
  String? resolvedQuality;

  @override
  Future<List<SourceScript>> loadSources() async => [
    SourceScript(
      id: 'source',
      name: '测试音源',
      version: '1.0.0',
      author: 'WCMusic',
      description: '测试',
      sourceKeys: sourceKeys,
      rawScript: '',
    ),
  ];

  @override
  Future<String?> loadSelectedSourceId() async => selectedId;

  @override
  Future<void> selectSource(String? id) async {
    selectedId = id;
  }

  @override
  Future<String> resolveUrl(
    Track track, {
    String quality = '320k',
    bool background = false,
  }) async {
    resolvedTrack = track;
    resolvedQuality = quality;
    return 'https://audio.example/full.flac';
  }

  @override
  Future<SourceScript> importScript(String rawScript) =>
      throw UnimplementedError();

  @override
  Future<void> deleteSource(String id) => throw UnimplementedError();
}

class _FakeTrackDownloadService extends TrackDownloadService {
  Track? downloadedTrack;
  String cachedPath = 'C:/cache/wcmusic-network-play.mp3';

  @override
  Future<String> download(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
  }) async {
    downloadedTrack = track;
    onProgress(1);
    return 'D:/music/${track.title}.mp3';
  }

  @override
  Future<String> downloadToCache(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
    bool Function()? shouldCancel,
  }) async {
    downloadedTrack = track;
    onProgress(1);
    return cachedPath;
  }
}

class _BulkSourceRepository implements SourceRepository {
  int imported = 0;

  @override
  Future<List<SourceScript>> loadSources() async => const [];

  @override
  Future<String?> loadSelectedSourceId() async => null;

  @override
  Future<void> selectSource(String? id) async {}

  @override
  Future<SourceScript> importScript(String rawScript) async {
    if (rawScript == 'bad') throw const FormatException('校验失败');
    imported++;
    return SourceScript(
      id: 'source-$imported',
      name: '批量源 $imported',
      version: '1.0.0',
      author: 'WCMusic',
      description: '测试',
      sourceKeys: const [],
      rawScript: rawScript,
    );
  }

  @override
  Future<void> deleteSource(String id) async {}

  @override
  Future<String> resolveUrl(
    Track track, {
    String quality = '320k',
    bool background = false,
  }) async => '';
}
