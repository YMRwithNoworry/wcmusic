import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

abstract interface class SourceStorage {
  Future<List<Map<String, dynamic>>> read();
  Future<String?> readSelectedSourceId();
  Future<void> write(
    List<Map<String, dynamic>> sources, {
    String? selectedSourceId,
  });
}

class FileSourceStorage implements SourceStorage {
  FileSourceStorage({Future<Directory> Function()? directoryProvider})
    : _directoryProvider = directoryProvider ?? getApplicationSupportDirectory;

  final Future<Directory> Function() _directoryProvider;

  Future<File> _sourceFile() async {
    final directory = await _directoryProvider();
    if (!await directory.exists()) await directory.create(recursive: true);
    return File('${directory.path}${Platform.pathSeparator}sources.json');
  }

  Future<dynamic> _readDecoded() async {
    final file = await _sourceFile();
    if (!await file.exists()) return null;
    final decoded = jsonDecode(await file.readAsString());
    if (decoded is! List && decoded is! Map) {
      throw const FormatException('音源存储文件格式无效');
    }
    return decoded;
  }

  @override
  Future<List<Map<String, dynamic>>> read() async {
    final decoded = await _readDecoded();
    if (decoded == null) return const [];
    final values = decoded is List ? decoded : (decoded as Map)['sources'];
    if (values is! List) {
      throw const FormatException('音源存储文件格式无效');
    }
    return values
        .whereType<Map>()
        .map((item) => Map<String, dynamic>.from(item))
        .toList(growable: false);
  }

  @override
  Future<String?> readSelectedSourceId() async {
    final decoded = await _readDecoded();
    if (decoded is! Map) return null;
    final selected = decoded['selectedSourceId'];
    return selected is String && selected.isNotEmpty ? selected : null;
  }

  @override
  Future<void> write(
    List<Map<String, dynamic>> sources, {
    String? selectedSourceId,
  }) async {
    final file = await _sourceFile();
    await file.writeAsString(
      jsonEncode({'selectedSourceId': selectedSourceId, 'sources': sources}),
      flush: true,
    );
  }
}
