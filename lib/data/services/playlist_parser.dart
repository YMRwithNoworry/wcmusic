import 'dart:convert';

import 'package:uuid/uuid.dart';

import '../../domain/models/track.dart';

class PlaylistParser {
  const PlaylistParser();

  Playlist parseM3u(List<int> bytes, String name) {
    final text = _decode(bytes);
    final tracks = <Track>[];
    int? duration;
    String? label;

    for (final raw in text.split(RegExp(r'\r?\n'))) {
      final line = raw.trim().replaceFirst('\ufeff', '');
      if (line.isEmpty || line == '#EXTM3U') continue;
      if (line.startsWith('#EXTINF:')) {
        final info = line.substring(8);
        final comma = info.indexOf(',');
        final durationValue = comma == -1 ? info : info.substring(0, comma);
        duration = (int.tryParse(durationValue) ?? 0).clamp(0, 86400);
        label = comma == -1 ? '未命名曲目' : info.substring(comma + 1).trim();
        continue;
      }
      if (line.startsWith('#')) continue;
      final parts = (label ?? _fileTitle(line)).split(' - ');
      final artist = parts.length > 1 ? parts.first.trim() : '';
      final title = parts.length > 1
          ? parts.skip(1).join(' - ').trim()
          : parts.first.trim();
      tracks.add(
        Track(
          id: const Uuid().v4(),
          title: title.isEmpty ? '未命名曲目' : title,
          artist: artist,
          album: '导入歌单',
          duration: Duration(seconds: duration ?? 0),
          uri: line,
        ),
      );
      duration = null;
      label = null;
    }
    if (tracks.isEmpty) throw const FormatException('没有找到可导入的曲目');
    return Playlist(id: const Uuid().v4(), name: name, tracks: tracks);
  }

  String _decode(List<int> bytes) {
    try {
      return utf8.decode(bytes);
    } catch (_) {
      return String.fromCharCodes(bytes);
    }
  }

  String _fileTitle(String path) {
    final normalized = path.replaceAll('\\', '/');
    final filename = normalized.split('/').last;
    return filename.replaceFirst(RegExp(r'\.[^.]+$'), '');
  }
}
