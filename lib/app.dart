import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import 'ui/core/app_theme.dart';
import 'ui/core/app_theme_controller.dart';
import 'ui/features/shell/music_shell.dart';

class WcMusicApp extends StatefulWidget {
  const WcMusicApp({super.key});

  @override
  State<WcMusicApp> createState() => _WcMusicAppState();
}

class _WcMusicAppState extends State<WcMusicApp> {
  late final AppThemeController _themeController;

  @override
  void initState() {
    super.initState();
    _themeController = AppThemeController()..load();
  }

  @override
  void dispose() {
    _themeController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ChangeNotifierProvider.value(
      value: _themeController,
      child: AnimatedBuilder(
        animation: _themeController,
        builder: (context, _) => MaterialApp(
          title: 'WCMusic',
          debugShowCheckedModeBanner: false,
          theme: AppTheme.light,
          darkTheme: AppTheme.dark,
          themeMode: _themeController.mode,
          home: const MusicShell(),
        ),
      ),
    );
  }
}
