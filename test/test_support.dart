import 'package:wcmusic/data/services/player_service.dart';
import 'package:wcmusic/data/services/online_search_service.dart';
import 'package:wcmusic/domain/models/track.dart';

class FakePlayerService implements AudioPlayerService {
  @override
  Stream<bool> get playing => const Stream.empty();

  @override
  Stream<Duration> get position => const Stream.empty();

  @override
  Future<void> dispose() async {}

  @override
  Future<void> play(Track track) async {}

  @override
  Future<void> seek(Duration position) async {}

  @override
  Future<void> toggle() async {}
}

class FakeOnlineSearchService implements OnlineSearchService {
  FakeOnlineSearchService([this.results = const []]);

  final List<Track> results;

  @override
  Future<List<Track>> search(String query, {int limit = 30}) async => results;
}
