import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:path/path.dart' as path;
import 'package:path_provider/path_provider.dart';
import 'package:uuid/uuid.dart';

import '../../domain/models/track.dart';
import '../../domain/repositories/music_repository.dart';
import '../services/playlist_parser.dart';

class FileMusicRepository implements MusicRepository {
  FileMusicRepository({
    Future<Directory> Function()? directoryProvider,
    this._parser = const PlaylistParser(),
  }) : _directoryProvider = directoryProvider ?? getApplicationSupportDirectory;

  final Future<Directory> Function() _directoryProvider;
  final PlaylistParser _parser;
  final List<Track> _tracks = [];
  final List<Playlist> _playlists = [];
  bool _loaded = false;

  Future<Directory> _rootDirectory() async {
    final directory = await _directoryProvider();
    if (!await directory.exists()) await directory.create(recursive: true);
    return directory;
  }

  Future<Directory> _musicDirectory() async {
    final root = await _rootDirectory();
    final directory = Directory(path.join(root.path, 'music'));
    if (!await directory.exists()) await directory.create(recursive: true);
    return directory;
  }

  Future<File> _libraryFile() async {
    final root = await _rootDirectory();
    return File(path.join(root.path, 'library.json'));
  }

  Future<void> _ensureLoaded() async {
    if (_loaded) return;
    final file = await _libraryFile();
    if (await file.exists()) {
      final decoded = jsonDecode(await file.readAsString());
      if (decoded is! Map) {
        throw const FormatException('曲库存储文件格式无效');
      }
      final data = Map<String, dynamic>.from(decoded);
      final tracks = data['tracks'];
      final playlists = data['playlists'];
      if (tracks is List) {
        _tracks.addAll(
          tracks.whereType<Map>().map(
            (item) => _trackFromJson(Map<String, dynamic>.from(item)),
          ),
        );
      }
      if (playlists is List) {
        _playlists.addAll(
          playlists.whereType<Map>().map(
            (item) => _playlistFromJson(Map<String, dynamic>.from(item)),
          ),
        );
      }
    }
    _loaded = true;
  }

  @override
  Future<List<Track>> loadTracks() async {
    await _ensureLoaded();
    return List.unmodifiable(_tracks);
  }

  @override
  Future<List<Playlist>> loadPlaylists() async {
    await _ensureLoaded();
    return List.unmodifiable(_playlists);
  }

  @override
  Future<void> saveTracks(List<Track> tracks) async {
    await _ensureLoaded();
    _tracks
      ..clear()
      ..addAll(tracks);
    await _persist();
  }

  @override
  Future<void> savePlaylist(Playlist playlist) async {
    await _ensureLoaded();
    _playlists.removeWhere((item) => item.id == playlist.id);
    _playlists.add(playlist);
    await _persist();
  }

  @override
  Future<void> deletePlaylist(String id) async {
    await _ensureLoaded();
    _playlists.removeWhere((item) => item.id == id);
    await _persist();
  }

  @override
  Future<List<Track>> importAudioFiles(List<String> paths) async {
    await _ensureLoaded();
    final imported = <Track>[];
    final copiedFiles = <File>[];
    try {
      for (final sourcePath in paths.where((value) => value.isNotEmpty)) {
        final source = File(sourcePath);
        if (!await source.exists()) {
          throw FileSystemException('找不到要导入的音频', sourcePath);
        }
        final track = await _copyTrack(
          Track(
            id: const Uuid().v4(),
            title: path.basenameWithoutExtension(sourcePath),
            artist: '本地音乐',
            album: '最近添加',
            duration: Duration.zero,
            uri: sourcePath,
          ),
          source,
        );
        imported.add(track);
        copiedFiles.add(File(track.uri));
      }
      _tracks.addAll(imported);
      await _persist();
      return List.unmodifiable(imported);
    } on Object {
      for (final file in copiedFiles) {
        if (await file.exists()) await file.delete();
      }
      rethrow;
    }
  }

