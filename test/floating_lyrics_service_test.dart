import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wcmusic/data/services/floating_lyrics_service.dart';
import 'package:wcmusic/domain/models/lyrics_overlay_style.dart';

void main() {
  test('persists lyrics style across service instances', () async {
    final directory = await Directory.systemTemp.createTemp('wcmusic-lyrics-');
    addTearDown(() => directory.delete(recursive: true));
    final first = PlatformFloatingLyricsService(
      directoryProvider: () async => directory,
    );
    addTearDown(first.dispose);
    const style = LyricsOverlayStyle(
      fontFamily: 'SimHei',
      fontSize: 36,
      alignment: 'right',
      textColor: 0xFFFF0000,
      backgroundColor: 0xFF000000,
      opacity: 0.5,
      cornerRadius: 8,
      locked: false,
      positionX: 120,
      positionY: 400,
    );

    await first.saveStyle(style);
    final second = PlatformFloatingLyricsService(
      directoryProvider: () async => directory,
    );
    addTearDown(second.dispose);

    final loaded = await second.loadStyle();

    expect(loaded.fontFamily, 'SimHei');
    expect(loaded.fontSize, 36);
    expect(loaded.alignment, 'right');
    expect(loaded.opacity, 0.5);
    expect(loaded.locked, isFalse);
    expect(loaded.positionX, 120);
    expect(loaded.positionY, 400);
  });
}
