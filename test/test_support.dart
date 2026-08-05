import 'package:wcmusic/data/services/player_service.dart';
import 'package:wcmusic/data/services/online_search_service.dart';
import 'package:wcmusic/data/services/desktop_window_service.dart';
import 'package:wcmusic/domain/models/track.dart';

const testLibraryTracks = <Track>[
  Track(
    id: 'seedling',
    title: 'Seedling',
    artist: 'North Field',
    album: 'The Quiet Orchard',
    duration: Duration(minutes: 3, seconds: 42),
    uri: '',
  ),
  Track(
    id: 'waterline',
    title: 'Waterline',
    artist: 'Mira Sol',
    album: 'Tidal Memory',
    duration: Duration(minutes: 4, seconds: 8),
    uri: '',
  ),
  Track(
    id: 'greenhouse',
    title: 'Greenhouse',
    artist: 'Fallow & Form',
    album: 'Soft Machinery',
    duration: Duration(minutes: 2, seconds: 58),
    uri: '',
  ),
  Track(
    id: 'hush',
    title: 'Hush Before Rain',
    artist: 'Arden Sleep',
    album: 'Weather Rooms',
    duration: Duration(minutes: 5, seconds: 12),
    uri: '',
  ),
  Track(
    id: 'moss',
    title: 'Moss on Stone',
    artist: 'Lumen Garden',
    album: 'Low Sun',
    duration: Duration(minutes: 3, seconds: 26),
    uri: '',
  ),
];

class FakePlayerService implements AudioPlayerService {
  Duration? soughtPosition;
  double? setVolumeValue;
  Track? playedTrack;

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
  Future<void> play(Track track) async {
    playedTrack = track;
  }

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
  FakeOnlineSearchService([
    this.results = const [],
    this.playlists = const [],
    this.newTracks = const [],
    this.playlistTracks = const {},
    this.matchedTrack,
  ]);

  final List<Track> results;
  final List<PlatformPlaylist> playlists;
  final List<Track> newTracks;
  final Map<String, List<Track>> playlistTracks;
  final Track? matchedTrack;
  OnlineSearchChannel? lastChannel;

  @override
  Future<List<PlatformPlaylist>> discoverPlaylists() async => playlists;

  @override
  Future<List<Track>> discoverNewTracks() async => newTracks;

  @override
  Future<List<Track>> discoverPlaylistTracks(PlatformPlaylist playlist) async =>
      playlistTracks[playlist.id] ?? const [];

  @override
  Future<Track?> matchTrackToSources(
    Track track,
    Set<String> sourceKeys,
  ) async => matchedTrack;

  @override
  Future<List<Track>> search(
    String query, {
    int limit = 30,
    OnlineSearchChannel channel = OnlineSearchChannel.kuwo,
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
