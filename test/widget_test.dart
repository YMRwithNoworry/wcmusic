import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/data/services/online_search_service.dart';
import 'package:wcmusic/data/services/track_download_service.dart';
import 'package:wcmusic/ui/core/remote_artwork.dart';
import 'package:wcmusic/ui/features/player/player_view_model.dart';
import 'package:wcmusic/ui/features/player/playback_controls.dart';
import 'package:wcmusic/ui/features/player/now_playing_view.dart';
import 'package:wcmusic/ui/features/player/player_bar.dart';
import 'package:wcmusic/domain/models/track.dart';

import 'test_support.dart';

void main() {
  testWidgets('renders the music workspace', (tester) async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();

    expect(find.text('让声音自然生长'), findsOneWidget);
    expect(find.text('Seedling'), findsWidgets);
  });

  testWidgets('opens online search and displays results', (tester) async {
    const result = Track(
      id: 'online-1',
      title: '在线晴天',
      artist: '测试歌手',
      album: '云端专辑',
      duration: Duration(minutes: 4),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.custom,
    );
    final searchService = FakeOnlineSearchService(const [result]);
    final player = FakePlayerService();
    final downloader = _FakeCacheDownloader();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: searchService,
      playerService: player,
      downloadService: downloader,
    );
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('搜索').first);
    await tester.pumpAndSettle();
    expect(find.text('在线搜索'), findsOneWidget);

    await viewModel.searchOnline('晴天');
    await tester.pumpAndSettle();
    expect(find.text('在线晴天'), findsOneWidget);
    expect(find.text('测试歌手'), findsOneWidget);

    await tester.tap(find.text('在线晴天'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 50));
    expect(viewModel.current?.id, result.id);
    expect(player.playedTrack?.id, result.id);

    await tester.tap(find.text('QQ 音乐'));
    await tester.pump(const Duration(milliseconds: 400));
    expect(searchService.lastChannel, OnlineSearchChannel.qqMusic);
  });

  testWidgets('home shows covers for platform playlists and new tracks', (
    tester,
  ) async {
    const platformPlaylist = PlatformPlaylist(
      id: 'playlist-cover',
      name: '热门歌单',
      artworkUri: 'https://image.example/playlist.jpg',
      url: 'https://music.example/playlist-cover',
      platform: '网易云音乐',
    );
    const newTrack = Track(
      id: 'new-cover',
      title: '封面新曲',
      artist: '封面歌手',
      album: '封面专辑',
      duration: Duration(minutes: 3),
      uri: '',
      artworkUri: 'https://image.example/track.jpg',
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [],
        const [platformPlaylist],
        const [newTrack],
      ),
      playerService: FakePlayerService(),
    );
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();

    expect(find.text('热门歌单'), findsOneWidget);
    expect(find.text('封面新曲'), findsOneWidget);
    expect(find.byType(RemoteArtwork), findsNWidgets(2));
  });

  testWidgets('favorites a library track into a folder from the menu', (
    tester,
  ) async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    await viewModel.load();
    await viewModel.createFolder('我的收藏');
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.text('曲库').first);
    await tester.pumpAndSettle();
    await tester.tap(find.byIcon(Icons.more_horiz).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('收藏到 我的收藏'));
    await tester.pumpAndSettle();

    expect(
      viewModel.folders.single.trackIds,
      contains(testLibraryTracks.first.id),
    );
  });

  testWidgets('favorites a platform playlist from the home view', (
    tester,
  ) async {
    const platformPlaylist = PlatformPlaylist(
      id: 'playlist-42',
      name: '今日热门',
      artworkUri: '',
      url: 'https://music.example/playlist-42',
      platform: '网易云音乐',
    );
    const playlistTrack = Track(
      id: 'apple-7',
      title: '歌单歌曲',
      artist: '歌手',
      album: '专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.m4a',
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [],
        const [platformPlaylist],
        const [],
        const {
          'playlist-42': [playlistTrack],
        },
      ),
      playerService: FakePlayerService(),
    );
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('收藏并载入歌曲'));
    await tester.pumpAndSettle();

    expect(viewModel.playlists.single.name, '今日热门');
    expect(viewModel.playlists.single.externalUrl, platformPlaylist.url);
    expect(viewModel.playlists.single.tracks, const [playlistTrack]);
    expect(find.byTooltip('取消收藏'), findsOneWidget);

    await tester.tap(find.text('歌单').first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('今日热门'));
    await tester.pumpAndSettle();
    expect(find.text('歌单歌曲'), findsOneWidget);

    await tester.tap(find.text('歌单歌曲'));
    await tester.pump(const Duration(milliseconds: 500));
    expect(viewModel.current, playlistTrack);
  });

  testWidgets('shows progress and volume controls while playing', (
    tester,
  ) async {
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
    );
    await viewModel.load();
    await viewModel.playTrack(viewModel.tracks.first);
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();

    expect(find.byType(PlaybackProgress), findsOneWidget);
    await tester.tap(find.byType(PlayerBar));
    await tester.pumpAndSettle();

    expect(find.byType(NowPlayingView), findsOneWidget);
    expect(find.byType(ImageFiltered), findsOneWidget);
    expect(find.byTooltip('关闭聚焦播放'), findsOneWidget);
    expect(find.byType(VolumeControl), findsOneWidget);
    expect(find.byType(PlaybackProgress), findsOneWidget);
    expect(find.byKey(const ValueKey('floatingLyricsToggle')), findsWidgets);
  });

  testWidgets('offers three qualities for an online track download', (
    tester,
  ) async {
    const track = Track(
      id: 'quality-menu',
      title: '音质菜单',
      artist: '测试歌手',
      album: '测试专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/preview.mp3',
      source: TrackSource.wy,
      sourceId: 'quality-menu-42',
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: const [track]),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      downloadService: _FakeCacheDownloader(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(track);
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: viewModel,
        child: const MaterialApp(home: Scaffold(body: DownloadButton())),
      ),
    );

    await tester.tap(find.byTooltip('选择音质并下载'));
    await tester.pumpAndSettle();

    expect(find.text('标准 128k'), findsOneWidget);
    expect(find.text('高品 320k'), findsOneWidget);
    expect(find.text('无损 flac'), findsOneWidget);
  });
}

class _FakeCacheDownloader extends TrackDownloadService {
  @override
  Future<String> downloadToCache(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
    bool Function()? shouldCancel,
  }) async => 'C:/cache/${track.id}.mp3';
}