  @override
  Future<void> importFile(String playlistPath, List<int> bytes) async {
    await _ensureLoaded();
    final parsed = _parser.parseM3u(
      Uint8List.fromList(bytes),
      path.basename(playlistPath),
    );
    final importedTracks = <Track>[];
    final baseDirectory = path.dirname(playlistPath);
    for (final track in parsed.tracks) {
      if (_isNetworkUri(track.uri)) {
        importedTracks.add(track);
        continue;
      }
      final sourcePath = path.isAbsolute(track.uri)
          ? track.uri
          : path.normalize(path.join(baseDirectory, track.uri));
      final source = File(sourcePath);
      importedTracks.add(
        await source.exists() ? await _copyTrack(track, source) : track,
      );
    }
    final playlist = Playlist(
      id: parsed.id,
      name: parsed.name,
      tracks: importedTracks,
    );
    _playlists.add(playlist);
    _tracks.addAll(importedTracks);
    await _persist();
  }

  Future<Track> _copyTrack(Track track, File source) async {
    final directory = await _musicDirectory();
    final extension = path.extension(source.path).toLowerCase();
    final destination = File(
      path.join(directory.path, '${const Uuid().v4()}$extension'),
    );
    await source.copy(destination.path);
    return Track(
      id: track.id,
      title: track.title,
      artist: track.artist,
      album: track.album,
      duration: track.duration,
      uri: destination.path,
      artworkUri: track.artworkUri,
      source: TrackSource.local,
      sourceId: track.sourceId,
      quality: track.quality,
      releaseDate: track.releaseDate,
      isFavorite: track.isFavorite,
    );
  }

  bool _isNetworkUri(String value) {
    final scheme = Uri.tryParse(value)?.scheme.toLowerCase();
    return scheme == 'http' || scheme == 'https';
  }

  Future<void> _persist() async {
    final file = await _libraryFile();
    await file.writeAsString(
      jsonEncode({
        'version': 1,
        'tracks': _tracks.map(_trackToJson).toList(growable: false),
        'playlists': _playlists.map(_playlistToJson).toList(growable: false),
      }),
      flush: true,
    );
  }

  Map<String, dynamic> _trackToJson(Track track) => {
    'id': track.id,
    'title': track.title,
    'artist': track.artist,
    'album': track.album,
    'durationMs': track.duration.inMilliseconds,
    'uri': track.uri,
    'artworkUri': track.artworkUri,
    'source': track.source.name,
    'sourceId': track.sourceId,
    'quality': track.quality,
    'releaseDate': track.releaseDate?.toIso8601String(),
    'isFavorite': track.isFavorite,
  };

  Track _trackFromJson(Map<String, dynamic> data) {
    final sourceName = data['source']?.toString();
    final source = TrackSource.values.where((item) => item.name == sourceName);
    return Track(
      id: data['id']?.toString() ?? const Uuid().v4(),
      title: data['title']?.toString() ?? '未命名曲目',
      artist: data['artist']?.toString() ?? '',
      album: data['album']?.toString() ?? '',
      duration: Duration(
        milliseconds: (data['durationMs'] as num?)?.round() ?? 0,
      ),
      uri: data['uri']?.toString() ?? '',
      artworkUri: data['artworkUri']?.toString(),
      source: source.isEmpty ? TrackSource.local : source.first,
      sourceId: data['sourceId']?.toString(),
      quality: data['quality']?.toString(),
      releaseDate: DateTime.tryParse(data['releaseDate']?.toString() ?? ''),
      isFavorite: data['isFavorite'] == true,
    );
  }

  Map<String, dynamic> _playlistToJson(Playlist playlist) => {
    'id': playlist.id,
    'name': playlist.name,
    'tracks': playlist.tracks.map(_trackToJson).toList(growable: false),
    'artworkUri': playlist.artworkUri,
    'externalUrl': playlist.externalUrl,
    'platform': playlist.platform,
  };

  Playlist _playlistFromJson(Map<String, dynamic> data) {
    final tracks = data['tracks'];
    return Playlist(
      id: data['id']?.toString() ?? const Uuid().v4(),
      name: data['name']?.toString() ?? '未命名歌单',
      artworkUri: data['artworkUri']?.toString(),
      externalUrl: data['externalUrl']?.toString(),
      platform: data['platform']?.toString(),
      tracks: tracks is List
          ? tracks
                .whereType<Map>()
                .map((item) => _trackFromJson(Map<String, dynamic>.from(item)))
                .toList(growable: false)
          : const [],
    );
  }
}
