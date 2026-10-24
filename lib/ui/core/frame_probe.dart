import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'package:flutter/foundation.dart';
import 'package:flutter/scheduler.dart';

/// 帧耗时探针。
///
/// 只在构建时通过 `--dart-define=WCMUSIC_FRAME_PROBE=true` 打开，用来量化
/// “动画是否流畅”：把 build / raster 耗时聚合成分位数，定期写到
/// `--dart-define=WCMUSIC_FRAME_LOG=<path>` 指定的 JSON 文件。
///
/// 正常发布构建不会注册任何回调，也不会碰文件系统。
class FrameProbe {
  FrameProbe._(this._file);

  /// 直接写入指定文件。集成测试没有走 app 的 main()，需要自己启动探针。
  factory FrameProbe.toFile(File file) = FrameProbe._;

  /// 从编译期常量创建；未开启时返回 null。
  static FrameProbe? fromEnvironment() {
    if (!const bool.fromEnvironment('WCMUSIC_FRAME_PROBE')) return null;
    final path = const String.fromEnvironment('WCMUSIC_FRAME_LOG');
    if (path.isEmpty) return null;
    return FrameProbe._(File(path));
  }

  /// 单个构建的判定阈值：60Hz 下一帧 16.7ms，超过就算掉帧。
  static const budget = Duration(microseconds: 16667);

  final File _file;
  final List<int> _build = [];
  final List<int> _raster = [];
  Timer? _flushTimer;
  bool _installed = false;
  int _overBudget = 0;
  int _windowWorstBuild = 0;
  int _windowWorstRaster = 0;
  int _worstBuild = 0;
  int _worstRaster = 0;

  void start() {
    if (_installed) return;
    _installed = true;
    SchedulerBinding.instance.addTimingsCallback(_onTimings);
    _flushTimer = Timer.periodic(const Duration(seconds: 2), (_) => flush());
    // 退出（含托盘退出）前把最后一批评分写出去。
    ProcessSignal.sigint.watch().listen((_) => flush());
  }

  void stop() {
    if (!_installed) return;
    _installed = false;
    _flushTimer?.cancel();
    SchedulerBinding.instance.removeTimingsCallback(_onTimings);
    flush();
  }

  void _onTimings(List<FrameTiming> timings) {
    for (final timing in timings) {
      final build = timing.buildDuration.inMicroseconds;
      final raster = timing.rasterDuration.inMicroseconds;
      _build.add(build);
      _raster.add(raster);
      if (build > budget.inMicroseconds ||
          raster > budget.inMicroseconds) {
        _overBudget++;
      }
      if (build > _windowWorstBuild) _windowWorstBuild = build;
      if (raster > _windowWorstRaster) _windowWorstRaster = raster;
    }
  }

  /// 把当前累积的样本写出去，并清零窗口最大值。
  Future<void> flush() async {
    if (_build.isEmpty) return;
    final build = [..._build];
    final raster = [..._raster];
    _build.clear();
    _raster.clear();
    int percentile(List<int> values, double p) {
      if (values.isEmpty) return 0;
      final sorted = [...values]..sort();
      final index = ((sorted.length - 1) * p).round();
      return sorted[index];
    }

    final summary = {
      'frames': build.length + _totalFrames,
      'sampleFrames': build.length,
      'fpsBudgetUs': budget.inMicroseconds,
      'buildUs': {
        'p50': percentile(build, .5),
        'p95': percentile(build, .95),
        'p99': percentile(build, .99),
        'max': _windowWorstBuild,
      },
      'rasterUs': {
        'p50': percentile(raster, .5),
        'p95': percentile(raster, .95),
        'p99': percentile(raster, .99),
        'max': _windowWorstRaster,
      },
      'overBudgetFrames': _overBudget + _totalOverBudget,
      'isProfile': kProfileMode,
      'timestamp': DateTime.now().toIso8601String(),
    };
    if (_windowWorstBuild > _worstBuild) _worstBuild = _windowWorstBuild;
    if (_windowWorstRaster > _worstRaster) _worstRaster = _windowWorstRaster;
    _windowWorstBuild = 0;
    _windowWorstRaster = 0;
    _totalFrames += build.length;
    _totalOverBudget += _overBudget;
    _overBudget = 0;
    lastSummary = summary;
    try {
      await _file.parent.create(recursive: true);
      await _file.writeAsString(jsonEncode(summary), flush: true);
    } on Object catch (error) {
      debugPrint('[frame-probe] 写入失败：$error');
    }
  }

  /// 最近一次 flush 的汇总，集成测试可以直接断言。
  Map<String, Object?>? lastSummary;

  int _totalFrames = 0;
  int _totalOverBudget = 0;
}
