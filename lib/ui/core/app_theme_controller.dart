import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';

class AppThemeController extends ChangeNotifier {
  ThemeMode _mode = ThemeMode.system;
  ThemeMode get mode => _mode;

  Future<void> load() async {
    try {
      final file = await _settingsFile();
      if (!await file.exists()) return;
      final value = jsonDecode(await file.readAsString());
      if (value is! String) return;
      _mode = ThemeMode.values.firstWhere(
        (mode) => mode.name == value,
        orElse: () => ThemeMode.system,
      );
      notifyListeners();
    } on Object {
      // 保留系统主题作为设置缺失或损坏时的默认值。
    }
  }

  Future<void> setMode(ThemeMode mode) async {
    if (_mode == mode) return;
    _mode = mode;
    notifyListeners();
    try {
      final file = await _settingsFile();
      await file.parent.create(recursive: true);
      await file.writeAsString(jsonEncode(mode.name));
    } on Object {
      // 即使设置无法落盘，本次运行仍立即应用所选主题。
    }
  }

  Future<File> _settingsFile() async {
    final directory = await getApplicationSupportDirectory();
    return File('${directory.path}${Platform.pathSeparator}theme_mode.json');
  }
}
