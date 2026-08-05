import 'package:flutter/material.dart';

import 'ui/core/app_theme.dart';
import 'ui/features/shell/music_shell.dart';

class WcMusicApp extends StatelessWidget {
  const WcMusicApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'WCMusic',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light,
      darkTheme: AppTheme.dark,
      themeMode: ThemeMode.system,
      home: const MusicShell(),
    );
  }
}
