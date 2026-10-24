import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';

/// 主题与动效偏好。
///
/// 只保存很小的一份 JSON；读取失败时回退到系统主题，保证启动路径不会因为
/// 设置文件损坏而变慢或抛错。
class AppThemeController extends ChangeNotifier {
  ThemeMode _mode = ThemeMode.system;
  bool _reduceMotion = false;
  File? _settingsFile;

  ThemeMode get mode => _mode;

  /// 关闭呼吸、视差与大幅转场，只保留必要的状态反馈。
  bool get reduceMotion => _reduceMotion;

  Future<void> load() async {
    try {
      final file = await _file();
      if (!await file.exists()) return;
      final value = jsonDecode(await file.readAsString());
      if (value is String) {
        // 兼容旧版本只存主题名。
        _mode = _parseMode(value);
      } else if (value is Map) {
        _mode = _parseMode(value['mode']?.toString());
        _reduceMotion = value['reduceMotion'] == true;
      }
      notifyListeners();
    } on Object {
      // 保留系统主题作为设置缺失或损坏时的默认值。
    }
  }

  Future<void> setMode(ThemeMode mode) async {
    if (_mode == mode) return;
    _mode = mode;
    notifyListeners();
    await _save();
  }

  Future<void> setReduceMotion(bool value) async {
    if (_reduceMotion == value) return;
    _reduceMotion = value;
    notifyListeners();
    await _save();
  }

  ThemeMode _parseMode(String? value) => ThemeMode.values.firstWhere(
    (mode) => mode.name == value,
    orElse: () => ThemeMode.system,
  );

  Future<void> _save() async {
    try {
      final file = await _file();
      await file.parent.create(recursive: true);
      await file.writeAsString(
        jsonEncode({'mode': _mode.name, 'reduceMotion': _reduceMotion}),
        flush: true,
      );
    } on Object {
      // 即使设置无法落盘，本次运行仍立即应用所选偏好。
    }
  }

  Future<File> _file() async {
    final cached = _settingsFile;
    if (cached != null) return cached;
    final directory = await getApplicationSupportDirectory();
    return _settingsFile = File(
      '${directory.path}${Platform.pathSeparator}theme_mode.json',
    );
  }
}
