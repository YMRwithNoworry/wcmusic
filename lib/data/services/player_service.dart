import 'package:media_kit/media_kit.dart' hide Track;

import '../../domain/models/track.dart';

abstract interface class AudioPlayerService {
  Stream<bool> get playing;
  Stream<Duration> get position;
  Stream<Duration> get duration;
  Stream<double> get volume;
  Future<void> play(Track track);
  Future<void> toggle();
  Future<void> seek(Duration position);
  Future<void> setVolume(double volume);
  Future<void> dispose();
}

class PlayerService implements AudioPlayerService {
  PlayerService() : player = Player();

  final Player player;

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
  Future<void> play(Track track) async {
    if (track.uri.isEmpty) return;
    await player.open(Media(track.uri), play: true);
  }

  @override
  Future<void> toggle() => player.playOrPause();

  @override
  Future<void> seek(Duration position) => player.seek(position);

  @override
  Future<void> setVolume(double volume) =>
      player.setVolume(volume.clamp(0.0, 1.0) * 100);

  @override
  Future<void> dispose() async {
    await player.dispose();
  }
}
