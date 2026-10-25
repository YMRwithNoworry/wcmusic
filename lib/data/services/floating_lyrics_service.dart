import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

import '../../domain/models/lyrics_overlay_style.dart';

abstract interface class FloatingLyricsService {
  bool get isSupported;
  Future<bool> setEnabled(bool enabled);
  Future<void> update({
    required String title,
    required String currentLine,
    required String nextLine,
    List<String> lines = const [],
    int currentIndex = -1,
    double lineProgress = 0,
    String? currentTranslation,
    String? nextTranslation,
  });
  Future<void> setStyle(LyricsOverlayStyle style);
  Future<LyricsOverlayStyle> loadStyle();
  Future<void> saveStyle(LyricsOverlayStyle style);

  /// 系统已安装字体（字族名，已排序）；不支持的平台返回空列表。
  Future<List<String>> listSystemFonts();

  /// 平台默认的界面字体，用作字体列表的「系统默认」项。
  Future<String?> defaultSystemFont();
  Future<void> dispose();
}

class PlatformFloatingLyricsService implements FloatingLyricsService {
  PlatformFloatingLyricsService({
    Future<Directory> Function()? directoryProvider,
  }) : _directoryProvider = directoryProvider ?? getApplicationSupportDirectory;

  static const _channel = MethodChannel('wcmusic/lyrics_overlay');
  // 读取系统字体走歌词通道，避免让歌词窗口常驻就能列出字体
  static const _systemChannel = MethodChannel('wcmusic/lyrics_overlay');

  final Future<Directory> Function() _directoryProvider;
  bool _enabled = false;
  bool _handlerRegistered = false;
  LyricsOverlayStyle? _style;
  File? _settingsFile;
  List<String>? _lastLyricsLines;
  List<String>? _systemFonts;
  String? _defaultFont;

  void _ensureHandler() {
    if (_handlerRegistered) return;
    _handlerRegistered = true;
    _channel.setMethodCallHandler(_handleNativeCall);
  }

  @override
  bool get isSupported => Platform.isWindows || Platform.isAndroid;

  @override
  Future<bool> setEnabled(bool enabled) async {
    if (!isSupported) return false;
    _ensureHandler();
    final result = await _channel.invokeMethod<bool>('setEnabled', {
      'enabled': enabled,
    });
    final succeeded = result ?? false;
    _enabled = enabled && succeeded;
    if (!enabled) _lastLyricsLines = null;
    return enabled ? _enabled : succeeded;
  }

  @override
  Future<void> update({
    required String title,
    required String currentLine,
    required String nextLine,
    List<String> lines = const [],
    int currentIndex = -1,
    double lineProgress = 0,
    String? currentTranslation,
    String? nextTranslation,
  }) async {
    if (!_enabled) return;
    _ensureHandler();
    final updateLines =
        Platform.isWindows && !identical(lines, _lastLyricsLines);
    if (updateLines) _lastLyricsLines = lines;
    final arguments = <String, Object?>{
      'title': title,
      'currentLine': currentLine,
      'nextLine': nextLine,
      'currentTranslation': currentTranslation,
      'nextTranslation': nextTranslation,
      'currentIndex': currentIndex,
      'lineProgress': lineProgress.clamp(0.0, 1.0),
    };
    if (updateLines) arguments['lines'] = lines;
    await _channel.invokeMethod<void>('update', arguments);
  }

  @override
  Future<void> setStyle(LyricsOverlayStyle style) async {
    _style = style;
    if (!Platform.isWindows) return;
    _ensureHandler();
    await _channel.invokeMethod<void>('setStyle', _styleArguments(style));
  }

  @override
  Future<LyricsOverlayStyle> loadStyle() async {
    if (_style != null) return _style!;
    final file = await _settingsFileFor();
    if (!await file.exists()) {
      _style = const LyricsOverlayStyle();
      return _style!;
    }
    try {
      final decoded = jsonDecode(await file.readAsString());
      _style = LyricsOverlayStyle.fromJson(
        decoded is Map<String, dynamic> ? decoded : const {},
      );
    } on Object {
      _style = const LyricsOverlayStyle();
    }
    return _style!;
  }

  @override
  Future<void> saveStyle(LyricsOverlayStyle style) async {
    _style = style;
    final file = await _settingsFileFor();
    await file.parent.create(recursive: true);
    await file.writeAsString(jsonEncode(style.toJson()), flush: true);
  }

  Future<void> _handleNativeCall(MethodCall call) async {
    if (call.method != 'positionChanged') return;
    final arguments = call.arguments;
    if (arguments is! Map) return;
    final x = (arguments['x'] as num?)?.toDouble();
    final y = (arguments['y'] as num?)?.toDouble();
    if (x == null || y == null) return;
    final current = await loadStyle();
    _style = current.copyWith(positionX: x, positionY: y);
    await saveStyle(_style!);
  }

  Future<File> _settingsFileFor() async {
    if (_settingsFile != null) return _settingsFile!;
    final directory = await _directoryProvider();
    _settingsFile = File(
      '${directory.path}${Platform.pathSeparator}lyrics_style.json',
    );
    return _settingsFile!;
  }

  Map<String, dynamic> _styleArguments(LyricsOverlayStyle style) => {
    'fontFamily': style.fontFamily,
    'fontWeight': style.fontWeight,
    'fontSize': style.fontSize,
    'align': style.alignment,
    'textColor': style.textColor,
    'backgroundColor': style.backgroundColor,
    'opacity': style.opacity,
    'cornerRadius': style.cornerRadius,
    'locked': style.locked,
    'x': style.positionX,
    'y': style.positionY,
  };

  @override
  Future<List<String>> listSystemFonts() async {
    if (!Platform.isWindows) return const [];
    final cached = _systemFonts;
    if (cached != null) return cached;
    try {
      final names =
          await _systemChannel.invokeListMethod<String>('listFonts') ?? [];
      final fonts = names.where((name) => name.trim().isNotEmpty).toList()
        ..sort((a, b) => a.toLowerCase().compareTo(b.toLowerCase()));
      return _systemFonts = List.unmodifiable(fonts);
    } on Object {
      return const [];
    }
  }

  @override
  Future<String?> defaultSystemFont() async {
    if (!Platform.isWindows) return null;
    if (_defaultFont != null) return _defaultFont;
    try {
      final name = await _systemChannel.invokeMethod<String>('defaultFont');
      return _defaultFont = (name == null || name.isEmpty) ? null : name;
    } on Object {
      return null;
    }
  }

  @override
  Future<void> dispose() async {
    if (_handlerRegistered) {
      try {
        _channel.setMethodCallHandler(null);
      } on Object {
        // 测试或无绑定环境忽略
      }
      _handlerRegistered = false;
    }
    if (!_enabled) return;
    _enabled = false;
    await _channel.invokeMethod<void>('setEnabled', {'enabled': false});
  }
}
