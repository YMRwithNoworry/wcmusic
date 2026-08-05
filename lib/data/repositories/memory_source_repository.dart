import '../../domain/models/track.dart';
import '../../domain/repositories/source_repository.dart';
import '../services/native_core_bridge.dart';
import '../services/source_script_parser.dart';
import '../services/source_resolver.dart';
import '../services/source_storage.dart';

class MemorySourceRepository implements SourceRepository {
  static const builtInSourceId = 'wcmusic-paojiao-internal-source';
  static const builtInSourceKeys = ['kg', 'kw', 'mg', 'tx', 'wy'];

  MemorySourceRepository({
    this._parser = const SourceScriptParser(),
    NativeCoreBridge? nativeCore,
    this._storage,
    this.builtInScript,
    this.builtInSourceName = '内置音源',
  }) : _nativeCore = nativeCore ?? NativeCoreBridge();

  final SourceScriptParser _parser;
  final NativeCoreBridge _nativeCore;
  final SourceStorage? _storage;
  final String? builtInScript;
  final String builtInSourceName;
  final List<SourceScript> _sources = [];
  String? _selectedSourceId;
  bool _loaded = false;

  @override
  Future<List<SourceScript>> loadSources() async {
    await _ensureLoaded();
    return List.unmodifiable(_sources);
  }

  @override
  Future<String?> loadSelectedSourceId() async {
    await _ensureLoaded();
    return _selectedSourceId;
  }

  @override
  Future<void> selectSource(String? id) async {
    await _ensureLoaded();
    if (id != null && !_sources.any((source) => source.id == id)) {
      throw StateError('没有找到要选择的音源');
    }
    _selectedSourceId = id;
    await _persist();
  }

  @override
  Future<SourceScript> importScript(String rawScript) async {
    await _ensureLoaded();
    final nativeResult = _nativeCore.validateSource(rawScript);
    final data = nativeResult != null && nativeResult['ok'] == true
        ? nativeResult['data']
        : null;
    var source = _parser.parse(
      rawScript,
      manifest: data is Map<String, dynamic> ? data : null,
    );
    if (source.sourceKeys.isEmpty || data == null) {
      source = _parser.parse(
        rawScript,
        manifest: {
          'metadata': {'name': source.name},
          'sources': [
            for (final key in builtInSourceKeys) {'key': key},
          ],
        },
      );
    }
    _sources.add(source);
    await _persist();
    return source;
  }

  @override
  Future<void> deleteSource(String id) async {
    await _ensureLoaded();
    if (id == builtInSourceId) throw StateError('内置音源不可删除');
    _sources.removeWhere((source) => source.id == id);
    if (_selectedSourceId == id) _selectedSourceId = null;
    await _persist();
  }

  @override
  Future<String> resolveUrl(
    Track track, {
    String quality = '320k',
    bool background = false,
  }) async {
    await _ensureLoaded();
    final sourceId = track.sourceId;
    if (sourceId == null || sourceId.isEmpty) {
      throw StateError('${track.title} 缺少平台歌曲 ID');
    }
    final sourceKey = track.source.name;
    SourceScript? selected;
    for (final source in _sources) {
      if (source.id == _selectedSourceId) {
        selected = source;
        break;
      }
    }
    if (selected != null) {
      if (!selected.sourceKeys.contains(sourceKey)) {
        throw StateError('所选音源 ${selected.name} 不支持 $sourceKey');
      }
      final script = selected.rawScript;
      if (background && _nativeCore.isLoaded) {
        return resolveSourceUrlInBackground(
          script: script,
          source: sourceKey,
          songId: sourceId,
          quality: quality,
        );
      }
      return _nativeCore.resolveSourceUrl(
        script: script,
        source: sourceKey,
        songId: sourceId,
        quality: quality,
      );
    }
    final candidates = _sources
        .where((source) => source.sourceKeys.contains(sourceKey))
        .toList(growable: false);
    if (candidates.isEmpty) throw StateError('没有支持 $sourceKey 的已启用音源');
    final script = candidates.last.rawScript;
    if (background && _nativeCore.isLoaded) {
      return resolveSourceUrlInBackground(
        script: script,
        source: sourceKey,
        songId: sourceId,
        quality: quality,
      );
    }
    return _nativeCore.resolveSourceUrl(
      script: script,
      source: sourceKey,
      songId: sourceId,
      quality: quality,
    );
  }

  Future<void> _ensureLoaded() async {
    if (_loaded) return;
    final storage = _storage;
    final stored = await storage?.read() ?? const [];
    _selectedSourceId = await storage?.readSelectedSourceId();
    _sources
      ..clear()
      ..addAll(stored.map(_decode));
    final bundledScript = builtInScript;
    if (bundledScript != null) {
      final nativeResult = _nativeCore.validateSource(bundledScript);
      final data = nativeResult != null && nativeResult['ok'] == true
          ? nativeResult['data']
          : null;
      final manifest = data is Map<String, dynamic> ? data : null;
      var parsed = _parser.parse(bundledScript, manifest: manifest);
      if (parsed.sourceKeys.isEmpty) {
        parsed = _parser.parse(
          bundledScript,
          manifest: {
            'metadata': {'name': builtInSourceName},
            'sources': [
              for (final key in builtInSourceKeys) {'key': key},
            ],
          },
        );
      }
      _sources.removeWhere(
        (source) =>
            source.id == builtInSourceId || source.name == builtInSourceName,
      );
      _sources.add(
        SourceScript(
          id: builtInSourceId,
          name: builtInSourceName,
          version: parsed.version,
          author: parsed.author,
          description: parsed.description,
          sourceKeys: parsed.sourceKeys,
          rawScript: parsed.rawScript,
          isBuiltIn: true,
        ),
      );
    }
    if (_selectedSourceId != null &&
        !_sources.any((source) => source.id == _selectedSourceId)) {
      _selectedSourceId = null;
    }
    _loaded = true;
  }

  Future<void> _persist() async {
    await _storage?.write(
      _sources.map(_encode).toList(growable: false),
      selectedSourceId: _selectedSourceId,
    );
  }

  static Map<String, dynamic> _encode(SourceScript source) => {
    'id': source.id,
    'name': source.name,
    'version': source.version,
    'author': source.author,
    'description': source.description,
    'sourceKeys': source.sourceKeys,
    'rawScript': source.rawScript,
    'isBuiltIn': source.isBuiltIn,
  };

  static SourceScript _decode(Map<String, dynamic> value) => SourceScript(
    id: value['id'] as String,
    name: value['name'] as String,
    version: value['version'] as String,
    author: value['author'] as String,
    description: value['description'] as String,
    sourceKeys: (value['sourceKeys'] as List).cast<String>(),
    rawScript: value['rawScript'] as String,
    isBuiltIn: value['isBuiltIn'] as bool? ?? false,
  );
}
