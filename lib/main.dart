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
  final builtInSources = <String, String>{};
  for (final asset in [
    'yuxi_final_source.js',
    'HelloWorld音源(中秋快乐).js',
    'HYWmusic_公益版V1.2.0.js',
    '安妮独家音源 v5.js',
    '独家音源-v7.js',
    '回避聚合V0.0.1.js',
    '裤佬SVIP音源(二改整合版) v3.0.0.js',
    '墨澜音乐源v2.3.5.js',
    '星海音乐源 v2.3.17.js',
  ]) {
    final script = await rootBundle.loadString('assets/sources/$asset');
    final name = asset.replaceFirst(RegExp(r'\.js$'), '');
    builtInSources[name] = script;
  }
  runApp(
    ChangeNotifierProvider(
      create: (_) => PlayerViewModel(
        musicRepository: FileMusicRepository(),
        sourceRepository: MemorySourceRepository(
          storage: FileSourceStorage(),
          builtInScripts: builtInSources,
        ),
        onlineSearchService: MultiSourceOnlineSearchService(),
        windowLifecycleService: windowService,
      )..load(),
      child: const WcMusicApp(),
    ),
  );
}
