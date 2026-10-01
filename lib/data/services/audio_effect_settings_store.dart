import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

abstract interface class AudioEffectSettingsStore {
  Future<bool> loadSpatialAudio();
  Future<void> saveSpatialAudio(bool enabled);
}

class FileAudioEffectSettingsStore implements AudioEffectSettingsStore {
  @override
  Future<bool> loadSpatialAudio() async {
    try {
      final file = await _settingsFile();
      if (!await file.exists()) return false;
      final value = jsonDecode(await file.readAsString());
      return value is Map && value['spatialAudio'] == true;
    } on Object {
      return false;
    }
  }

  @override
  Future<void> saveSpatialAudio(bool enabled) async {
    try {
      final file = await _settingsFile();
      await file.parent.create(recursive: true);
      await file.writeAsString(jsonEncode({'spatialAudio': enabled}));
    } on Object {
      // 音效开关仍在当前会话生效，设置文件不可用时保留默认行为。
    }
  }

  Future<File> _settingsFile() async {
    final directory = await getApplicationSupportDirectory();
    return File('${directory.path}${Platform.pathSeparator}audio_effects.json');
  }
}
