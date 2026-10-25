/// 桌面歌词字体变体，取值与原生端约定保持一致。
const lyricsOverlayFontWeightRegular = 'regular';
const lyricsOverlayFontWeightBold = 'bold';
const lyricsOverlayFontWeightLight = 'light';
const lyricsOverlayFontWeights = {
  lyricsOverlayFontWeightRegular,
  lyricsOverlayFontWeightBold,
  lyricsOverlayFontWeightLight,
};

class LyricsOverlayStyle {
  const LyricsOverlayStyle({
    this.fontFamily = 'Microsoft YaHei UI',
    this.fontWeight = lyricsOverlayFontWeightBold,
    this.fontSize = 24,
    this.alignment = 'right',
    this.textColor = 0xFFF3F3F3,
    this.backgroundColor = 0x00000000,
    this.opacity = 0.92,
    this.cornerRadius = 0,
    this.locked = true,
    this.positionX,
    this.positionY,
  });

  final String fontFamily;

  /// 字体变体：常规 / 粗体 / 细体。只影响当前高亮行，其余行始终使用常规字面。
  final String fontWeight;
  final double fontSize;
  final String alignment;
  final int textColor;
  final int backgroundColor;
  final double opacity;
  final double cornerRadius;
  final bool locked;
  final double? positionX;
  final double? positionY;

  LyricsOverlayStyle copyWith({
    String? fontFamily,
    String? fontWeight,
    double? fontSize,
    String? alignment,
    int? textColor,
    int? backgroundColor,
    double? opacity,
    double? cornerRadius,
    bool? locked,
    double? positionX,
    double? positionY,
  }) => LyricsOverlayStyle(
    fontFamily: fontFamily ?? this.fontFamily,
    fontWeight: fontWeight ?? this.fontWeight,
    fontSize: fontSize ?? this.fontSize,
    alignment: alignment ?? this.alignment,
    textColor: textColor ?? this.textColor,
    backgroundColor: backgroundColor ?? this.backgroundColor,
    opacity: opacity ?? this.opacity,
    cornerRadius: cornerRadius ?? this.cornerRadius,
    locked: locked ?? this.locked,
    positionX: positionX ?? this.positionX,
    positionY: positionY ?? this.positionY,
  );

  Map<String, dynamic> toJson() => {
    'layoutVersion': 3,
    'fontFamily': fontFamily,
    'fontWeight': fontWeight,
    'fontSize': fontSize,
    'alignment': alignment,
    'textColor': textColor,
    'backgroundColor': backgroundColor,
    'opacity': opacity,
    'cornerRadius': cornerRadius,
    'locked': locked,
    'positionX': positionX,
    'positionY': positionY,
  };

  factory LyricsOverlayStyle.fromJson(Map<String, dynamic> json) {
    final fontSize = (json['fontSize'] as num?)?.toDouble();
    final alignment = json['alignment'] as String?;
    final textColor = (json['textColor'] as num?)?.toInt();
    final backgroundColor = (json['backgroundColor'] as num?)?.toInt();
    final opacity = (json['opacity'] as num?)?.toDouble();
    final cornerRadius = (json['cornerRadius'] as num?)?.toDouble();
    final layoutVersion = (json['layoutVersion'] as num?)?.toInt() ?? 0;
    final needsAppleMusicMigration = layoutVersion < 2;
    // 桌面歌词默认透明：v1/v2 时代保存下来的黑色底会被迁移成透明，用户想加背景
    // 可以在设置页重新挑选颜色（v3 起会记住用户的选择）。
    final needsTransparentBackground = layoutVersion < 3;
    return LyricsOverlayStyle(
      fontFamily: json['fontFamily'] as String? ?? 'Microsoft YaHei UI',
      fontWeight: lyricsOverlayFontWeights.contains(json['fontWeight'])
          ? json['fontWeight'] as String
          : lyricsOverlayFontWeightBold,
      fontSize: needsAppleMusicMigration ? 24 : fontSize ?? 24,
      alignment: needsAppleMusicMigration ? 'right' : alignment ?? 'right',
      textColor: needsAppleMusicMigration
          ? 0xFFF3F3F3
          : textColor ?? 0xFFF3F3F3,
      backgroundColor: needsAppleMusicMigration || needsTransparentBackground
          ? 0x00000000
          : backgroundColor ?? 0x00000000,
      opacity: needsAppleMusicMigration ? 0.92 : opacity ?? 0.92,
      cornerRadius: needsAppleMusicMigration ? 0 : cornerRadius ?? 0,
      locked: json['locked'] as bool? ?? true,
      positionX: needsAppleMusicMigration
          ? null
          : (json['positionX'] as num?)?.toDouble(),
      positionY: needsAppleMusicMigration
          ? null
          : (json['positionY'] as num?)?.toDouble(),
    );
  }
}
