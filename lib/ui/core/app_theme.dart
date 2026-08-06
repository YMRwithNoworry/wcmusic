import 'package:flutter/material.dart';

abstract final class AppColors {
  static const paper = Color(0xFFF2EFE7);
  static const paperDeep = Color(0xFFE6E0D4);
  static const paperLight = Color(0xFFF8F6F2);
  static const ink = Color(0xFF20231F);
  static const moss = Color(0xFF68785A);
  static const mossDark = Color(0xFF3E4C36);
  static const mossLight = Color(0xFF8FA682);
  static const clay = Color(0xFFC7654F);
  static const clayLight = Color(0xFFD48B78);
  static const mist = Color(0xFF8CA9AA);
  static const night = Color(0xFF181B18);
  static const nightSurface = Color(0xFF252A25);
  static const nightElevated = Color(0xFF2D332D);
}

abstract final class AppTheme {
  static ThemeData get light => _theme(
    brightness: Brightness.light,
    background: AppColors.paper,
    surface: AppColors.paperLight,
    ink: AppColors.ink,
  );

  static ThemeData get dark => _theme(
    brightness: Brightness.dark,
    background: AppColors.night,
    surface: AppColors.nightSurface,
    ink: const Color(0xFFF2EFE7),
  );

  static ThemeData _theme({
    required Brightness brightness,
    required Color background,
    required Color surface,
    required Color ink,
  }) {
    final scheme = ColorScheme.fromSeed(
      seedColor: AppColors.moss,
      brightness: brightness,
      surface: surface,
      surfaceContainerHighest: brightness == Brightness.light
          ? AppColors.paperDeep
          : AppColors.nightElevated,
      primary: brightness == Brightness.light
          ? AppColors.mossDark
          : const Color(0xFFA7C58F),
      secondary: brightness == Brightness.light
          ? AppColors.clay
          : AppColors.clayLight,
      tertiary: AppColors.mist,
    );
    const radius = BorderRadius.all(Radius.circular(12));
    const smallRadius = BorderRadius.all(Radius.circular(8));
    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: background,
      fontFamily: 'Segoe UI',
      splashFactory: InkRipple.splashFactory,
      pageTransitionsTheme: const PageTransitionsTheme(
        builders: {
          TargetPlatform.android: ZoomPageTransitionsBuilder(),
          TargetPlatform.iOS: ZoomPageTransitionsBuilder(),
          TargetPlatform.windows: FadeUpwardsPageTransitionsBuilder(),
        },
      ),
      textTheme: TextTheme(
        displayLarge: TextStyle(
          fontSize: 52,
          height: 1.02,
          fontWeight: FontWeight.w600,
          color: ink,
          letterSpacing: -0.5,
        ),
        displayMedium: TextStyle(
          fontSize: 38,
          height: 1.08,
          fontWeight: FontWeight.w600,
          color: ink,
          letterSpacing: -0.3,
        ),
        headlineLarge: TextStyle(
          fontSize: 28,
          height: 1.15,
          fontWeight: FontWeight.w600,
          color: ink,
          letterSpacing: -0.2,
        ),
        headlineMedium: TextStyle(
          fontSize: 22,
          height: 1.2,
          fontWeight: FontWeight.w600,
          color: ink,
        ),
        titleLarge: TextStyle(
          fontSize: 18,
          height: 1.25,
          fontWeight: FontWeight.w600,
          color: ink,
        ),
        titleMedium: TextStyle(
          fontSize: 16,
          height: 1.3,
          fontWeight: FontWeight.w500,
          color: ink,
        ),
        bodyLarge: TextStyle(fontSize: 16, height: 1.45, color: ink),
        bodyMedium: TextStyle(
          fontSize: 14,
          height: 1.4,
          color: ink.withValues(alpha: .82),
        ),
      ),
      cardTheme: CardThemeData(
        elevation: 0,
        margin: EdgeInsets.zero,
        color: surface,
        shape: const RoundedRectangleBorder(borderRadius: smallRadius),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          shape: const RoundedRectangleBorder(borderRadius: smallRadius),
          padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 14),
        ),
      ),
      iconButtonTheme: IconButtonThemeData(
        style: IconButton.styleFrom(
          shape: const RoundedRectangleBorder(borderRadius: smallRadius),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: surface,
        border: OutlineInputBorder(
          borderRadius: radius,
          borderSide: BorderSide.none,
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: radius,
          borderSide: BorderSide.none,
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: radius,
          borderSide: BorderSide(color: scheme.primary, width: 2),
        ),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 16,
          vertical: 14,
        ),
      ),
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: Colors.transparent,
        indicatorColor: scheme.primary.withValues(alpha: .15),
        indicatorShape: const RoundedRectangleBorder(borderRadius: smallRadius),
        selectedIconTheme: IconThemeData(color: scheme.primary),
        selectedLabelTextStyle: TextStyle(
          color: scheme.primary,
          fontWeight: FontWeight.w600,
        ),
      ),
      navigationBarTheme: NavigationBarThemeData(
        elevation: 0,
        backgroundColor: surface,
        indicatorColor: scheme.primary.withValues(alpha: .15),
        indicatorShape: const RoundedRectangleBorder(borderRadius: smallRadius),
      ),
      sliderTheme: SliderThemeData(
        activeTrackColor: scheme.primary,
        inactiveTrackColor: scheme.primary.withValues(alpha: .20),
        thumbColor: scheme.primary,
        trackHeight: 4,
        thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 7),
        overlayShape: const RoundSliderOverlayShape(overlayRadius: 18),
      ),
      tooltipTheme: const TooltipThemeData(
        waitDuration: Duration(milliseconds: 400),
      ),
      dividerTheme: DividerThemeData(
        color: ink.withValues(alpha: .08),
        thickness: 1,
        space: 1,
      ),
    );
  }
}
