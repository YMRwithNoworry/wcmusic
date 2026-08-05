import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/repositories/memory_source_repository.dart';
import 'package:wcmusic/data/services/source_script_parser.dart';
import 'package:wcmusic/data/services/source_storage.dart';

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
}
