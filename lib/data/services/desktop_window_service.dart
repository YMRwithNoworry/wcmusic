import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

abstract interface class WindowLifecycleService {
  bool get closeToTray;
  Future<void> setCloseToTray(bool value);
}

class DesktopWindowService
    with WindowListener, TrayListener
    implements WindowLifecycleService {
  static const _showWindowKey = 'show-window';
  static const _exitKey = 'exit';
  static const _trayIcon = 'windows/runner/resources/app_icon.ico';

  bool _closeToTray = true;
  bool _exiting = false;
  File? _settingsFile;

  @override
  bool get closeToTray => _closeToTray;

  Future<void> initialize() async {
    if (!Platform.isWindows) return;
    await _loadSettings();
    await windowManager.ensureInitialized();
    windowManager.addListener(this);
    trayManager.addListener(this);
    await windowManager.setPreventClose(_closeToTray);
    await trayManager.setIcon(_trayIcon);
    await trayManager.setToolTip('WCMusic');
    await trayManager.setContextMenu(
      Menu(
        items: [
          MenuItem(key: _showWindowKey, label: '显示 WCMusic'),
          MenuItem.separator(),
          MenuItem(key: _exitKey, label: '退出'),
        ],
      ),
    );
  }

  @override
  Future<void> setCloseToTray(bool value) async {
    _closeToTray = value;
    if (Platform.isWindows) await windowManager.setPreventClose(value);
    await _saveSettings();
  }

  @override
  void onWindowClose() {
    if (!_exiting && _closeToTray) unawaited(windowManager.hide());
  }

  @override
  void onTrayIconMouseDown() => unawaited(_showWindow());

  @override
  void onTrayMenuItemClick(MenuItem menuItem) {
    switch (menuItem.key) {
      case _showWindowKey:
        unawaited(_showWindow());
        return;
      case _exitKey:
        unawaited(_exitApplication());
        return;
    }
  }

  Future<void> _showWindow() async {
    await windowManager.show();
    await windowManager.restore();
    await windowManager.focus();
  }

  Future<void> _exitApplication() async {
    _exiting = true;
    await windowManager.setPreventClose(false);
    await trayManager.destroy();
    await windowManager.destroy();
  }

  Future<void> _loadSettings() async {
    final directory = await getApplicationSupportDirectory();
    _settingsFile = File(
      '${directory.path}${Platform.pathSeparator}window_settings.json',
    );
    if (!await _settingsFile!.exists()) return;
    try {
      final value = jsonDecode(await _settingsFile!.readAsString());
      if (value is Map && value['closeToTray'] is bool) {
        _closeToTray = value['closeToTray'] as bool;
      }
    } on Object {
      _closeToTray = true;
    }
  }

  Future<void> _saveSettings() async {
    final file = _settingsFile;
    if (file == null) return;
    await file.parent.create(recursive: true);
    await file.writeAsString(jsonEncode({'closeToTray': _closeToTray}));
  }
}
