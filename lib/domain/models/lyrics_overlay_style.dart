class LyricsOverlayStyle {
  const LyricsOverlayStyle({
    this.fontFamily = 'Microsoft YaHei UI',
    this.fontSize = 24,
    this.alignment = 'right',
    this.textColor = 0xFFF3F3F3,
    this.backgroundColor = 0xFF141416,
    this.opacity = 0.96,
    this.cornerRadius = 8,
    this.locked = true,
    this.positionX,
    this.positionY,
  });

  final String fontFamily;
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
    'fontFamily': fontFamily,
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
    final usesLegacyDefaults =
        fontSize == 30 &&
        alignment == 'center' &&
        textColor == 0xFFF5F3EC &&
        backgroundColor == 0xFF1C1F1B &&
        opacity == 0.88 &&
        cornerRadius == 18;
    return LyricsOverlayStyle(
      fontFamily: json['fontFamily'] as String? ?? 'Microsoft YaHei UI',
      fontSize: usesLegacyDefaults ? 24 : fontSize ?? 24,
      alignment: usesLegacyDefaults ? 'right' : alignment ?? 'right',
      textColor: usesLegacyDefaults ? 0xFFF3F3F3 : textColor ?? 0xFFF3F3F3,
      backgroundColor: usesLegacyDefaults
          ? 0xFF141416
          : backgroundColor ?? 0xFF141416,
      opacity: usesLegacyDefaults ? 0.96 : opacity ?? 0.96,
      cornerRadius: usesLegacyDefaults ? 8 : cornerRadius ?? 8,
      locked: json['locked'] as bool? ?? true,
      positionX: usesLegacyDefaults
          ? null
          : (json['positionX'] as num?)?.toDouble(),
      positionY: usesLegacyDefaults
          ? null
          : (json['positionY'] as num?)?.toDouble(),
    );
  }
}
