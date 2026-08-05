import '../../domain/models/track.dart';
import '../../domain/repositories/source_repository.dart';
import '../services/native_core_bridge.dart';
import '../services/source_script_parser.dart';
import '../services/source_storage.dart';

class MemorySourceRepository implements SourceRepository {
  MemorySourceRepository({
    this._parser = const SourceScriptParser(),
    NativeCoreBridge? nativeCore,
    this._storage,
  }) : _nativeCore = nativeCore ?? NativeCoreBridge();

  final SourceScriptParser _parser;
  final NativeCoreBridge _nativeCore;
  final SourceStorage? _storage;
  final List<SourceScript> _sources = [];
  bool _loaded = false;

  @override
  Future<List<SourceScript>> loadSources() async {
    await _ensureLoaded();
    return List.unmodifiable(_sources);
  }

  @override
  Future<SourceScript> importScript(String rawScript) async {
    await _ensureLoaded();
    final nativeResult = _nativeCore.validateSource(rawScript);
    if (nativeResult != null && nativeResult['ok'] != true) {
      throw FormatException(
        nativeResult['error'] as String? ?? 'Rust 音源运行时拒绝了该脚本',
      );
    }
    final data = nativeResult?['data'];
    final source = _parser.parse(
      rawScript,
      manifest: data is Map<String, dynamic> ? data : null,
    );
    _sources.removeWhere((item) => item.name == source.name);
    _sources.add(source);
    await _persist();
    return source;
  }

  @override
  Future<void> deleteSource(String id) async {
    await _ensureLoaded();
    _sources.removeWhere((source) => source.id == id);
    await _persist();
  }

  @override
  Future<String> resolveUrl(Track track, {String quality = '320k'}) async {
    await _ensureLoaded();
    final sourceId = track.sourceId;
    if (sourceId == null || sourceId.isEmpty) {
      throw StateError('${track.title} 缺少平台歌曲 ID');
    }
    final sourceKey = track.source.name;
    final candidates = _sources
        .where((source) => source.sourceKeys.contains(sourceKey))
        .toList(growable: false);
    if (candidates.isEmpty) throw StateError('没有支持 $sourceKey 的已启用音源');
    return _nativeCore.resolveSourceUrl(
      script: candidates.last.rawScript,
      source: sourceKey,
      songId: sourceId,
      quality: quality,
    );
  }

  Future<void> _ensureLoaded() async {
    if (_loaded) return;
    final stored = await _storage?.read() ?? const [];
    _sources
      ..clear()
      ..addAll(stored.map(_decode));
    _loaded = true;
  }

  Future<void> _persist() async {
    await _storage?.write(_sources.map(_encode).toList(growable: false));
  }

  static Map<String, dynamic> _encode(SourceScript source) => {
    'id': source.id,
    'name': source.name,
    'version': source.version,
    'author': source.author,
    'description': source.description,
    'sourceKeys': source.sourceKeys,
    'rawScript': source.rawScript,
  };

  static SourceScript _decode(Map<String, dynamic> value) => SourceScript(
    id: value['id'] as String,
    name: value['name'] as String,
    version: value['version'] as String,
    author: value['author'] as String,
    description: value['description'] as String,
    sourceKeys: (value['sourceKeys'] as List).cast<String>(),
    rawScript: value['rawScript'] as String,
  );
}
