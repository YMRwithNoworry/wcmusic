import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../../data/services/player_service.dart';
import '../../../data/services/online_search_service.dart';
import '../../../domain/models/track.dart';
import '../../../domain/repositories/music_repository.dart';
import '../../../domain/repositories/source_repository.dart';

class PlayerViewModel extends ChangeNotifier {
  PlayerViewModel({
    required this.musicRepository,
    required this.sourceRepository,
    OnlineSearchService? onlineSearchService,
    AudioPlayerService? playerService,
  }) : onlineSearchService =
           onlineSearchService ?? AppleOnlineSearchService(),
       playerService = playerService ?? PlayerService() {
    _playingSubscription = this.playerService.playing.listen((value) {
      isPlaying = value;
      notifyListeners();
    });
    _positionSubscription = this.playerService.position.listen((value) {
      position = value;
      notifyListeners();
    });
  }

  final MusicRepository musicRepository;
  final SourceRepository sourceRepository;
  final OnlineSearchService onlineSearchService;
  final AudioPlayerService playerService;
  List<Track> tracks = const [];
  List<Playlist> playlists = const [];
  List<SourceScript> sources = const [];
  Track? current;
  bool isLoading = true;
  bool isPlaying = false;
  Duration position = Duration.zero;
  Duration buffered = Duration.zero;
  String? message;
  String query = '';
  String onlineQuery = '';
  List<Track> onlineResults = const [];
  bool isSearchingOnline = false;
  String? onlineSearchError;
  int _searchGeneration = 0;
  StreamSubscription<bool>? _playingSubscription;
  StreamSubscription<Duration>? _positionSubscription;

  Future<void> load() async {
    isLoading = true;
    notifyListeners();
    try {
      tracks = await musicRepository.loadTracks();
      playlists = await musicRepository.loadPlaylists();
      sources = await sourceRepository.loadSources();
    } on Object catch (error) {
      message = '载入音乐数据失败：$error';
    } finally {
      isLoading = false;
      notifyListeners();
    }
  }

  List<Track> get visibleTracks {
    final needle = query.trim().toLowerCase();
    if (needle.isEmpty) return tracks;
    return tracks
        .where(
          (track) => '${track.title} ${track.artist} ${track.album}'
              .toLowerCase()
              .contains(needle),
        )
        .toList();
  }

  Future<void> playTrack(Track track) async {
    current = track;
    message = track.uri.isEmpty
        ? track.source == TrackSource.local
              ? '这是演示曲目；导入本地歌单后即可播放'
              : '该歌曲暂无可用的在线试听地址'
        : null;
    if (track.uri.isNotEmpty) {
      await playerService.play(track);
      isPlaying = true;
    }
    notifyListeners();
  }

  Future<void> togglePlayback() async {
    if (current == null) return;
    if (current!.uri.isEmpty) {
      message = '演示曲目没有音频地址';
      notifyListeners();
      return;
    }
    await playerService.toggle();
    isPlaying = !isPlaying;
    notifyListeners();
  }

  Future<void> seek(double value) async {
    final target = Duration(milliseconds: value.round());
    position = target;
    await playerService.seek(target);
    notifyListeners();
  }

  void setQuery(String value) {
    query = value;
    notifyListeners();
  }

  Future<void> searchOnline(String value) async {
    final keyword = value.trim();
    onlineQuery = keyword;
    final generation = ++_searchGeneration;
    if (keyword.isEmpty) {
      onlineResults = const [];
      onlineSearchError = null;
      isSearchingOnline = false;
      notifyListeners();
      return;
    }

    isSearchingOnline = true;
    onlineSearchError = null;
    notifyListeners();
    try {
      final results = await onlineSearchService.search(keyword);
      if (generation != _searchGeneration) return;
      onlineResults = results;
    } on Object catch (error) {
      if (generation != _searchGeneration) return;
      onlineResults = const [];
      onlineSearchError = '搜索失败，请检查网络后重试：$error';
    } finally {
      if (generation == _searchGeneration) {
        isSearchingOnline = false;
        notifyListeners();
      }
    }
  }

  Future<void> importPlaylist(String path, List<int> bytes) async {
    try {
      await musicRepository.importFile(path, bytes);
      await load();
      message = '已导入歌单';
    } on FormatException catch (error) {
      message = error.message;
    } on Object catch (error) {
      message = '导入失败：$error';
    }
    notifyListeners();
  }

  Future<void> addTracks(List<Track> additions) async {
    if (additions.isEmpty) return;
    tracks = [...tracks, ...additions];
    await musicRepository.saveTracks(tracks);
    message = '已添加 ${additions.length} 首本地音乐';
    notifyListeners();
  }

  Future<void> reorderTracks(int oldIndex, int newIndex) async {
    final reordered = [...tracks];
    final track = reordered.removeAt(oldIndex);
    reordered.insert(newIndex, track);
    tracks = reordered;
    await musicRepository.saveTracks(tracks);
    notifyListeners();
  }

  Future<void> importSource(String rawScript) async {
    try {
      final source = await sourceRepository.importScript(rawScript);
      sources = await sourceRepository.loadSources();
      message = '已启用 ${source.name}';
    } on FormatException catch (error) {
      message = error.message;
    } on Object catch (error) {
      message = '导入失败：$error';
    }
    notifyListeners();
  }

  Future<void> deleteSource(String id) async {
    try {
      await sourceRepository.deleteSource(id);
      sources = await sourceRepository.loadSources();
      message = '已删除音源';
    } on Object catch (error) {
      message = '删除失败：$error';
    }
    notifyListeners();
  }

  @override
  void dispose() {
    unawaited(_playingSubscription?.cancel());
    unawaited(_positionSubscription?.cancel());
    unawaited(playerService.dispose());
    super.dispose();
  }
}
