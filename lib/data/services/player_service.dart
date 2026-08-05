import 'package:media_kit/media_kit.dart' hide Track;

import '../../domain/models/track.dart';

abstract interface class AudioPlayerService {
  Stream<bool> get playing;
  Stream<Duration> get position;
  Future<void> play(Track track);
  Future<void> toggle();
  Future<void> seek(Duration position);
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
  Future<void> play(Track track) async {
    if (track.uri.isEmpty) return;
    await player.open(Media(track.uri), play: true);
  }

  @override
  Future<void> toggle() => player.playOrPause();

  @override
  Future<void> seek(Duration position) => player.seek(position);

  @override
  Future<void> dispose() async {
    await player.dispose();
  }
}
