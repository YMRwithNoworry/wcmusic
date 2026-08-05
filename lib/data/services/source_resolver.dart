import 'dart:isolate';

import 'native_core_bridge.dart';

Future<String> resolveSourceUrlInBackground({
  required String script,
  required String source,
  required String songId,
  required String quality,
}) {
  return Isolate.run(() {
    final bridge = NativeCoreBridge();
    return bridge.resolveSourceUrl(
      script: script,
      source: source,
      songId: songId,
      quality: quality,
    );
  });
}
