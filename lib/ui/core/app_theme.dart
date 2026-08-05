import 'package:flutter/material.dart';

abstract final class AppColors {
  static const paper = Color(0xFFF2EFE7);
  static const paperDeep = Color(0xFFE6E0D4);
  static const ink = Color(0xFF20231F);
  static const moss = Color(0xFF68785A);
  static const mossDark = Color(0xFF3E4C36);
  static const clay = Color(0xFFC7654F);
  static const mist = Color(0xFF8CA9AA);
  static const night = Color(0xFF181B18);
  static const nightSurface = Color(0xFF252A25);
}

abstract final class AppTheme {
  static ThemeData get light => _theme(
    brightness: Brightness.light,
    background: AppColors.paper,
    surface: const Color(0xFFF9F7F1),
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
      primary: brightness == Brightness.light
          ? AppColors.mossDark
          : const Color(0xFFA7C58F),
      secondary: AppColors.clay,
    );
    const radius = BorderRadius.all(Radius.circular(8));
    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: background,
      fontFamily: 'Segoe UI',
      splashFactory: InkSparkle.splashFactory,
      textTheme: TextTheme(
        displayLarge: TextStyle(
          fontSize: 52,
          height: 1.02,
          fontWeight: FontWeight.w600,
          color: ink,
        ),
        displayMedium: TextStyle(
          fontSize: 38,
          height: 1.08,
          fontWeight: FontWeight.w600,
          color: ink,
        ),
        headlineLarge: TextStyle(
          fontSize: 28,
          height: 1.15,
          fontWeight: FontWeight.w600,
          color: ink,
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
        shape: const RoundedRectangleBorder(borderRadius: radius),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: surface,
        border: const OutlineInputBorder(
          borderRadius: radius,
          borderSide: BorderSide.none,
        ),
        enabledBorder: const OutlineInputBorder(
          borderRadius: radius,
          borderSide: BorderSide.none,
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: radius,
          borderSide: BorderSide(color: scheme.primary, width: 1.5),
        ),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: 16,
          vertical: 14,
        ),
      ),
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: Colors.transparent,
        indicatorColor: scheme.primary.withValues(alpha: .13),
        selectedIconTheme: IconThemeData(color: scheme.primary),
        selectedLabelTextStyle: TextStyle(
          color: scheme.primary,
          fontWeight: FontWeight.w600,
        ),
      ),
      navigationBarTheme: NavigationBarThemeData(
        elevation: 0,
        backgroundColor: surface,
        indicatorColor: scheme.primary.withValues(alpha: .13),
      ),
      sliderTheme: SliderThemeData(
        activeTrackColor: scheme.primary,
        inactiveTrackColor: scheme.primary.withValues(alpha: .18),
        thumbColor: scheme.primary,
        trackHeight: 3,
        thumbShape: const RoundSliderThumbShape(enabledThumbRadius: 6),
      ),
      tooltipTheme: const TooltipThemeData(
        waitDuration: Duration(milliseconds: 450),
      ),
    );
  }
}
