import 'dart:io';

import 'package:flutter/services.dart';

abstract interface class FloatingLyricsService {
  bool get isSupported;
  Future<bool> setEnabled(bool enabled);
  Future<void> update({
    required String title,
    required String currentLine,
    required String nextLine,
  });
  Future<void> dispose();
}

class PlatformFloatingLyricsService implements FloatingLyricsService {
  static const _channel = MethodChannel('wcmusic/lyrics_overlay');

  bool _enabled = false;

  @override
  bool get isSupported => Platform.isWindows || Platform.isAndroid;

  @override
  Future<bool> setEnabled(bool enabled) async {
    if (!isSupported) return false;
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
  }) async {
    if (!_enabled) return;
    await _channel.invokeMethod<void>('update', {
      'title': title,
      'currentLine': currentLine,
      'nextLine': nextLine,
    });
  }

  @override
  Future<void> dispose() async {
    if (!_enabled) return;
    _enabled = false;
    await _channel.invokeMethod<void>('setEnabled', {'enabled': false});
  }
}
