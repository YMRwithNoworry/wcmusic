import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

abstract interface class WindowLifecycleService {
  bool get closeToTray;
  Future<void> setCloseToTray(bool value);

  /// 一键更新：立即退出进程（跳过关闭到托盘），由更新脚本重启新版本
  Future<void> exitForUpdate();
}

/// Windows 桌面窗口：无边框 + 自绘标题栏、最小尺寸、托盘、关闭到托盘。
///
/// 窗口状态只在这里维护，UI 通过 [maximized] 监听最大化状态，避免每帧查询原生窗口。
class DesktopWindowService
    with WindowListener, TrayListener
    implements WindowLifecycleService {
  static const _showWindowKey = 'show-window';
  static const _exitKey = 'exit';
  static const _trayIcon = 'windows/runner/resources/app_icon.ico';

  bool _closeToTray = true;
  bool _exiting = false;
  File? _settingsFile;

  /// 当前是否最大化，供标题栏切换图标。
  final ValueNotifier<bool> maximized = ValueNotifier(false);

  @override
  bool get closeToTray => _closeToTray;

  bool get isDesktop => Platform.isWindows;

  Future<void> initialize() async {
    if (!Platform.isWindows) return;
    await _loadSettings();
    await windowManager.ensureInitialized();
    windowManager.addListener(this);
    trayManager.addListener(this);
    await windowManager.waitUntilReadyToShow(
      const WindowOptions(
        size: Size(1320, 860),
        minimumSize: Size(960, 620),
        center: true,
        titleBarStyle: TitleBarStyle.hidden,
        backgroundColor: Colors.transparent,
        title: 'WCMusic',
      ),
      () async {
        await windowManager.show();
        await windowManager.focus();
      },
    );
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

  Future<void> minimize() async {
    if (isDesktop) await windowManager.minimize();
  }

  Future<void> toggleMaximize() async {
    if (!isDesktop) return;
    if (await windowManager.isMaximized()) {
      await windowManager.unmaximize();
    } else {
      await windowManager.maximize();
    }
  }

  Future<void> close() async {
    if (isDesktop) await windowManager.close();
  }

  Future<void> startDragging() async {
    if (isDesktop) await windowManager.startDragging();
  }

  /// 一键更新：不做「关闭到托盘」，直接退出进程，交给后台更新脚本替换文件后重启
  @override
  Future<void> exitForUpdate() async {
    if (isDesktop) await _exitApplication();
  }

  @override
  Future<void> setCloseToTray(bool value) async {
    _closeToTray = value;
    if (Platform.isWindows) await windowManager.setPreventClose(value);
    await _saveSettings();
  }

  @override
  void onWindowMaximize() => maximized.value = true;

  @override
  void onWindowUnmaximize() => maximized.value = false;

  @override
  void onWindowClose() {
    if (!_exiting && _closeToTray) unawaited(windowManager.hide());
  }

  @override
  void onTrayIconMouseDown() => unawaited(_showWindow());

  @override
  void onTrayIconRightMouseDown() => unawaited(trayManager.popUpContextMenu());

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
    if (_exiting) return;
    _exiting = true;
    await windowManager.setPreventClose(false);
    windowManager.removeListener(this);
    trayManager.removeListener(this);
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

  void dispose() {
    if (!isDesktop) return;
    windowManager.removeListener(this);
    trayManager.removeListener(this);
    maximized.dispose();
  }
}
