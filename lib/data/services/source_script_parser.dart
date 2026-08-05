import 'package:uuid/uuid.dart';

import '../../domain/models/track.dart';

class SourceScriptParser {
  const SourceScriptParser();

  SourceScript parse(String rawScript, {Map<String, dynamic>? manifest}) {
    String? field(String name) {
      final pattern = r'^\s*\*?\s*@' + RegExp.escape(name) + r'\s+(.+?)\s*$';
      final match = RegExp(pattern, multiLine: true).firstMatch(rawScript);
      return match?.group(1)?.trim();
    }

    final metadata = manifest?['metadata'];
    final metadataMap = metadata is Map<String, dynamic> ? metadata : null;
    final name = _string(metadataMap?['name']) ?? field('name');
    if (name == null || name.isEmpty) throw const FormatException('脚本缺少 @name');
    final keys = <String>{};
    final manifestSources = manifest?['sources'];
    if (manifestSources is List) {
      for (final source in manifestSources) {
        if (source is Map) {
          final key = _string(source['key']);
          if (key != null) keys.add(key);
        }
      }
    }
    if (keys.isEmpty) {
      final sourceKeyPattern = RegExp(
        r"([a-zA-Z][a-zA-Z0-9_-]*)\s*:\s*\{[^{}]*?actions\s*:",
        dotAll: true,
      );
      for (final match in sourceKeyPattern.allMatches(rawScript)) {
        keys.add(match.group(1)!);
      }
    }
    return SourceScript(
      id: const Uuid().v4(),
      name: name,
      version: _string(metadataMap?['version']) ?? field('version') ?? '未声明',
      author: _string(metadataMap?['author']) ?? field('author') ?? '未声明',
      description:
          _string(metadataMap?['description']) ??
          field('description') ??
          '洛雪兼容音源',
      sourceKeys: keys.toList()..sort(),
      rawScript: rawScript,
    );
  }

  static String? _string(Object? value) {
    if (value is! String) return null;
    final trimmed = value.trim();
    return trimmed.isEmpty ? null : trimmed;
  }
}
