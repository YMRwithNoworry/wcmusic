import '../models/track.dart';

abstract interface class MusicRepository {
  Future<List<Track>> loadTracks();
  Future<List<Playlist>> loadPlaylists();
  Future<void> saveTracks(List<Track> tracks);
  Future<void> savePlaylist(Playlist playlist);
  Future<void> importFile(String path, List<int> bytes);
}
