import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/data/services/native_core_bridge.dart';
import 'package:wcmusic/data/services/source_script_parser.dart';
import 'package:wcmusic/data/services/source_storage.dart';
import 'package:wcmusic/domain/models/track.dart';

const _script = r'''
/**
 * @name 林间测试源
 * @description 持久化测试
 * @version 1.2.0
 * @author WCMusic
 */
globalThis.lx.send(globalThis.lx.EVENT_NAMES.inited, {
  sources: { kw: { actions: ['musicUrl'] } }
})
''';

const _secondScript = r'''
/**
 * @name 二号测试源
 * @description 选择测试
 * @version 1.0.0
 * @author WCMusic
 */
globalThis.lx.send(globalThis.lx.EVENT_NAMES.inited, {
  sources: { kw: { actions: ['musicUrl'] } }
})
''';

const _quotedKeysScript = r'''
/**
 * @name 内嵌兜底源
 * @description 模拟无法通过正则识别渠道的混淆脚本
 * @version 1.0.0
 * @author WCMusic
 */
globalThis.lx.send(globalThis.lx.EVENT_NAMES.inited, {
  sources: { 'kw': { 'actions': ['musicUrl'] } }
})
''';

void main() {
  test('uses the Rust manifest as the authoritative source list', () {
    final source = const SourceScriptParser().parse(
      _script,
      manifest: {
        'metadata': {
          'name': '运行时名称',
          'version': '2.0.0',
          'author': 'Runtime',
          'description': '来自运行时',
        },
        'sources': [
          {'key': 'wy'},
          {'key': 'tx'},
        ],
      },
    );

    expect(source.name, '运行时名称');
    expect(source.version, '2.0.0');
    expect(source.sourceKeys, ['tx', 'wy']);
  });

  test('persists and deletes imported scripts', () async {
    final directory = await Directory.systemTemp.createTemp('wcmusic-source-');
    addTearDown(() => directory.delete(recursive: true));
    final storage = FileSourceStorage(directoryProvider: () async => directory);
    final first = MemorySourceRepository(storage: storage);

    final imported = await first.importScript(_script);
    final restored = MemorySourceRepository(storage: storage);
    expect((await restored.loadSources()).single.name, '林间测试源');

    await restored.deleteSource(imported.id);
    expect(await restored.loadSources(), isEmpty);
  });

  test(
    'resolves a full track url through the imported source runtime',
    () async {
      final repository = MemorySourceRepository(nativeCore: _FakeNativeCore());
      await repository.importScript(_script);
      const track = Track(
        id: 'online-123',
        title: '整曲测试',
        artist: '测试歌手',
        album: '测试专辑',
        duration: Duration(minutes: 4),
        uri: 'https://audio.example/preview.m4a',
        source: TrackSource.kw,
        sourceId: '123',
      );

      final url = await repository.resolveUrl(track);

      expect(url, 'https://audio.example/full.flac');
    },
  );

  test('persists the selected source across restarts', () async {
    final directory = await Directory.systemTemp.createTemp('wcmusic-source-');
    addTearDown(() => directory.delete(recursive: true));
    final storage = FileSourceStorage(directoryProvider: () async => directory);
    final first = MemorySourceRepository(storage: storage);
    final imported = await first.importScript(_secondScript);

    await first.selectSource(imported.id);
    final restored = MemorySourceRepository(storage: storage);

    expect(await restored.loadSelectedSourceId(), imported.id);
  });

  test('resolves through the user-selected source', () async {
    final repository = MemorySourceRepository(nativeCore: _FakeNativeCore());
    await repository.importScript(_script);
    final second = await repository.importScript(_secondScript);
    await repository.selectSource(second.id);
    const track = Track(
      id: 'online-123',
      title: '整曲测试',
      artist: '测试歌手',
      album: '测试专辑',
      duration: Duration(minutes: 4),
      uri: '',
      source: TrackSource.kw,
      sourceId: '123',
    );

    final url = await repository.resolveUrl(track);

    expect(url, 'https://audio.example/second.flac');
  });

  test('clears the selection when the selected source is deleted', () async {
    final repository = MemorySourceRepository(nativeCore: _FakeNativeCore());
    final imported = await repository.importScript(_secondScript);
    await repository.selectSource(imported.id);

    await repository.deleteSource(imported.id);

    expect(await repository.loadSelectedSourceId(), isNull);
  });

  test('loads the bundled source with its internal display name', () async {
    final repository = MemorySourceRepository(
      nativeCore: _FakeNativeCore(),
      builtInScript: _script,
      builtInSourceName: '泡椒内部测试音源',
    );

    final source = (await repository.loadSources()).single;

    expect(source.id, MemorySourceRepository.builtInSourceId);
    expect(source.name, '泡椒内部测试音源');
    expect(source.isBuiltIn, isTrue);
    expect(source.sourceKeys, ['kw']);
    expect(
      () => repository.deleteSource(source.id),
      throwsA(isA<StateError>()),
    );
  });

  test(
    'keeps the bundled source when the Rust runtime is unavailable',
    () async {
      final repository = MemorySourceRepository(
        nativeCore: _UnavailableNativeCore(),
        builtInScript: _quotedKeysScript,
        builtInSourceName: '泡椒内部测试音源',
      );

      final source = (await repository.loadSources()).single;

      expect(source.name, '泡椒内部测试音源');
      expect(source.sourceKeys, MemorySourceRepository.builtInSourceKeys);
    },
  );

  test('keeps the bundled source when validation rejects the script', () async {
    final repository = MemorySourceRepository(
      nativeCore: _RejectingNativeCore(),
      builtInScript: _quotedKeysScript,
      builtInSourceName: '泡椒内部测试音源',
    );

    final source = (await repository.loadSources()).single;

    expect(source.name, '泡椒内部测试音源');
    expect(source.sourceKeys, MemorySourceRepository.builtInSourceKeys);
  });
}

class _FakeNativeCore extends NativeCoreBridge {
  @override
  Map<String, dynamic>? validateSource(String script) => null;

  @override
  String resolveSourceUrl({
    required String script,
    required String source,
    required String songId,
    required String quality,
  }) => script.contains('二号测试源')
      ? 'https://audio.example/second.flac'
      : 'https://audio.example/full.flac';
}

class _UnavailableNativeCore extends NativeCoreBridge {
  @override
  Map<String, dynamic>? validateSource(String script) => null;

  @override
  String resolveSourceUrl({
    required String script,
    required String source,
    required String songId,
    required String quality,
  }) => throw UnsupportedError('Rust 音源运行时未加载');
}

class _RejectingNativeCore extends NativeCoreBridge {
  @override
  Map<String, dynamic>? validateSource(String script) => {
    'ok': false,
    'error': '移动端运行时暂不可用',
  };

  @override
  String resolveSourceUrl({
    required String script,
    required String source,
    required String songId,
    required String quality,
  }) => throw UnsupportedError('Rust 音源运行时未加载');
}
