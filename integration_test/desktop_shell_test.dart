import 'dart:io';

import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forui/forui.dart';
import 'package:integration_test/integration_test.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/data/services/track_download_service.dart';
import 'package:wcmusic/domain/models/track.dart';
import 'package:wcmusic/ui/core/frame_probe.dart';
import 'package:wcmusic/ui/desktop/player/now_playing.dart';
import 'package:wcmusic/ui/desktop/widgets/track_list.dart';
import 'package:wcmusic/ui/state/player_view_model.dart';

import '../test/test_support.dart';

/// 播放网络曲目时只落到假缓存，避免真实下载。
class _CacheOnlyDownloader extends TrackDownloadService {
  @override
  Future<String> downloadToCache(
    Track track, {
    String fallbackExtension = '.mp3',
    required void Function(double progress) onProgress,
    bool Function()? shouldCancel,
  }) async => 'C:/cache/${track.id}.mp3';
}

/// 在真实 Windows 引擎里跑一遍桌面端主流程。
///
/// 只用 WidgetTester 驱动（不注入系统鼠标事件），因此用户正占用鼠标时也能
/// 回归；同时用 [FrameProbe] 采集真实帧耗时，量化“动画是否流畅”。
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('desktop shell navigates every section smoothly', (tester) async {
    tester.view.physicalSize = const Size(1560, 940);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final logPath =
        const String.fromEnvironment('WCMUSIC_FRAME_LOG', defaultValue: '') ==
            ''
        ? '.codex-tmp/frames.json'
        : const String.fromEnvironment('WCMUSIC_FRAME_LOG');
    final probe = FrameProbe.toFile(File(logPath))..start();
    addTearDown(probe.stop);

    const result = Track(
      id: 'perf-online',
      title: '性能测试歌曲',
      artist: '测试歌手',
      album: '真实引擎',
      duration: Duration(minutes: 3, seconds: 20),
      uri: 'https://audio.example/perf.mp3',
      source: TrackSource.custom,
    );
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [result],
        const [
          PlatformPlaylist(
            id: 'perf-playlist',
            name: '真实引擎歌单',
            artworkUri: '',
            url: 'https://music.example/perf',
            platform: '网易云音乐',
          ),
        ],
        const [
          Track(
            id: 'perf-new',
            title: '真实引擎新曲',
            artist: '新曲歌手',
            album: '新曲',
            duration: Duration(minutes: 2),
            uri: '',
          ),
        ],
      ),
      playerService: FakePlayerService(),
      downloadService: _CacheOnlyDownloader(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();

    /// 播放后会有常驻的呼吸 / 跳动动画，pumpAndSettle 永远不会返回，
    /// 所以统一用固定时长的 pump 推进真实帧。
    Future<void> settle([int ms = 360]) async {
      final frames = (ms / 16).ceil();
      for (var i = 0; i < frames; i++) {
        await tester.pump(const Duration(milliseconds: 16));
      }
    }

    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await settle(600);
    expect(find.text('此刻，听见喜欢'), findsOneWidget);

    for (final label in [
      '搜索音乐',
      '排行榜',
      '曲库',
      '我的歌单',
      '音源管理',
      '设置',
      '发现音乐',
    ]) {
      await tester.tap(find.text(label).first);
      await settle();
      await tester.drag(find.byType(Scrollable).first, const Offset(0, -420));
      await settle();
      await tester.drag(find.byType(Scrollable).first, const Offset(0, 320));
      await settle();
    }

    await tester.tap(find.text('搜索音乐').first);
    await settle();
    await viewModel.searchOnline('性能');
    await settle();
    expect(find.text('性能测试歌曲'), findsOneWidget);

    final rowTitle = find.descendant(
      of: find.byType(TrackRow),
      matching: find.text('性能测试歌曲'),
    );
    await tester.tap(rowTitle.first);
    await settle(500);
    expect(viewModel.current?.id, 'perf-online');

    await tester.tap(find.byIcon(FLucideIcons.panelBottomOpen));
    await settle(600);
    expect(find.byType(NowPlayingPanel), findsOneWidget);
    await tester.tap(find.text('返回'));
    await settle(600);
    expect(find.byType(NowPlayingPanel), findsNothing);

    await tester.tap(rowTitle.first, buttons: kSecondaryMouseButton);
    await settle(500);
    expect(find.textContaining('下载'), findsWidgets);
    await tester.tap(find.byIcon(FLucideIcons.x).last);
    await settle(400);

    await settle(1200);
    await tester.runAsync(probe.flush);
    final summary = probe.lastSummary;
    debugPrint('[frame-probe] $summary');
    expect(summary, isNotNull);
    expect(summary!['frames'] as int, greaterThan(120));
  }, timeout: const Timeout(Duration(minutes: 5)));
}
