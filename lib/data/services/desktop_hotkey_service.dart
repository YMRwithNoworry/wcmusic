import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

class DesktopHotkeyAction {
  const DesktopHotkeyAction(this.id, this.label, this.defaultBinding);

  final String id;
  final String label;
  final String defaultBinding;
}

const desktopHotkeyActions = [
  DesktopHotkeyAction('previous', '上一首', 'ctrl-alt-left'),
  DesktopHotkeyAction('next', '下一首', 'ctrl-alt-right'),
  DesktopHotkeyAction('togglePlay', '播放/暂停', 'ctrl-alt-space'),
  DesktopHotkeyAction('volumeUp', '音量加', 'ctrl-alt-up'),
  DesktopHotkeyAction('volumeDown', '音量减', 'ctrl-alt-down'),
  DesktopHotkeyAction('mute', '静音', 'ctrl-alt-m'),
  DesktopHotkeyAction('seekForward', '快进 5 秒', 'ctrl-alt-shift-right'),
  DesktopHotkeyAction('seekBackward', '快退 5 秒', 'ctrl-alt-shift-left'),
  DesktopHotkeyAction('toggleLyrics', '显示/隐藏桌面歌词', 'ctrl-alt-l'),
  DesktopHotkeyAction('toggleWindow', '显示/隐藏主界面', 'ctrl-alt-h'),
];

class DesktopHotkeySettings {
  DesktopHotkeySettings({required this.enabled, required Map<String, String> bindings})
    : bindings = Map.unmodifiable(bindings);

  factory DesktopHotkeySettings.defaults() => DesktopHotkeySettings(
    enabled: false,
    bindings: {
      for (final action in desktopHotkeyActions)
        action.id: action.defaultBinding,
    },
  );

  factory DesktopHotkeySettings.fromJson(Map<String, dynamic> json) {
    final defaults = DesktopHotkeySettings.defaults();
    final stored = json['bindings'];
    final bindings = <String, String>{};
    for (final action in desktopHotkeyActions) {
      final value = stored is Map ? stored[action.id] : null;
      bindings[action.id] = value is String ? value : defaults.bindings[action.id]!;
    }
    return DesktopHotkeySettings(
      enabled: json['enabled'] as bool? ?? false,
      bindings: bindings,
    );
  }

  final bool enabled;
  final Map<String, String> bindings;

  DesktopHotkeySettings copyWith({bool? enabled, Map<String, String>? bindings}) =>
      DesktopHotkeySettings(
        enabled: enabled ?? this.enabled,
        bindings: bindings ?? this.bindings,
      );

  Map<String, dynamic> toJson() => {'enabled': enabled, 'bindings': bindings};
}

abstract interface class DesktopHotkeyService {
  bool get isSupported;
  String? get lastError;
  Stream<String> get actions;
  Future<DesktopHotkeySettings> load();
  Future<void> save(DesktopHotkeySettings settings);
  Future<void> dispose();
}

class PlatformDesktopHotkeyService implements DesktopHotkeyService {
  PlatformDesktopHotkeyService({
    Future<Directory> Function()? directoryProvider,
    bool? supported,
  }) : _directoryProvider = directoryProvider ?? getApplicationSupportDirectory,
       _supported = supported ?? Platform.isWindows;

  static const _channel = MethodChannel('wcmusic/global_hotkeys');

  final Future<Directory> Function() _directoryProvider;
  final bool _supported;
  final StreamController<String> _actions = StreamController<String>.broadcast();
  File? _settingsFile;
  DesktopHotkeySettings? _settings;
  bool _handlerInstalled = false;
  String? _lastError;

  @override
  bool get isSupported => _supported;

  @override
  String? get lastError => _lastError;

  @override
  Stream<String> get actions => _actions.stream;

