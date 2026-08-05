import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/ui/features/player/player_view_model.dart';
import 'package:wcmusic/domain/models/track.dart';

import 'test_support.dart';

void main() {
  testWidgets('renders the music workspace', (tester) async {
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
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
    final viewModel = PlayerViewModel(
      musicRepository: MemoryMusicRepository(),
      sourceRepository: MemorySourceRepository(),
      onlineSearchService: FakeOnlineSearchService(const [result]),
      playerService: FakePlayerService(),
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
  });
}
