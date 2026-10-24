import 'dart:async';

import 'package:flutter/foundation.dart';

import 'package:media_kit/media_kit.dart' hide Track;

import '../../domain/models/track.dart';

abstract interface class AudioPlayerService {
  Stream<bool> get playing;
  Stream<Duration> get position;
  Stream<Duration> get duration;
  Stream<double> get volume;
  Stream<void> get completed;
  Stream<String> get errors;
  Future<void> play(Track track);
  Future<void> toggle();
  Future<void> stop();
  Future<void> seek(Duration position);
  Future<void> setVolume(double volume);
  Future<void> setSpatialAudioEnabled(bool enabled);
  Future<void> dispose();
}

class PlayerService implements AudioPlayerService {
  PlayerService() : player = Player() {
    _errorSubscription = player.stream.error.listen((error) {
      // 空间音效滤镜不被当前 mpv 支持时，先卸掉滤镜重试一次，
      // 避免“开了空间音效就整首播不出来”。
      if (_spatialEnabled && error.contains('Audio filter')) {
        unawaited(_disableSpatialAndRetry());
        return;
      }
      final track = _networkTrack;
      if (track != null && !_retriedWithoutProxy) {
        _retriedWithoutProxy = true;
        unawaited(_retryWithoutProxy(track));
      }
    });
  }

  final Player player;
  late final StreamSubscription<String> _errorSubscription;
  Track? _networkTrack;
  bool _retriedWithoutProxy = false;

  @override
  Stream<bool> get playing => player.stream.playing;

  @override
  Stream<Duration> get position => player.stream.position;

  @override
  Stream<Duration> get duration => player.stream.duration;

  @override
  Stream<double> get volume => player.stream.volume.map(
    (value) => (value / 100).clamp(0.0, 1.0).toDouble(),
  );

  @override
  Stream<void> get completed => player.stream.completed;

  @override
  Stream<String> get errors => player.stream.error;

  @override
  Future<void> play(Track track) async {
    if (track.uri.isEmpty) return;
    _networkTrack = _isNetworkTrack(track) ? track : null;
    _retriedWithoutProxy = false;
    await _configureNetworkPlayback();
    // mpv 在 open 时重建滤镜图，之前设置的 af 会被清空；先清一遍，
    // 等 open 完成后再按当前设置重新挂上（见 _applyAudioFilters）。
    await _resetAudioFilters();
    await player.open(
      Media(
        track.uri,
        httpHeaders: const {
          'User-Agent':
              'Mozilla/5.0 (Windows NT 10.0; Win64; x64) '
              'AppleWebKit/537.36 Chrome/126.0 Safari/537.36',
        },
      ),
      play: true,
    );
    await _applyAudioFilters();
  }

  @override
  Future<void> toggle() => player.playOrPause();

  @override
  Future<void> stop() => player.stop();

  @override
  Future<void> seek(Duration position) => player.seek(position);

  @override
  Future<void> setVolume(double volume) =>
      player.setVolume(volume.clamp(0.0, 1.0) * 100);

  /// 空间音效的 mpv 滤镜链。
  ///
  /// 踩过的坑：之前写的是 `lavfi=[extrastereo=m=2.0:c=0]`。随包的 libmpv 里
  /// 并没有 extrastereo 这个滤镜，初始化直接失败并抛
  /// “Audio filter initialized failed!”，整首歌都放不出来；而切换音质会重新
  /// open 媒体，所以表现为“切完音质就播不了”。
  ///
  /// 现在走 libmpv 自带的 lavfi 桥，滤镜名与参数用 `lavfi=graph` 的形式传入
  /// （mpv 的 af 语法，不是 lavfi 的方括号写法）。extrastereo 用来拓宽声场，
  /// aecho 叠两路短延迟制造空间感。
  static const _spatialFilter =
      'lavfi=[extrastereo=m=1.8:c=0,aecho=0.8:0.85:12|22:0.22|0.14]';

  @override
  Future<void> setSpatialAudioEnabled(bool enabled) async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.future;
    _spatialEnabled = enabled;
    try {
      await platform.setProperty('af', enabled ? _spatialFilter : '');
    } on Object {
      // 滤镜链不被当前 mpv 支持时不能让播放失败，回退为无滤镜并回报错误。
      _spatialEnabled = false;
      await platform.setProperty('af', '');
      rethrow;
    }
  }

  bool _spatialEnabled = false;

  /// open 之后重新挂上音效滤镜。
  ///
  /// 必须放在 open 之后：mpv 打开新媒体时会重建滤镜图，open 前设置的 af
  /// 会被清空（实测切音质后 af 变成空值，空间音效静默失效）。
  Future<void> _applyAudioFilters() async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.future;
    if (!_spatialEnabled) return;
    try {
      await platform.setProperty('af', _spatialFilter);
    } on Object catch (error) {
      // 这个 libmpv 没编进对应滤镜时不能连累播放：退回无滤镜。
      _spatialEnabled = false;
      await platform.setProperty('af', '');
      debugPrint('[spatial-audio] 滤镜不可用，已回退：$error');
    }
  }

  Future<void> _resetAudioFilters() async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.future;
    await platform.setProperty('af', '');
  }

  Future<void> _disableSpatialAndRetry() async {
    final track = _networkTrack;
    _spatialEnabled = false;
    try {
      await _applyAudioFilters();
      if (track != null) await player.open(Media(track.uri), play: true);
    } on Object {
      // 已经尽力；错误会继续通过 errors 流上报。
    }
  }

  Future<void> _retryWithoutProxy(Track track) async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.setProperty('http-proxy', '');
    if (_networkTrack?.id != track.id) return;
    await player.open(
      Media(
        track.uri,
        httpHeaders: const {
          'User-Agent':
              'Mozilla/5.0 (Windows NT 10.0; Win64; x64) '
              'AppleWebKit/537.36 Chrome/126.0 Safari/537.36',
        },
      ),
      play: true,
    );
  }

  Future<void> _configureNetworkPlayback() async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.future;
    await platform.setProperty('http-proxy', '');
    await platform.setProperty('network-timeout', '15000');
    await platform.setProperty('user-agent', 'Mozilla/5.0');
  }

  bool _isNetworkTrack(Track track) {
    final scheme = Uri.tryParse(track.uri)?.scheme.toLowerCase();
    return scheme == 'http' || scheme == 'https';
  }

  @override
  Future<void> dispose() async {
    await _errorSubscription.cancel();
    await player.dispose();
  }
}
