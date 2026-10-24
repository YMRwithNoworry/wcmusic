import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:forui/forui.dart';
import 'package:golden_toolkit/golden_toolkit.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/domain/models/track.dart';
import 'package:wcmusic/ui/state/player_view_model.dart';

import 'test_support.dart';

void main() {
  var fontsLoaded = false;

  Future<void> render(
    WidgetTester tester,
    Size size,
    String golden, {
    Future<void> Function(WidgetTester tester)? beforeCapture,
  }) async {
    // 固定加载随包分发的 Noto Sans SC：CI 的 Linux/Windows runner 都没有中文字体，
    // 之前读本机 C:\Windows\Fonts\simhei.ttf 会让 CI 的像素对比必然失败。
    // 测试环境的默认字体 Ahem 只画方块，中文字体必须显式注册。
    if (!fontsLoaded) {
      await loadAppFonts();
      final loader = FontLoader('Noto Sans SC')
        ..addFont(rootBundle.load('assets/fonts/NotoSansSC-Bold.otf'));
      await loader.load();
      fontsLoaded = true;
    }
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    tester.platformDispatcher.accessibilityFeaturesTestValue =
        const FakeAccessibilityFeatures(disableAnimations: true);
    addTearDown(
      tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
    );
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(initialTracks: testLibraryTracks),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(
        const [],
        List.generate(
          4,
          (index) => PlatformPlaylist(
            id: 'platform-$index',
            name: ['今日热门', '华语新声', '轻松周末', '流行精选'][index],
            artworkUri: '',
            url: 'https://music.example/$index',
            platform: '网易云音乐',
          ),
        ),
        List.generate(
          4,
          (index) => Track(
            id: 'new-$index',
            title: ['凌晨信号', '潮汐来信', '玻璃晴空', '静默花园'][index],
            artist: ['林间回声', '夏屿', '蓝色房间', '北岸'][index],
            album: '新曲推荐',
            duration: const Duration(minutes: 3),
            uri: 'https://audio.example/$index.m4a',
            releaseDate: DateTime(2026, 7, 20 - index),
          ),
        ),
      ),
      playerService: FakePlayerService(),
      audioEffectSettingsStore: FakeAudioEffectSettingsStore(),
    );
    addTearDown(viewModel.dispose);
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();
    await beforeCapture?.call(tester);
    await tester.pumpAndSettle();
    // 失败时把实际渲染结果写到 test/failures/，CI 上传后可直接对比 Windows 与 Linux 的差异。
    await expectLater(
      find.byType(MaterialApp),
      matchesGoldenFile(golden),
      reason: '快照 $golden',
    );
  }

  testWidgets('desktop home has a stable composition', (tester) async {
    await render(tester, const Size(1440, 900), 'goldens/home_desktop.png');
  });

  testWidgets('mobile home fits a compact viewport', (tester) async {
    await render(tester, const Size(390, 844), 'goldens/home_mobile.png');
  });

  testWidgets('desktop search channels have a stable layout', (
    tester,
  ) async {
    await render(
      tester,
      const Size(1440, 900),
      'goldens/search_desktop.png',
      beforeCapture: (tester) async {
        await tester.tap(find.text('搜索音乐').first);
      },
    );
  });

  testWidgets('desktop focused player has a stable immersive layout', (
    tester,
  ) async {
    await render(
      tester,
      const Size(1440, 900),
      'goldens/now_playing_desktop.png',
      beforeCapture: (tester) async {
        final context = tester.element(find.byType(MaterialApp));
        final viewModel = context.read<PlayerViewModel>();
        await viewModel.playTrack(viewModel.tracks.first);
        await tester.pump();
        await tester.tap(find.byIcon(FLucideIcons.panelBottomOpen));
      },
    );
  });
}
