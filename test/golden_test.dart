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

import 'test_support.dart';

void main() {
  var fontLoaded = false;

  Future<void> render(WidgetTester tester, Size size, String golden) async {
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
    addTearDown(tester.view.resetPhysicalSize);
    addTearDown(tester.view.resetDevicePixelRatio);

    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
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
      ),
      playerService: FakePlayerService(),
    );
    await viewModel.load();
    await tester.pumpWidget(
      ChangeNotifierProvider.value(value: viewModel, child: const WcMusicApp()),
    );
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
}
