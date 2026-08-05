import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:wcmusic/app.dart';
import 'package:wcmusic/data/repositories/memory_music_repository.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/ui/features/player/player_view_model.dart';

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
}
