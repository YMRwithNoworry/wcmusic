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
  });
  Future<void> setStyle(LyricsOverlayStyle style);
  Future<LyricsOverlayStyle> loadStyle();
  Future<void> saveStyle(LyricsOverlayStyle style);
  Future<void> dispose();
}

class PlatformFloatingLyricsService implements FloatingLyricsService {
  PlatformFloatingLyricsService({
    Future<Directory> Function()? directoryProvider,
  }) : _directoryProvider = directoryProvider ?? getApplicationSupportDirectory;

  static const _channel = MethodChannel('wcmusic/lyrics_overlay');

  final Future<Directory> Function() _directoryProvider;
  bool _enabled = false;
  bool _handlerRegistered = false;
  LyricsOverlayStyle? _style;
  File? _settingsFile;

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
    return enabled ? _enabled : succeeded;
  }

  @override
  Future<void> update({
    required String title,
    required String currentLine,
    required String nextLine,
    List<String> lines = const [],
    int currentIndex = -1,
  }) async {
    if (!_enabled) return;
    _ensureHandler();
    await _channel.invokeMethod<void>('update', {
      'title': title,
      'currentLine': currentLine,
      'nextLine': nextLine,
      'lines': lines,
      'currentIndex': currentIndex,
    });
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
