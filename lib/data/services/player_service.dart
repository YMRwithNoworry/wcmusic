import 'dart:async';

import 'package:media_kit/media_kit.dart' hide Track;

import '../../domain/models/track.dart';

abstract interface class AudioPlayerService {
  Stream<bool> get playing;
  Stream<Duration> get position;
  Stream<Duration> get duration;
  Stream<double> get volume;
  Stream<void> get completed;
  Future<void> play(Track track);
  Future<void> toggle();
  Future<void> stop();
  Future<void> seek(Duration position);
  Future<void> setVolume(double volume);
  Future<void> dispose();
}

class PlayerService implements AudioPlayerService {
  PlayerService() : player = Player() {
    _errorSubscription = player.stream.error.listen((_) {
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
  Future<void> play(Track track) async {
    if (track.uri.isEmpty) return;
    _networkTrack = _isNetworkTrack(track) ? track : null;
    _retriedWithoutProxy = false;
    await player.open(Media(track.uri), play: true);
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

  Future<void> _retryWithoutProxy(Track track) async {
    final platform = player.platform;
    if (platform is! NativePlayer) return;
    await platform.setProperty('http-proxy', '');
    if (_networkTrack?.id != track.id) return;
    await player.open(Media(track.uri), play: true);
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
