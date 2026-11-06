import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/desktop_hotkey_service.dart';

void main() {
  test('hotkey settings fill defaults and preserve custom bindings', () {
    final settings = DesktopHotkeySettings.fromJson({
      'enabled': true,
      'bindings': {'togglePlay': 'ctrl-shift-p', 'mute': ''},
    });

    expect(settings.enabled, isTrue);
    expect(settings.bindings['togglePlay'], 'ctrl-shift-p');
    expect(settings.bindings['mute'], '');
    expect(settings.bindings['next'], 'ctrl-alt-right');
    expect(hotkeyBindingLabel('ctrl-alt-shift-right'), 'Ctrl + Alt + Shift + →');
  });

  test('persists hotkey settings across service instances', () async {
    final directory = await Directory.systemTemp.createTemp('wcmusic-hotkeys-');
    addTearDown(() => directory.delete(recursive: true));
    final first = PlatformDesktopHotkeyService(
      directoryProvider: () async => directory,
      supported: false,
    );
    final bindings = Map<String, String>.from(
      DesktopHotkeySettings.defaults().bindings,
    )..['togglePlay'] = 'ctrl-shift-p';
    final value = DesktopHotkeySettings(enabled: true, bindings: bindings);
    await first.save(value);
    await first.dispose();

    final second = PlatformDesktopHotkeyService(
      directoryProvider: () async => directory,
      supported: false,
    );
    addTearDown(second.dispose);
    final loaded = await second.load();

    expect(loaded.enabled, isTrue);
    expect(loaded.bindings['togglePlay'], 'ctrl-shift-p');
  });
}
