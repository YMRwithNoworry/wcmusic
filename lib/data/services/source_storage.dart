import 'dart:convert';
import 'dart:io';

import 'package:path_provider/path_provider.dart';

abstract interface class SourceStorage {
  Future<List<Map<String, dynamic>>> read();
  Future<void> write(List<Map<String, dynamic>> sources);
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

  @override
  Future<List<Map<String, dynamic>>> read() async {
    final file = await _sourceFile();
    if (!await file.exists()) return const [];
    final decoded = jsonDecode(await file.readAsString());
    if (decoded is! List) throw const FormatException('音源存储文件格式无效');
    return decoded
        .whereType<Map>()
        .map((item) => Map<String, dynamic>.from(item))
        .toList(growable: false);
  }

  @override
  Future<void> write(List<Map<String, dynamic>> sources) async {
    final file = await _sourceFile();
    await file.writeAsString(jsonEncode(sources), flush: true);
  }
}
