import 'package:flutter/material.dart';
import 'package:forui/forui.dart';
import 'package:provider/provider.dart';

import 'ui/core/app_theme.dart';
import 'ui/core/app_theme_controller.dart';
import 'ui/desktop/shell/desktop_shell.dart';

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
          theme: AppTheme.material(Brightness.light),
          darkTheme: AppTheme.material(Brightness.dark),
          themeMode: _themeController.mode,
          localizationsDelegates: FLocalizations.localizationsDelegates,
          supportedLocales: FLocalizations.supportedLocales,
          home: Builder(
            builder: (context) {
              final brightness = Theme.of(context).brightness;
              final reduceMotion = _themeController.reduceMotion;
              return FTheme(
                data: brightness == Brightness.dark
                    ? AppTheme.dark
                    : AppTheme.light,
                accessibility: reduceMotion
                    ? const FAccessibility(
                        accessibleNavigation: false,
                        motion: FAccessibilityMotion.disabled,
                        focusHighlight: true,
                      )
                    : null,
                child: const DesktopShell(),
              );
            },
          ),
        ),
      ),
    );
  }
}
