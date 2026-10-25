import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/data/services/online_search_service.dart';
import 'package:wcmusic/data/services/track_download_service.dart';
import 'package:wcmusic/domain/models/track.dart';
import 'package:wcmusic/ui/core/app_theme.dart';
import 'package:wcmusic/ui/core/remote_artwork.dart';
import 'package:wcmusic/ui/desktop/player/controls.dart';
import 'package:wcmusic/ui/desktop/player/lyrics_view.dart';
import 'package:wcmusic/ui/desktop/player/now_playing.dart';
import 'package:wcmusic/ui/desktop/player/player_bar.dart';
import 'package:wcmusic/ui/desktop/widgets/hover.dart';
import 'package:wcmusic/ui/state/player_view_model.dart';

import 'test_support.dart';

/// 每个用例结束时卸载界面并走完 toast 的 5 秒计时，避免遗留定时器。
void appTest(
  String description,
  Future<void> Function(WidgetTester tester) body,
) {
  testWidgets(description, (tester) async {
    await body(tester);
    await tester.pumpWidget(const SizedBox());
    await tester.pump(const Duration(seconds: 6));
  }, timeout: const Timeout(Duration(seconds: 60)));
}

void main() {
  void useDesktopSurface(WidgetTester tester) {
    tester.view.physicalSize = const Size(1400, 900);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);
  }

  Future<void> pumpApp(WidgetTester tester, PlayerViewModel viewModel) async {
    useDesktopSurface(tester);
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();
  }

  appTest('renders the music workspace', (tester) async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await pumpApp(tester, viewModel);

    expect(find.text('此刻，听见喜欢'), findsOneWidget);
    expect(find.text('Seedling'), findsWidgets);
    expect(find.text('发现音乐'), findsOneWidget);
    expect(find.text('搜索音乐'), findsOneWidget);
    expect(find.text('设置'), findsOneWidget);
  });

  appTest('opens online search and displays results', (tester) async {
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
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: searchService,
      playerService: player,
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
      downloadService: _FakeCacheDownloader(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await pumpApp(tester, viewModel);

    await tester.tap(find.text('搜索音乐').first);
    await tester.pumpAndSettle();
    expect(find.text('跨平台寻找歌曲、艺术家与专辑'), findsOneWidget);

    await viewModel.searchOnline('晴天');
    await tester.pumpAndSettle();
    expect(find.text('在线晴天'), findsOneWidget);
    expect(find.text('测试歌手'), findsOneWidget);

    await tester.tap(find.text('在线晴天'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 120));
    expect(viewModel.current?.id, result.id);
    expect(player.playedTrack?.id, result.id);

    await tester.tap(find.text('QQ 音乐'));
    await tester.pump(const Duration(milliseconds: 400));
    expect(searchService.lastChannel, OnlineSearchChannel.qqMusic);
  });

  appTest('favorites an online search result from the context menu', (tester) async {
    const result = Track(
      id: 'online-favorite',
      title: '可收藏在线歌曲',
      artist: '测试歌手',
      album: '云端专辑',
      duration: Duration(minutes: 4),
      uri: 'https://audio.example/preview.m4a',
      source: TrackSource.custom,
    );
    final repository = MemoryMusicRepository();
    final viewModel = PlayerViewModel(
      musicRepository: repository,
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(const [result]),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.createFolder('在线收藏');
    await pumpApp(tester, viewModel);

    await tester.tap(find.text('搜索音乐').first);
    await tester.pumpAndSettle();
    await viewModel.searchOnline('收藏');
    await tester.pumpAndSettle();
    await tester.tap(
      find.text('可收藏在线歌曲'),
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('收藏到 在线收藏'));
    await tester.pumpAndSettle();

    expect(viewModel.folders.single.trackIds, contains(result.id));
    expect(await repository.loadTracks(), contains(result));
  });

  appTest('downloads the selected song from its context menu', (tester) async {
    const result = Track(
      id: 'context-download',
      title: '右键下载歌曲',
      artist: '测试歌手',
      album: '云端专辑',
      duration: Duration(minutes: 4),
      uri: 'https://audio.example/context.mp3',
      source: TrackSource.custom,
    );
    final downloader = _FakeCacheDownloader();
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(const [result]),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
      downloadService: downloader,
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await pumpApp(tester, viewModel);

    await tester.tap(find.text('搜索音乐').first);
    await tester.pumpAndSettle();
    await viewModel.searchOnline('下载');
    await tester.pumpAndSettle();
    await tester.tap(
      find.text('右键下载歌曲'),
      buttons: kSecondaryMouseButton,
    );
    await tester.pumpAndSettle();
    await tester.tap(find.text('下载 · 原始音质'));
    await tester.pumpAndSettle();

    expect(downloader.downloadedTrack, result);
    expect(viewModel.current, isNull);
  });

  appTest('home shows covers for platform playlists and new tracks', (tester) async {
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
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await pumpApp(tester, viewModel);

    expect(find.text('热门歌单'), findsOneWidget);
    expect(find.text('封面新曲'), findsOneWidget);
    expect(find.byType(RemoteArtwork), findsNWidgets(2));
  });

  appTest('opens rankings and displays tracks from the selected platform', (tester) async {
    const ranking = PlatformRanking(
      id: 'rank-1',
      name: '酷我热歌榜',
      channel: OnlineSearchChannel.kuwo,
    );
    const rankingTrack = Track(
      id: 'kw-rank-song',
      title: '榜单测试歌曲',
      artist: '榜单歌手',
      album: '榜单专辑',
      duration: Duration(minutes: 3),
      uri: 'https://audio.example/rank.mp3',
      source: TrackSource.kw,
      sourceId: 'rank-song',
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [],
        const [],
        const [],
        const {},
        null,
        const {
          OnlineSearchChannel.kuwo: [ranking],
        },
        const {
          'rank-1': [rankingTrack],
        },
      ),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
      downloadService: _FakeCacheDownloader(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await pumpApp(tester, viewModel);

    await tester.tap(find.text('排行榜').first);
    await tester.pumpAndSettle();

    expect(find.text('酷我热歌榜'), findsWidgets);
    expect(find.text('榜单测试歌曲'), findsOneWidget);
  });

  appTest('favorites a library track into a folder from the row menu', (tester) async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.createFolder('我的收藏');
    await pumpApp(tester, viewModel);

    await tester.tap(find.text('曲库').first);
    await tester.pumpAndSettle();
    await tester.tap(find.byIcon(FLucideIcons.moreHorizontal).first);
    await tester.pumpAndSettle();
    await tester.tap(find.text('收藏到 我的收藏'));
    await tester.pumpAndSettle();

    expect(
      viewModel.folders.single.trackIds,
      contains(testLibraryTracks.first.id),
    );
  });

  appTest('opens a platform playlist and favorites it from home', (tester) async {
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
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await pumpApp(tester, viewModel);

    await tester.tap(find.text('今日热门').first);
    await tester.pumpAndSettle();
    expect(find.text('歌单歌曲'), findsOneWidget);
    await tester.tap(find.byIcon(FLucideIcons.x).last);
    await tester.pumpAndSettle();

    await tester.tap(find.byIcon(FLucideIcons.plus).first);
    await tester.pumpAndSettle();

    expect(viewModel.playlists.single.name, '今日热门');
    expect(viewModel.playlists.single.externalUrl, platformPlaylist.url);
    expect(viewModel.playlists.single.tracks, const [playlistTrack]);

    await tester.tap(find.text('我的歌单').first);
    await tester.pumpAndSettle();
    expect(find.text('今日热门'), findsOneWidget);
  });

  appTest('shows progress, lyrics and volume controls while playing', (tester) async {
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(tester.platformDispatcher.clearAccessibilityFeaturesTestValue);
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(viewModel.tracks.first);
    await pumpApp(tester, viewModel);

    expect(find.byType(PlayerBar), findsOneWidget);
    expect(find.byType(SeekBar), findsOneWidget);
    expect(find.byType(TransportControls), findsOneWidget);
    // 播放顺序按钮在播放条右侧、下载按钮旁边，不在上一首/播放/下一首中间。
    expect(find.byKey(const ValueKey('playbackModeButton')), findsOneWidget);
    // 桌面歌词开关也在下载按钮旁边，点击即可开启。
    final floatingLyrics = find.byKey(const ValueKey('floatingLyricsToggle'));
    expect(floatingLyrics, findsOneWidget);
    // 测试环境没有原生通道，这里只验证开关已接到真实的切换方法。
    await tester.runAsync(() async {
      await viewModel.setFloatingLyrics(true);
      await viewModel.setFloatingLyrics(false);
    });
    expect(viewModel.floatingLyricsEnabled, isFalse);
    expect(find.byIcon(FLucideIcons.panelBottomOpen), findsNothing);

    // 点击播放条左侧歌曲信息打开歌词页。
    await tester.tap(
      find
          .descendant(
            of: find.byType(PlayerBar),
            matching: find.byType(HoverLift),
          )
          .first,
    );
    await tester.pumpAndSettle();

    expect(find.byType(NowPlayingPanel), findsOneWidget);
    expect(find.byType(LyricsPane), findsOneWidget);
    // 详情页不再重复播放条上的进度与控制，只剩歌词。
    expect(find.byType(SeekBar), findsOneWidget);
    expect(find.text('聚焦播放'), findsNothing);
    expect(floatingLyrics, findsOneWidget);

    await tester.tap(find.text('返回').last);
    await tester.pumpAndSettle();
    expect(find.byType(NowPlayingPanel), findsNothing);
  });

  appTest('offers three qualities for an online track download', (tester) async {
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
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
      downloadService: _FakeCacheDownloader(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(track);
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: viewModel,
        child: MaterialApp(
          home: FTheme(
            data: AppTheme.dark,
            child: const Scaffold(body: Center(child: DownloadButton())),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 100));

    await tester.tap(find.byIcon(FLucideIcons.download));
    await tester.pumpAndSettle();

    expect(find.text('下载 · 标准 128k'), findsOneWidget);
    expect(find.text('下载 · 高品 320k'), findsOneWidget);
    expect(find.text('下载 · 无损 flac'), findsOneWidget);
  });

  appTest('shows an original-quality option for a local track download', (tester) async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await viewModel.playTrack(viewModel.tracks.first);
    await tester.pumpWidget(
      ChangeNotifierProvider.value(
        value: viewModel,
        child: MaterialApp(
          home: FTheme(
            data: AppTheme.dark,
            child: const Scaffold(body: Center(child: DownloadButton())),
          ),
        ),
      ),
    );
    await tester.pump(const Duration(milliseconds: 100));

    await tester.tap(find.byIcon(FLucideIcons.download));
    await tester.pumpAndSettle();

    expect(find.text('下载 · 原始音质'), findsOneWidget);
  });
}

class _FakeCacheDownloader extends TrackDownloadService {
  Track? downloadedTrack;

  @override
  Future<String> download(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
  }) async {
    downloadedTrack = track;
    onProgress(1);
    return 'D:/music/${track.id}$fallbackExtension';
  }

  @override
  Future<String> downloadToCache(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
    bool Function()? shouldCancel,
  }) async => 'C:/cache/${track.id}.mp3';
}
