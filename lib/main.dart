import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:media_kit/media_kit.dart';
import 'package:provider/provider.dart';

import 'app.dart';
import 'data/repositories/file_music_repository.dart';
import 'data/repositories/memory_source_repository.dart';
import 'data/services/source_storage.dart';
import 'data/services/online_search_service.dart';
import 'data/services/desktop_window_service.dart';
import 'ui/features/player/player_view_model.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  MediaKit.ensureInitialized();
  final windowService = DesktopWindowService();
  await windowService.initialize();
  final builtInSourceScript = await rootBundle.loadString(
    'assets/sources/yuxi_final_source.js',
  );
  runApp(
    ChangeNotifierProvider(
      create: (_) => PlayerViewModel(
        musicRepository: FileMusicRepository(),
        sourceRepository: MemorySourceRepository(
          storage: FileSourceStorage(),
          builtInScript: builtInSourceScript,
          builtInSourceName: '屿溪-终章',
        ),
        onlineSearchService: MultiSourceOnlineSearchService(),
        windowLifecycleService: windowService,
      )..load(),
      child: const WcMusicApp(),
    ),
  );
}
