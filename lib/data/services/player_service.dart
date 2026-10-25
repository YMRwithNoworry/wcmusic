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
      // 不兼容的音频滤镜不能让整首歌无法播放，清空滤镜并重新打开当前媒体。
      if (_isAudioFilterError(error)) {
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
  Track? _currentTrack;
  Track? _networkTrack;
  bool _retriedWithoutProxy = false;
  bool _recoveringFromAudioFilterError = false;

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
  Stream<String> get errors =>
      player.stream.error.where((error) => !_isAudioFilterError(error));

  bool _isAudioFilterError(String error) => error.contains('Audio filter');

  /// 与 [play] 相同的 UA，恢复播放时保持一致的请求头。
  static const _userAgent =
      'Mozilla/5.0 (Windows NT 10.0; Win64; x64) '
      'AppleWebKit/537.36 Chrome/126.0 Safari/537.36';

  @override
  Future<void> play(Track track) async {
    if (track.uri.isEmpty) return;
    _currentTrack = track;
    _networkTrack = _isNetworkTrack(track) ? track : null;
    _retriedWithoutProxy = false;
    await _configureNetworkPlayback();
    // mpv 在 open 时会重建滤镜图，因此只在媒体后应用当前设置。
    await player.open(
      Media(track.uri, httpHeaders: const {'User-Agent': _userAgent}),
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
  /// 同理 aecho、pan、earwax 也都没编进 media_kit 的 Windows 构建，唯一随包
  /// 启用的音频滤镜是 FFmpeg 的 equalizer（见 libmpv 的 configure 参数
  /// `--enable-filter=equalizer --enable-filter=overlay`），因此只能用它做频段
  /// 塑形：中高频 3kHz 提 2dB 让声像更贴耳，8kHz 略降 1dB 压掉刺耳齿音。
  static const _spatialFilter =
      'lavfi=[equalizer=f=3000:t=q:w=1:g=2,equalizer=f=8000:t=q:w=1:g=-1]';

  @override
  Future<void> setSpatialAudioEnabled(bool enabled) async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.future;
    _spatialEnabled = enabled;
    if (!enabled) {
      await platform.setProperty('af', '');
      return;
    }
    // 首次启动时还没有音频轨，不能提前初始化 af；等 play/open 完成后再挂载。
    if (player.state.playlist.medias.isEmpty) return;
    await _applyAudioFilters();
  }

  bool _spatialEnabled = false;

  /// open 之后重新挂上音效滤镜。
  ///
  /// 必须放在 open 之后：mpv 打开新媒体时会重建滤镜图，open 前设置的 af
  /// 会被清空（实测切音质后 af 变成空值，空间音效静默失效）。
  ///
  /// 注意 `Player.setProperty` 只调 `mpv_set_property_string` 且丢弃返回值，
  /// 滤镜名写错不会抛异常，而是通过 errors 流报 “Audio filter initialized
  /// failed!”；真正的回退由 [_disableSpatialAndRetry] 负责。
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

  Future<void> _disableSpatialAndRetry() async {
    // mpv 可能连续上报多条滤镜错误，只处理第一条。
    if (_recoveringFromAudioFilterError) return;
    _recoveringFromAudioFilterError = true;
    _spatialEnabled = false;
    try {
      final platform = player.platform;
      if (platform is! NativePlayer) return;
      await platform.future;
      await platform.setProperty('af', '');
      // 滤镜初始化失败时 mpv 已经停掉了音频输出，必须重新打开才能出声。
      final track = _currentTrack;
      if (track != null && track.uri.isNotEmpty) {
        await player.open(
          Media(track.uri, httpHeaders: const {'User-Agent': _userAgent}),
          play: true,
        );
      }
    } on Object {
      // 已经尽力；恢复失败时保持无滤镜状态，不再向界面抛滤镜错误。
    } finally {
      _recoveringFromAudioFilterError = false;
    }
  }

  Future<void> _retryWithoutProxy(Track track) async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.setProperty('http-proxy', '');
    if (_networkTrack?.id != track.id) return;
    await player.open(
      Media(track.uri, httpHeaders: const {'User-Agent': _userAgent}),
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
