class LyricsOverlayStyle {
  const LyricsOverlayStyle({
    this.fontFamily = 'Microsoft YaHei',
    this.fontSize = 24,
    this.alignment = 'center',
    this.textColor = 0xFFF3F3F3,
    this.backgroundColor = 0xFF141416,
    this.opacity = 0.92,
    this.cornerRadius = 8,
    this.locked = false,
    this.alwaysOnTop = true,
    this.showTranslation = true,
    this.zoomActiveLine = true,
    this.windowWidth = 1000,
    this.windowHeight = 140,
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
  final bool alwaysOnTop;
  final bool showTranslation;
  final bool zoomActiveLine;
  final double windowWidth;
  final double windowHeight;
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
    bool? alwaysOnTop,
    bool? showTranslation,
    bool? zoomActiveLine,
    double? windowWidth,
    double? windowHeight,
    bool clearPosition = false,
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
    alwaysOnTop: alwaysOnTop ?? this.alwaysOnTop,
    showTranslation: showTranslation ?? this.showTranslation,
    zoomActiveLine: zoomActiveLine ?? this.zoomActiveLine,
    windowWidth: windowWidth ?? this.windowWidth,
    windowHeight: windowHeight ?? this.windowHeight,
    positionX: clearPosition ? null : positionX ?? this.positionX,
    positionY: clearPosition ? null : positionY ?? this.positionY,
  );

  Map<String, dynamic> toJson() => {
    'layoutVersion': 3,
    'fontFamily': fontFamily,
    'fontSize': fontSize,
    'alignment': alignment,
    'textColor': textColor,
    'backgroundColor': backgroundColor,
    'opacity': opacity,
    'cornerRadius': cornerRadius,
    'locked': locked,
    'alwaysOnTop': alwaysOnTop,
    'showTranslation': showTranslation,
    'zoomActiveLine': zoomActiveLine,
    'windowWidth': windowWidth,
    'windowHeight': windowHeight,
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
    final savedFontFamily = json['fontFamily'] as String?;
    final needsFontMigration =
        layoutVersion < 3 &&
        (savedFontFamily == null ||
            savedFontFamily == 'MiSans' ||
            savedFontFamily == 'Microsoft YaHei UI');
    return LyricsOverlayStyle(
      fontFamily: needsFontMigration
          ? 'Microsoft YaHei'
          : savedFontFamily ?? 'Microsoft YaHei',
      fontSize: needsAppleMusicMigration ? 24 : fontSize ?? 24,
      alignment: needsAppleMusicMigration ? 'center' : alignment ?? 'center',
      textColor: needsAppleMusicMigration
          ? 0xFFF3F3F3
          : textColor ?? 0xFFF3F3F3,
      backgroundColor: needsAppleMusicMigration
          ? 0xFF141416
          : backgroundColor ?? 0xFF141416,
      opacity: needsAppleMusicMigration ? 0.92 : opacity ?? 0.92,
      cornerRadius: needsAppleMusicMigration ? 8 : cornerRadius ?? 8,
      locked: json['locked'] as bool? ?? false,
      alwaysOnTop: json['alwaysOnTop'] as bool? ?? true,
      showTranslation: json['showTranslation'] as bool? ?? true,
      zoomActiveLine: json['zoomActiveLine'] as bool? ?? true,
      windowWidth: (json['windowWidth'] as num?)?.toDouble() ?? 1000,
      windowHeight: (json['windowHeight'] as num?)?.toDouble() ?? 140,
      positionX: needsAppleMusicMigration
          ? null
          : (json['positionX'] as num?)?.toDouble(),
      positionY: needsAppleMusicMigration
          ? null
          : (json['positionY'] as num?)?.toDouble(),
    );
  }
}
