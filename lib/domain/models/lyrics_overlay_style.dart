class LyricsOverlayStyle {
  const LyricsOverlayStyle({
    this.fontFamily = 'Microsoft YaHei UI',
    this.fontSize = 24,
    this.alignment = 'right',
    this.textColor = 0xFFF3F3F3,
    this.backgroundColor = 0xFF141416,
    this.opacity = 0.92,
    this.cornerRadius = 0,
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
    'layoutVersion': 2,
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
    final needsAppleMusicMigration =
        ((json['layoutVersion'] as num?)?.toInt() ?? 0) < 2;
    return LyricsOverlayStyle(
      fontFamily: json['fontFamily'] as String? ?? 'Microsoft YaHei UI',
      fontSize: needsAppleMusicMigration ? 24 : fontSize ?? 24,
      alignment: needsAppleMusicMigration ? 'right' : alignment ?? 'right',
      textColor: needsAppleMusicMigration
          ? 0xFFF3F3F3
          : textColor ?? 0xFFF3F3F3,
      backgroundColor: needsAppleMusicMigration
          ? 0xFF141416
          : backgroundColor ?? 0xFF141416,
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
