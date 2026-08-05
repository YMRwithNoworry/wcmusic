import 'dart:typed_data';

import '../../domain/models/track.dart';
import '../../domain/repositories/music_repository.dart';
import '../services/playlist_parser.dart';

class MemoryMusicRepository implements MusicRepository {
  MemoryMusicRepository({this._parser = const PlaylistParser()});

  final PlaylistParser _parser;
  final List<Track> _tracks = List.of(_seedTracks);
  final List<Playlist> _playlists = [
    Playlist(id: 'morning', name: '晨光漫游', tracks: _seedTracks.take(3).toList()),
  ];

  @override
  Future<List<Track>> loadTracks() async => List.unmodifiable(_tracks);

  @override
  Future<List<Playlist>> loadPlaylists() async => List.unmodifiable(_playlists);

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
  Future<void> importFile(String path, List<int> bytes) async {
    final playlist = _parser.parseM3u(
      Uint8List.fromList(bytes),
      _basename(path),
    );
    _playlists.add(playlist);
    _tracks.addAll(playlist.tracks);
  }

  String _basename(String value) => value.replaceAll('\\', '/').split('/').last;
}

const _seedTracks = <Track>[
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