  @override
  Future<DesktopHotkeySettings> load() async {
    if (_settings != null) return _settings!;
    var loaded = DesktopHotkeySettings.defaults();
    final file = await _settingsFileFor();
    if (await file.exists()) {
      try {
        final value = jsonDecode(await file.readAsString());
        if (value is Map<String, dynamic>) {
          loaded = DesktopHotkeySettings.fromJson(value);
        }
      } on Object {
        loaded = DesktopHotkeySettings.defaults();
      }
    }
    if (isSupported) {
      _installHandler();
      try {
        await _apply(loaded);
      } on PlatformException catch (error) {
        _lastError = error.message ?? '快捷键注册失败';
      }
    }
    _settings = loaded;
    return loaded;
  }

  @override
  Future<void> save(DesktopHotkeySettings settings) async {
    if (isSupported) {
      _installHandler();
      await _apply(settings);
    }
    final file = await _settingsFileFor();
    await file.parent.create(recursive: true);
    await file.writeAsString(jsonEncode(settings.toJson()), flush: true);
    _settings = settings;
    _lastError = null;
  }

  void _installHandler() {
    if (_handlerInstalled) return;
    _handlerInstalled = true;
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'action' && call.arguments is String) {
        _actions.add(call.arguments as String);
      }
    });
  }

  Future<void> _apply(DesktopHotkeySettings settings) async {
    final bindings = settings.enabled
        ? Map<String, String>.from(settings.bindings)
        : <String, String>{};
    await _channel.invokeMethod<void>('setBindings', {'bindings': bindings});
  }

  Future<File> _settingsFileFor() async {
    if (_settingsFile != null) return _settingsFile!;
    final directory = await _directoryProvider();
    _settingsFile = File(
      '${directory.path}${Platform.pathSeparator}hotkey_settings.json',
    );
    return _settingsFile!;
  }

  @override
  Future<void> dispose() async {
    if (_handlerInstalled) {
      _channel.setMethodCallHandler(null);
      _handlerInstalled = false;
    }
    if (isSupported) {
      try {
        await _channel.invokeMethod<void>('setBindings', {
          'bindings': <String, String>{},
        });
      } on Object {
        // Channel may already be detached during shutdown.
      }
    }
    await _actions.close();
  }
}

String? hotkeyBindingFromEvent(KeyEvent event) {
  if (event is! KeyDownEvent) return null;
  final key = event.logicalKey;
  final keyName = switch (key) {
    LogicalKeyboardKey.space => 'space',
    LogicalKeyboardKey.arrowLeft => 'left',
    LogicalKeyboardKey.arrowRight => 'right',
    LogicalKeyboardKey.arrowUp => 'up',
    LogicalKeyboardKey.arrowDown => 'down',
    LogicalKeyboardKey.escape => 'escape',
    LogicalKeyboardKey.enter => 'enter',
    _ => key.keyLabel.trim().toLowerCase(),
  };
  if (keyName.isEmpty ||
      keyName == 'control' ||
      keyName == 'shift' ||
      keyName == 'alt' ||
      keyName == 'meta') {
    return null;
  }
  final parts = <String>[];
  final keyboard = HardwareKeyboard.instance;
  if (keyboard.isControlPressed) parts.add('ctrl');
  if (keyboard.isAltPressed) parts.add('alt');
  if (keyboard.isShiftPressed) parts.add('shift');
  if (keyboard.isMetaPressed) parts.add('win');
  if (parts.isEmpty && !RegExp(r'^f([1-9]|1[0-9]|2[0-4])$').hasMatch(keyName)) {
    return null;
  }
  parts.add(keyName);
  return parts.join('-');
}

String hotkeyBindingLabel(String binding) {
  if (binding.trim().isEmpty) return '未设置';
  final labels = {
    'ctrl': 'Ctrl',
    'alt': 'Alt',
    'shift': 'Shift',
    'win': 'Win',
    'left': '←',
    'right': '→',
    'up': '↑',
    'down': '↓',
    'space': '空格',
    'escape': 'Esc',
    'enter': '回车',
  };
  return binding
      .split('-')
      .map((part) => labels[part] ?? part.toUpperCase())
      .join(' + ');
}
