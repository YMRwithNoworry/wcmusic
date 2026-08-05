import 'dart:typed_data';

import 'package:uuid/uuid.dart';

import '../../domain/models/track.dart';
import '../../domain/models/music_folder.dart';
import '../../domain/repositories/music_repository.dart';
import '../services/playlist_parser.dart';

class MemoryMusicRepository implements MusicRepository {
  MemoryMusicRepository({
    this._parser = const PlaylistParser(),
    List<Track> initialTracks = const [],
    List<Playlist> initialPlaylists = const [],
    List<MusicFolder> initialFolders = const [],
  }) : _tracks = List.of(initialTracks),
       _playlists = List.of(initialPlaylists),
       _folders = List.of(initialFolders);

  final PlaylistParser _parser;
  final List<Track> _tracks;
  final List<Playlist> _playlists;
  final List<MusicFolder> _folders;

  @override
  Future<List<Track>> loadTracks() async => List.unmodifiable(_tracks);

  @override
  Future<List<Playlist>> loadPlaylists() async => List.unmodifiable(_playlists);

  @override
  Future<List<MusicFolder>> loadFolders() async => List.unmodifiable(_folders);

  @override
  Future<MusicFolder> createFolder(String name) async {
    final folder = MusicFolder(
      id: const Uuid().v4(),
      name: name.trim().isEmpty ? '新建文件夹' : name.trim(),
      trackIds: const [],
    );
    _folders.add(folder);
    return folder;
  }

  @override
  Future<void> deleteFolder(String id) async {
    _folders.removeWhere((folder) => folder.id == id);
  }

  @override
  Future<void> addTrackToFolder(String folderId, String trackId) async {
    final index = _folders.indexWhere((folder) => folder.id == folderId);
    if (index < 0) throw StateError('没有找到要收藏的文件夹');
    final folder = _folders[index];
    if (folder.trackIds.contains(trackId)) return;
    _folders[index] = folder.copyWith(trackIds: [...folder.trackIds, trackId]);
  }

  @override
  Future<void> removeTrackFromFolder(String folderId, String trackId) async {
    final index = _folders.indexWhere((folder) => folder.id == folderId);
    if (index < 0) return;
    final folder = _folders[index];
    _folders[index] = folder.copyWith(
      trackIds: folder.trackIds.where((id) => id != trackId).toList(),
    );
  }

  @override
  Future<void> saveTracks(List<Track> tracks) async {
    _tracks
      ..clear()
      ..addAll(tracks);
  }

  @override
  Future<void> savePlaylist(Playlist playlist) async {
    _playlists.removeWhere((item) => item.id == playlist.id);
    _playlists.add(playlist);
  }

  @override
  Future<void> deletePlaylist(String id) async {
    _playlists.removeWhere((item) => item.id == id);
  }

  @override
  Future<void> importFile(String path, List<int> bytes) async {
    final playlist = _parser.parseM3u(
      Uint8List.fromList(bytes),
      _basename(path),
    );
    _playlists.add(playlist);
    _tracks.addAll(playlist.tracks);
  }

  @override
  Future<List<Track>> importAudioFiles(List<String> paths) async {
    final imported = paths
        .where((path) => path.isNotEmpty)
        .map(
          (path) => Track(
            id: const Uuid().v4(),
            title: _basename(path).replaceFirst(RegExp(r'\.[^.]+$'), ''),
            artist: '本地音乐',
            album: '最近添加',
            duration: Duration.zero,
            uri: path,
          ),
        )
        .toList(growable: false);
    _tracks.addAll(imported);
    return imported;
  }

  String _basename(String value) => value.replaceAll('\\', '/').split('/').last;
}
