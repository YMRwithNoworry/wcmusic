import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:forui/forui.dart';

/// WCMusic 的视觉令牌。
///
/// 桌面端整体由 Forui 组件构建，这里的 [AppPalette] 同时供 Forui 主题和少量
/// 自绘组件（封面、歌词、标题栏）使用，避免两套颜色互相漂移。
abstract final class AppPalette {
  // 夜间：近黑底 + 暖金强调色，长时间听歌不刺眼。
  static const nightBackground = Color(0xFF0A0A0B);
  static const nightSidebar = Color(0xFF0E0E10);
  static const nightCard = Color(0xFF151517);
  static const nightElevated = Color(0xFF1C1C1F);
  static const nightForeground = Color(0xFFF4F1EA);
  static const nightMuted = Color(0xFF9C978D);
  static const nightBorder = Color(0x1FFFFFFF);
  static const amber = Color(0xFFE8B667);
  static const amberDeep = Color(0xFFC98A34);
  static const amberSoft = Color(0xFFF3D49B);

  // 日间：暖纸色，保留同一套金色强调。
  static const paper = Color(0xFFF7F4ED);
  static const paperSidebar = Color(0xFFF1ECE2);
  static const paperCard = Color(0xFFFFFFFF);
  static const paperElevated = Color(0xFFFBF8F2);
  static const ink = Color(0xFF1B1A17);
  static const inkMuted = Color(0xFF6E6A61);
  static const paperBorder = Color(0x1A000000);
  static const clay = Color(0xFFA9622A);
  static const clayForeground = Color(0xFFFFF8EE);

  /// 封面占位与渐变使用的色板，和金色强调保持同一色温。
  static const artworkPalettes = <List<Color>>[
    [Color(0xFF4A5D4E), Color(0xFFD9B26F), Color(0xFFF2E9D8)],
    [Color(0xFF33565C), Color(0xFFD0795C), Color(0xFFF2C98B)],
    [Color(0xFF5C4E47), Color(0xFFA88C7D), Color(0xFFE8DDD3)],
    [Color(0xFF2C4A52), Color(0xFF8B6F5C), Color(0xFFD4C4B0)],
    [Color(0xFF3A4A3F), Color(0xFFC7A15F), Color(0xFFEDE3D1)],
    [Color(0xFF4B3F55), Color(0xFFB06A63), Color(0xFFE6D3C4)],
  ];
}

abstract final class AppTheme {
  static final FThemeData dark = _build(
    colors: FColors(
      brightness: Brightness.dark,
      systemOverlayStyle: SystemUiOverlayStyle.light,
      barrier: const Color(0x8A000000),
      background: AppPalette.nightBackground,
      foreground: AppPalette.nightForeground,
      primary: AppPalette.amber,
      primaryForeground: const Color(0xFF1A1308),
      secondary: AppPalette.nightElevated,
      secondaryForeground: AppPalette.nightForeground,
      muted: const Color(0xFF1A1A1D),
      mutedForeground: AppPalette.nightMuted,
      destructive: const Color(0xFFFF6B6E),
      destructiveForeground: const Color(0xFF2A0607),
      error: const Color(0xFFFF6B6E),
      errorForeground: const Color(0xFF2A0607),
      card: AppPalette.nightCard,
      border: AppPalette.nightBorder,
    ),
  );

  static final FThemeData light = _build(
    colors: FColors(
      brightness: Brightness.light,
      systemOverlayStyle: SystemUiOverlayStyle.dark,
      barrier: const Color(0x3D000000),
      background: AppPalette.paper,
      foreground: AppPalette.ink,
      primary: AppPalette.clay,
      primaryForeground: AppPalette.clayForeground,
      secondary: const Color(0xFFEDE7DB),
      secondaryForeground: AppPalette.ink,
      muted: const Color(0xFFF1ECE3),
      mutedForeground: AppPalette.inkMuted,
      destructive: const Color(0xFFC33A3D),
      destructiveForeground: const Color(0xFFFFF8F7),
      error: const Color(0xFFC33A3D),
      errorForeground: const Color(0xFFFFF8F7),
      card: AppPalette.paperCard,
      border: AppPalette.paperBorder,
    ),
  );

  static FThemeData _build({required FColors colors}) {
    final typeface = FTypeface(
      fontFamilyFallback: const [
        'Microsoft YaHei UI',
        'Microsoft YaHei',
        'Segoe UI',
        'PingFang SC',
        'Noto Sans CJK SC',
        // 随包分发，保证 golden 测试与无中文字体的 CI runner 也有一致的字形
        'Noto Sans SC',
      ],
    );
    final base = FTypography.inherit(colors: colors, touch: false);
    return FThemeData(
      colors: colors,
      touch: false,
      typography: base.copyWith(display: typeface, body: typeface),
    );
  }

  /// Material 只作为 [MaterialApp] 的宿主与路由容器，视觉全部由 Forui 负责。
  static ThemeData material(Brightness brightness) {
    final isDark = brightness == Brightness.dark;
    final background = isDark
        ? AppPalette.nightBackground
        : AppPalette.paper;
    final foreground = isDark ? AppPalette.nightForeground : AppPalette.ink;
    final scheme = ColorScheme.fromSeed(
      seedColor: AppPalette.amber,
      brightness: brightness,
      surface: isDark ? AppPalette.nightCard : AppPalette.paperCard,
      primary: isDark ? AppPalette.amber : AppPalette.clay,
      onPrimary: isDark ? const Color(0xFF1A1308) : AppPalette.clayForeground,
    );
    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: background,
      canvasColor: background,
      fontFamily: 'Inter',
      fontFamilyFallback: const [
        'Microsoft YaHei UI',
        'Microsoft YaHei',
        'Segoe UI',
        'Noto Sans SC',
      ],
      splashFactory: NoSplash.splashFactory,
      highlightColor: Colors.transparent,
      hoverColor: Colors.transparent,
      textTheme: ThemeData(brightness: brightness).textTheme.apply(
        bodyColor: foreground,
        displayColor: foreground,
      ),
    );
  }
}
