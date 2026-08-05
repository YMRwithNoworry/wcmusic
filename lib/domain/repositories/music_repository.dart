import '../models/track.dart';
import '../models/music_folder.dart';

abstract interface class MusicRepository {
  Future<List<Track>> loadTracks();
  Future<List<Playlist>> loadPlaylists();
  Future<List<MusicFolder>> loadFolders();
  Future<MusicFolder> createFolder(String name);
  Future<void> deleteFolder(String id);
  Future<void> addTrackToFolder(String folderId, String trackId);
  Future<void> removeTrackFromFolder(String folderId, String trackId);
  Future<void> saveTracks(List<Track> tracks);
  Future<void> savePlaylist(Playlist playlist);
  Future<void> deletePlaylist(String id);
  Future<void> importFile(String path, List<int> bytes);
  Future<List<Track>> importAudioFiles(List<String> paths);
}
