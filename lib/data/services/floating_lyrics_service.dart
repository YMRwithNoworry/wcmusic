import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

abstract interface class FloatingLyricsService {
  bool get isSupported;
  Future<bool> setEnabled(bool enabled);
  Future<void> update({
    required String title,
    required String currentLine,
    required String nextLine,
    String? currentTranslation,
    String? nextTranslation,
    List<String> lines = const [],
    int currentIndex = -1,
    double lineProgress = 0,
  });
  Future<void> setStyle(LyricsOverlayStyle style);
  Future<LyricsOverlayStyle> loadStyle();
  Future<void> saveStyle(LyricsOverlayStyle style);
  Stream<String> get events;
  Future<void> dispose();
}

class PlatformFloatingLyricsService implements FloatingLyricsService {
  PlatformFloatingLyricsService({
    Future<Directory> Function()? directoryProvider,
  }) : _directoryProvider = directoryProvider ?? getApplicationSupportDirectory;

  static const _channel = MethodChannel('wcmusic/lyrics_overlay');

  final Future<Directory> Function() _directoryProvider;
  final StreamController<String> _events = StreamController<String>.broadcast();
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
  Stream<String> get events => _events.stream;

  @override
  bool get isSupported => Platform.isWindows || Platform.isAndroid;

  @override
  Future<bool> setEnabled(bool enabled) async {
    if (!isSupported) return false;
    _ensureHandler();
    if (enabled) await setStyle(await loadStyle());
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
    String? currentTranslation,
    String? nextTranslation,
    List<String> lines = const [],
    int currentIndex = -1,
    double lineProgress = 0,
  }) async {
    if (!_enabled) return;
    _ensureHandler();
    await _channel.invokeMethod<void>('update', {
      'title': title,
      'currentLine': currentLine,
      'nextLine': nextLine,
      'currentTranslation': currentTranslation,
      'nextTranslation': nextTranslation,
      'lines': lines,
      'currentIndex': currentIndex,
      'lineProgress': lineProgress.clamp(0.0, 1.0),
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
    final arguments = call.arguments;
    if (arguments is! Map) return;
    if (call.method == 'toolbarAction') {
      await _handleToolbarAction(arguments['action']?.toString() ?? '');
      return;
    }
    if (call.method != 'positionChanged') return;
    final x = (arguments['x'] as num?)?.toDouble();
    final y = (arguments['y'] as num?)?.toDouble();
    if (x == null || y == null) return;
    final current = await loadStyle();
    _style = current.copyWith(
      positionX: x,
      positionY: y,
      windowWidth: (arguments['width'] as num?)?.toDouble(),
      windowHeight: (arguments['height'] as num?)?.toDouble(),
    );
    await saveStyle(_style!);
    _events.add('positionChanged');
  }

  Future<void> _handleToolbarAction(String action) async {
    if (action == 'close') {
      await setEnabled(false);
      _events.add('close');
      return;
    }
    final current = await loadStyle();
    final updated = switch (action) {
      'fontDown' => current.copyWith(
        fontSize: (current.fontSize - 2).clamp(16, 48).toDouble(),
      ),
      'fontUp' => current.copyWith(
        fontSize: (current.fontSize + 2).clamp(16, 48).toDouble(),
      ),
      'opacityDown' => current.copyWith(
        opacity: (current.opacity - 0.05).clamp(0.15, 1).toDouble(),
      ),
      'opacityUp' => current.copyWith(
        opacity: (current.opacity + 0.05).clamp(0.15, 1).toDouble(),
      ),
      'toggleLock' => current.copyWith(locked: !current.locked),
      'toggleAlwaysOnTop' => current.copyWith(
        alwaysOnTop: !current.alwaysOnTop,
      ),
      'toggleTranslation' => current.copyWith(
        showTranslation: !current.showTranslation,
      ),
      'toggleZoom' => current.copyWith(
        zoomActiveLine: !current.zoomActiveLine,
      ),
      _ => current,
    };
    if (updated == current) return;
    await setStyle(updated);
    await saveStyle(updated);
    _events.add(action);
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
    'alwaysOnTop': style.alwaysOnTop,
    'showTranslation': style.showTranslation,
    'zoomActiveLine': style.zoomActiveLine,
    'windowWidth': style.windowWidth,
    'windowHeight': style.windowHeight,
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
    if (_enabled) {
      _enabled = false;
      await _channel.invokeMethod<void>('setEnabled', {'enabled': false});
    }
    await _events.close();
  }
}
