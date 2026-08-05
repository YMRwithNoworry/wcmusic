import 'package:wcmusic/data/services/player_service.dart';
import 'package:wcmusic/data/services/online_search_service.dart';
import 'package:wcmusic/data/services/desktop_window_service.dart';
import 'package:wcmusic/domain/models/track.dart';

class FakePlayerService implements AudioPlayerService {
  Duration? soughtPosition;
  double? setVolumeValue;

  @override
  Stream<Duration> get duration => const Stream.empty();

  @override
  Stream<bool> get playing => const Stream.empty();

  @override
  Stream<Duration> get position => const Stream.empty();

  @override
  Stream<double> get volume => const Stream.empty();

  @override
  Future<void> dispose() async {}

  @override
  Future<void> play(Track track) async {}

  @override
  Future<void> seek(Duration position) async {
    soughtPosition = position;
  }

  @override
  Future<void> setVolume(double volume) async {
    setVolumeValue = volume;
  }

  @override
  Future<void> toggle() async {}
}

class FakeOnlineSearchService implements OnlineSearchService {
  FakeOnlineSearchService([this.results = const [], this.playlists = const []]);

  final List<Track> results;
  final List<PlatformPlaylist> playlists;
  OnlineSearchChannel? lastChannel;

  @override
  Future<List<PlatformPlaylist>> discoverPlaylists() async => playlists;

  @override
  Future<List<Track>> search(
    String query, {
    int limit = 30,
    OnlineSearchChannel channel = OnlineSearchChannel.appleMusic,
  }) async {
    lastChannel = channel;
    return results;
  }
}

class FakeWindowLifecycleService implements WindowLifecycleService {
  FakeWindowLifecycleService({this.closeToTray = true});

  @override
  bool closeToTray;

  @override
  Future<void> setCloseToTray(bool value) async {
    closeToTray = value;
  }
}
