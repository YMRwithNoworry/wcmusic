import 'dart:io';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/domain/models/track.dart';
import 'package:wcmusic/ui/features/player/player_view_model.dart';
import 'package:wcmusic/ui/features/player/player_bar.dart';

import 'test_support.dart';

void main() {
  var fontLoaded = false;

  Future<void> render(
    WidgetTester tester,
    Size size,
    String golden, {
    Future<void> Function(WidgetTester tester)? beforeCapture,
    bool disableAnimations = false,
  }) async {
    if (!fontLoaded) {
      final fontFile = File(r'C:\Windows\Fonts\simhei.ttf');
      if (fontFile.existsSync()) {
        final bytes = await tester.runAsync(fontFile.readAsBytes);
        if (bytes == null) throw StateError('无法加载 golden 测试字体');
        final loader = FontLoader('Segoe UI')
          ..addFont(Future.value(ByteData.sublistView(bytes)));
        await loader.load();
      }
      fontLoaded = true;
    }
    tester.view.physicalSize = size;
    tester.view.devicePixelRatio = 1;
    if (disableAnimations) {
      tester.platformDispatcher.accessibilityFeaturesTestValue =
          const FakeAccessibilityFeatures(disableAnimations: true);
      addTearDown(
        tester.platformDispatcher.clearAccessibilityFeaturesTestValue,
      );
    }
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
            platform: 'Apple Music',
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
    );
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
    await tester.pumpAndSettle();
    await beforeCapture?.call(tester);
    await tester.pumpAndSettle();
    await expectLater(find.byType(MaterialApp), matchesGoldenFile(golden));
  }

  testWidgets(
    'desktop home has a stable three-column composition',
    (tester) async {
      await render(tester, const Size(1440, 900), 'goldens/home_desktop.png');
    },
    skip: !Platform.isWindows,
  );

  testWidgets('mobile home fits a compact Android viewport', (tester) async {
    await render(tester, const Size(390, 844), 'goldens/home_mobile.png');
  }, skip: !Platform.isWindows);

  testWidgets('desktop search channels have a stable tab layout', (
    tester,
  ) async {
    await render(
      tester,
      const Size(1440, 900),
      'goldens/search_desktop.png',
      beforeCapture: (tester) async {
        await tester.tap(find.text('搜索').first);
      },
    );
  }, skip: !Platform.isWindows);

  testWidgets(
    'desktop focused player has a stable immersive layout',
    (tester) async {
      await render(
        tester,
        const Size(1440, 900),
        'goldens/now_playing_desktop.png',
        disableAnimations: true,
        beforeCapture: (tester) async {
          final context = tester.element(find.byType(MaterialApp));
          final viewModel = context.read<PlayerViewModel>();
          await viewModel.playTrack(viewModel.tracks.first);
          await tester.pump();
          await tester.tap(find.byType(PlayerBar));
          await tester.pumpAndSettle();
        },
      );
    },
    skip: !Platform.isWindows,
  );
}
