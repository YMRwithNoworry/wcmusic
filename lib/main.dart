import 'package:flutter/material.dart';
import 'package:media_kit/media_kit.dart';
import 'package:provider/provider.dart';

import 'app.dart';
import 'data/repositories/memory_music_repository.dart';
import 'data/repositories/memory_source_repository.dart';
import 'data/services/source_storage.dart';
import 'ui/features/player/player_view_model.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  MediaKit.ensureInitialized();
  runApp(
    ChangeNotifierProvider(
      create: (_) => PlayerViewModel(
        musicRepository: MemoryMusicRepository(),
        sourceRepository: MemorySourceRepository(storage: FileSourceStorage()),
      )..load(),
      child: const WcMusicApp(),
    ),
  );
}
