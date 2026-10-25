import 'dart:io';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:media_kit/media_kit.dart' hide Track;
import 'package:wcmusic/data/services/player_service.dart';
import 'package:wcmusic/domain/models/track.dart';

/// 在真实 libmpv 上验证「空间音效」的滤镜链可用，且不会让整首歌播不出来。
///
/// 回归的 bug：滤镜名在当前 libmpv 里不存在时，mpv 会报
/// “Audio filter initialized failed!” 并停掉音频输出，表现为「播放失败」。
/// 这个 libmpv 只编进了 FFmpeg 的 equalizer，aecho / pan / earwax 等都不存在。
void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();

  testWidgets('keeps playing after switching quality with spatial audio on', (
    tester,
  ) async {
    MediaKit.ensureInitialized();
    final wav = await tester.runAsync(_writeTestTone);
    expect(wav, isNotNull);

    final service = PlayerService();
    addTearDown(service.dispose);
    final errors = <String>[];
    final sub = service.errors.listen(errors.add);
    addTearDown(sub.cancel);

    final played = <String>[];
    final playingSub = service.playing.listen((value) {
      if (value) played.add('playing');
    });
    addTearDown(playingSub.cancel);

    // 首次启动时没有任何媒体，开启空间音效只能记录状态，不能提前初始化 af。
    await tester.runAsync(() => service.setSpatialAudioEnabled(true));

    final track = Track(
      id: 'tone',
      title: '测试音',
      artist: 'WCMusic',
      album: '测试',
      duration: const Duration(seconds: 2),
      uri: wav!,
    );

    // 第一次播放。
    await tester.runAsync(() => service.play(track));
    await _pumpFor(tester, const Duration(milliseconds: 900));
    expect(errors, isEmpty, reason: '第一次播放不应有错误：$errors');
    expect(played, isNotEmpty, reason: '第一次播放没有真正开始');

    // 播放中再开启一次（走「已经有媒体」的分支）。
    await tester.runAsync(() => service.setSpatialAudioEnabled(true));
    await _pumpFor(tester, const Duration(milliseconds: 400));
    expect(errors, isEmpty, reason: '播放中开启空间音效不应有错误：$errors');

    // 模拟“切换音质”：同一个播放器重新 open。
    played.clear();
    await tester.runAsync(() => service.play(track));
    await _pumpFor(tester, const Duration(milliseconds: 900));
    expect(errors, isEmpty, reason: '切音质后不应有错误：$errors');
    expect(played, isNotEmpty, reason: '切音质后没有真正播放');

    // 暂停 / 继续也必须正常。
    await tester.runAsync(service.toggle);
    await _pumpFor(tester, const Duration(milliseconds: 300));
    await tester.runAsync(service.toggle);
    await _pumpFor(tester, const Duration(milliseconds: 300));
    expect(errors, isEmpty, reason: '暂停/继续不应有错误：$errors');

    // 关闭后滤镜必须真的被摘掉，之后继续播放不受影响。
    await tester.runAsync(() => service.setSpatialAudioEnabled(false));
    await tester.runAsync(() => service.play(track));
    await _pumpFor(tester, const Duration(milliseconds: 600));
    expect(errors, isEmpty, reason: '关闭空间音效后不应有错误：$errors');
  }, timeout: const Timeout(Duration(minutes: 2)));
}

Future<void> _pumpFor(WidgetTester tester, Duration duration) async {
  final deadline = DateTime.now().add(duration);
  while (DateTime.now().isBefore(deadline)) {
    await tester.pump(const Duration(milliseconds: 50));
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }
}

/// 生成一段 1 秒的 440Hz WAV，避免测试依赖网络。
Future<String> _writeTestTone() async {
  const sampleRate = 44100;
  const seconds = 1;
  const samples = sampleRate * seconds;
  final data = ByteData(44 + samples * 2);
  void ascii(int offset, String value) {
    for (var i = 0; i < value.length; i++) {
      data.setUint8(offset + i, value.codeUnitAt(i));
    }
  }

  ascii(0, 'RIFF');
  data.setUint32(4, 36 + samples * 2, Endian.little);
  ascii(8, 'WAVE');
  ascii(12, 'fmt ');
  data.setUint32(16, 16, Endian.little);
  data.setUint16(20, 1, Endian.little);
  data.setUint16(22, 1, Endian.little);
  data.setUint32(24, sampleRate, Endian.little);
  data.setUint32(28, sampleRate * 2, Endian.little);
  data.setUint16(32, 2, Endian.little);
  data.setUint16(34, 16, Endian.little);
  ascii(36, 'data');
  data.setUint32(40, samples * 2, Endian.little);
  for (var i = 0; i < samples; i++) {
    final value = (3200 * math.sin(i * 440 * 2 * math.pi / sampleRate))
        .round();
    data.setInt16(44 + i * 2, value, Endian.little);
  }
  final file = File(
    '${Directory.systemTemp.path}${Platform.pathSeparator}wcmusic-test-tone.wav',
  );
  await file.writeAsBytes(data.buffer.asUint8List(), flush: true);
  return file.path;
}
