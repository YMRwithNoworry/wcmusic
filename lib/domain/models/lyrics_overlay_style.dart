class LyricsOverlayStyle {
  const LyricsOverlayStyle({
    this.fontFamily = 'Microsoft YaHei UI',
    this.fontSize = 30,
    this.alignment = 'center',
    this.textColor = 0xFFF5F3EC,
    this.backgroundColor = 0xFF1C1F1B,
    this.opacity = 0.88,
    this.cornerRadius = 18,
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

  factory LyricsOverlayStyle.fromJson(Map<String, dynamic> json) =>
      LyricsOverlayStyle(
        fontFamily: json['fontFamily'] as String? ?? 'Microsoft YaHei UI',
        fontSize: (json['fontSize'] as num?)?.toDouble() ?? 30,
        alignment: json['alignment'] as String? ?? 'center',
        textColor: (json['textColor'] as num?)?.toInt() ?? 0xFFF5F3EC,
        backgroundColor:
            (json['backgroundColor'] as num?)?.toInt() ?? 0xFF1C1F1B,
        opacity: (json['opacity'] as num?)?.toDouble() ?? 0.88,
        cornerRadius: (json['cornerRadius'] as num?)?.toDouble() ?? 18,
        locked: json['locked'] as bool? ?? true,
        positionX: (json['positionX'] as num?)?.toDouble(),
        positionY: (json['positionY'] as num?)?.toDouble(),
      );
}
