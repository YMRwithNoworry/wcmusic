import 'dart:convert';
import 'dart:io';

import 'package:json5/json5.dart';

import '../../domain/models/track.dart';

enum OnlineSearchChannel { kuwo, kugou, qqMusic, netease }

extension OnlineSearchChannelLabel on OnlineSearchChannel {
  String get label => switch (this) {
    OnlineSearchChannel.kuwo => '酷我音乐',
    OnlineSearchChannel.kugou => '酷狗音乐',
    OnlineSearchChannel.qqMusic => 'QQ 音乐',
    OnlineSearchChannel.netease => '网易云音乐',
  };
}

class PlatformRanking {
  const PlatformRanking({
    required this.id,
    required this.name,
    required this.channel,
    this.artworkUri,
  });

  final String id;
  final String name;
  final OnlineSearchChannel channel;
  final String? artworkUri;
}

abstract interface class OnlineSearchService {
  Future<List<Track>> search(
    String query, {
    int limit = 30,
    OnlineSearchChannel channel = OnlineSearchChannel.kuwo,
  });
  Future<List<PlatformPlaylist>> discoverPlaylists();
  Future<List<Track>> discoverNewTracks();
  Future<List<Track>> discoverPlaylistTracks(PlatformPlaylist playlist);
  Future<List<PlatformRanking>> loadRankings(OnlineSearchChannel channel);
  Future<List<Track>> loadRankingTracks(PlatformRanking ranking);
  Future<Track?> matchTrackToSources(Track track, Set<String> sourceKeys);
}

class MultiSourceOnlineSearchService implements OnlineSearchService {
  MultiSourceOnlineSearchService({
    HttpClient Function()? clientFactory,
    String Function(Uri)? proxyResolver,
    Uri? kuwoSearchEndpoint,
    Uri? kugouSearchEndpoint,
    Uri? qqSearchEndpoint,
    Uri? neteaseSearchEndpoint,
    Uri? playlistEndpoint,
    Uri? playlistDetailEndpoint,
    Uri? newTracksEndpoint,
    Uri? neteaseRankingsEndpoint,
    Uri? qqRankingsEndpoint,
    Uri? qqRankingTracksEndpoint,
    Uri? kugouRankingsEndpoint,
    Uri? kugouRankingTracksEndpoint,
    Uri? kuwoRankingTracksEndpoint,
  }) : _clientFactory = clientFactory ?? HttpClient.new,
       _proxyResolver =
           proxyResolver ??
           ((uri) => HttpClient.findProxyFromEnvironment(
             uri,
             environment: Platform.environment,
           )),
       _kuwoSearchEndpoint =
           kuwoSearchEndpoint ?? Uri.https('search.kuwo.cn', '/r.s'),
       _kugouSearchEndpoint =
           kugouSearchEndpoint ??
           Uri.https('songsearch.kugou.com', '/song_search_v2'),
       _qqSearchEndpoint =
           qqSearchEndpoint ??
           Uri.https('c.y.qq.com', '/soso/fcgi-bin/client_search_cp'),
       _neteaseSearchEndpoint =
           neteaseSearchEndpoint ??
           Uri.https('music.163.com', '/api/search/get'),
       _playlistEndpoint =
           playlistEndpoint ?? Uri.https('music.163.com', '/api/playlist/list'),
       _playlistDetailEndpoint =
           playlistDetailEndpoint ??
           Uri.https('music.163.com', '/api/playlist/detail'),
       _newTracksEndpoint =
           newTracksEndpoint ??
           Uri.https('music.163.com', '/api/discovery/new/songs'),
       _neteaseRankingsEndpoint =
           neteaseRankingsEndpoint ??
           Uri.https('music.163.com', '/api/toplist/detail'),
       _qqRankingsEndpoint =
           qqRankingsEndpoint ??
           Uri.https('c.y.qq.com', '/v8/fcg-bin/fcg_myqq_toplist.fcg'),
       _qqRankingTracksEndpoint =
           qqRankingTracksEndpoint ??
           Uri.https('c.y.qq.com', '/v8/fcg-bin/fcg_v8_toplist_cp.fcg'),
       _kugouRankingsEndpoint =
           kugouRankingsEndpoint ??
           Uri.http('mobilecdnbj.kugou.com', '/api/v3/rank/list'),
       _kugouRankingTracksEndpoint =
           kugouRankingTracksEndpoint ??
           Uri.http('mobilecdnbj.kugou.com', '/api/v3/rank/song'),
       _kuwoRankingTracksEndpoint =
           kuwoRankingTracksEndpoint ??
           Uri.http('kbangserver.kuwo.cn', '/ksong.s');

  final HttpClient Function() _clientFactory;
  final String Function(Uri) _proxyResolver;
  final Uri _kuwoSearchEndpoint;
  final Uri _kugouSearchEndpoint;
  final Uri _qqSearchEndpoint;
  final Uri _neteaseSearchEndpoint;
  final Uri _playlistEndpoint;
  final Uri _playlistDetailEndpoint;
  final Uri _newTracksEndpoint;
  final Uri _neteaseRankingsEndpoint;
  final Uri _qqRankingsEndpoint;
  final Uri _qqRankingTracksEndpoint;
  final Uri _kugouRankingsEndpoint;
  final Uri _kugouRankingTracksEndpoint;
  final Uri _kuwoRankingTracksEndpoint;

  @override
  Future<List<Track>> search(
    String query, {
    int limit = 30,
    OnlineSearchChannel channel = OnlineSearchChannel.kuwo,
  }) async {
    final keyword = query.trim();
    if (keyword.isEmpty) return const [];
    return switch (channel) {
      OnlineSearchChannel.kuwo => _searchKuwo(keyword, limit),
      OnlineSearchChannel.kugou => _searchKugou(keyword, limit),
      OnlineSearchChannel.qqMusic => _searchQq(keyword, limit),
      OnlineSearchChannel.netease => _searchNetease(keyword, limit),
    };
  }

  Future<List<Track>> _searchKuwo(String keyword, int limit) async {
    final uri = _kuwoSearchEndpoint.replace(
      queryParameters: {
        ..._kuwoSearchEndpoint.queryParameters,
        'all': keyword,
        'ft': 'music',
        'itemset': 'web_2013',
        'client': 'kt',
        'pn': '0',
        'rn': limit.clamp(1, 50).toString(),
        'rformat': 'json',
        'encoding': 'utf8',
      },
    );
    final decoded = await _getJson(uri, relaxed: true);
    if (decoded is! Map || decoded['abslist'] is! List) {
      throw const FormatException('酷我音乐返回了无法识别的数据');
    }
    final tracks = <Track>[];
    for (final value in decoded['abslist'] as List) {
      if (value is! Map) continue;
      final musicRid = _text(value['MUSICRID'] ?? value['musicrid']);
      final sourceId =
          _text(value['DC_TARGETID']) ??
          musicRid?.replaceFirst(RegExp(r'^MUSIC_'), '');
      final title = _cleanHtml(_text(value['SONGNAME'] ?? value['NAME']));
      final artist = _cleanHtml(_text(value['ARTIST']));
      if (sourceId == null || title == null || artist == null) continue;
      final durationSeconds = _integer(value['DURATION']);
      tracks.add(
        Track(
          id: 'kw-$sourceId',
          title: title,
          artist: artist,
          album: _cleanHtml(_text(value['ALBUM'])) ?? '单曲',
          duration: Duration(seconds: durationSeconds ?? 0),
          uri: '',
          artworkUri: _secureUrl(
            _text(value['web_albumpic_short'] ?? value['hts_MVPIC']),
          ),
          source: TrackSource.kw,
          sourceId: sourceId,
          quality: '酷我音乐 · 整曲',
        ),
      );
    }
    return tracks;
  }

  Future<List<Track>> _searchKugou(String keyword, int limit) async {
    final uri = _kugouSearchEndpoint.replace(
      queryParameters: {
        ..._kugouSearchEndpoint.queryParameters,
        'keyword': keyword,
        'page': '1',
        'pagesize': limit.clamp(1, 50).toString(),
        'platform': 'WebFilter',
      },
    );
    final decoded = await _getJson(uri);
    final data = decoded is Map ? decoded['data'] : null;
    final values = data is Map ? data['lists'] : null;
    if (values is! List) {
      throw const FormatException('酷狗音乐返回了无法识别的数据');
    }
    final tracks = <Track>[];
    for (final value in values) {
      if (value is! Map) continue;
      final sourceId = _text(value['FileHash'] ?? value['EMixSongID']);
      final title = _cleanHtml(_text(value['SongName']));
      final artist = _cleanHtml(_text(value['SingerName']));
      if (sourceId == null || title == null || artist == null) continue;
      tracks.add(
        Track(
          id: 'kg-$sourceId',
          title: title,
          artist: artist,
          album: _cleanHtml(_text(value['AlbumName'])) ?? '单曲',
          duration: Duration(seconds: _integer(value['Duration']) ?? 0),
          uri: '',
          artworkUri: _secureUrl(
            _text(value['Image'])?.replaceFirst('{size}', '400'),
          ),
          source: TrackSource.kg,
          sourceId: sourceId,
          quality: '酷狗音乐 · 整曲',
        ),
      );
    }
    return tracks;
  }

  Future<List<Track>> _searchQq(String keyword, int limit) async {
    final uri = _qqSearchEndpoint.replace(
      queryParameters: {
        ..._qqSearchEndpoint.queryParameters,
        'w': keyword,
        'p': '1',
        'n': limit.clamp(1, 50).toString(),
        'format': 'json',
        'new_json': '1',
      },
    );
    final decoded = await _getJson(uri);
    final data = decoded is Map ? decoded['data'] : null;
    final song = data is Map ? data['song'] : null;
    final values = song is Map ? song['list'] : null;
    if (values is! List) {
      throw const FormatException('QQ 音乐返回了无法识别的数据');
    }
    final tracks = <Track>[];
    for (final value in values) {
      if (value is! Map) continue;
      final sourceId = _text(value['songmid'] ?? value['mid']);
      final title = _text(value['songname'] ?? value['name'] ?? value['title']);
      final singers = value['singer'];
      final artist = singers is List
          ? singers
                .whereType<Map>()
                .map((item) => _text(item['name']))
                .whereType<String>()
                .join(' / ')
          : null;
      if (sourceId == null ||
          title == null ||
          artist == null ||
          artist.isEmpty) {
        continue;
      }
      final album = value['album'];
      final albumName = album is Map
          ? _text(album['name'] ?? album['title'])
          : _text(value['albumname']);
      final albumMid = album is Map
          ? _text(album['mid'])
          : _text(value['albummid']);
      tracks.add(
        Track(
          id: 'tx-$sourceId',
          title: title,
          artist: artist,
          album: albumName ?? '单曲',
          duration: Duration(
            seconds: _integer(value['interval'] ?? value['duration']) ?? 0,
          ),
          uri: '',
          artworkUri: albumMid == null
              ? null
              : 'https://y.gtimg.cn/music/photo_new/T002R300x300M000$albumMid.jpg',
          source: TrackSource.tx,
          sourceId: sourceId,
          quality: 'QQ 音乐 · 整曲',
        ),
      );
    }
    return tracks;
  }

  Future<List<Track>> _searchNetease(String keyword, int limit) async {
    final uri = _neteaseSearchEndpoint.replace(
      queryParameters: {
        ..._neteaseSearchEndpoint.queryParameters,
        's': keyword,
        'type': '1',
        'limit': limit.clamp(1, 50).toString(),
        'offset': '0',
      },
    );
    final decoded = await _getJson(uri);
    final result = decoded is Map ? decoded['result'] : null;
    final values = result is Map ? result['songs'] : null;
    if (values is! List) {
      throw const FormatException('网易云音乐返回了无法识别的数据');
    }
    return values
        .whereType<Map>()
        .map(_parseNeteaseTrack)
        .whereType<Track>()
        .toList(growable: false);
  }

  @override
  Future<List<PlatformPlaylist>> discoverPlaylists() async {
    final uri = _playlistEndpoint.replace(
      queryParameters: {
        ..._playlistEndpoint.queryParameters,
        'cat': '全部',
        'order': 'hot',
        'limit': '20',
        'offset': '0',
      },
    );
    final decoded = await _getJson(uri);
    final values = decoded is Map ? decoded['playlists'] : null;
    if (values is! List) {
      throw const FormatException('网易云热门歌单返回了无法识别的数据');
    }
    final playlists = <PlatformPlaylist>[];
    for (final value in values) {
      if (value is! Map) continue;
      final id = value['id']?.toString();
      final name = _text(value['name']);
      final artwork = _text(value['coverImgUrl'] ?? value['picUrl']);
      if (id == null || name == null || artwork == null) continue;
      playlists.add(
        PlatformPlaylist(
          id: id,
          name: name,
          artworkUri: _secureUrl(artwork)!,
          url: 'https://music.163.com/#/playlist?id=$id',
          platform: '网易云音乐',
        ),
      );
    }
    return playlists;
  }

  @override
  Future<List<Track>> discoverPlaylistTracks(PlatformPlaylist playlist) async {
    final uri = _playlistDetailEndpoint.replace(
      queryParameters: {
        ..._playlistDetailEndpoint.queryParameters,
        'id': playlist.id,
      },
    );
    final decoded = await _getJson(uri);
    final result = decoded is Map ? decoded['result'] : null;
    final values = result is Map ? result['tracks'] : null;
    if (values is! List) {
      throw const FormatException('网易云歌单缺少曲目数据');
    }
    return values
        .whereType<Map>()
        .map(_parseNeteaseTrack)
        .whereType<Track>()
        .toList(growable: false);
  }

  @override
  Future<List<Track>> discoverNewTracks() async {
    final uri = _newTracksEndpoint.replace(
      queryParameters: {..._newTracksEndpoint.queryParameters, 'areaId': '0'},
    );
    final decoded = await _getJson(uri);
    final values = decoded is Map ? decoded['data'] : null;
    if (values is! List) {
      throw const FormatException('网易云新曲返回了无法识别的数据');
    }
    return values
        .whereType<Map>()
        .map(_parseNeteaseTrack)
        .whereType<Track>()
        .toList(growable: false);
  }

  @override
  Future<List<PlatformRanking>> loadRankings(
    OnlineSearchChannel channel,
  ) async {
    return switch (channel) {
      OnlineSearchChannel.netease => _loadNeteaseRankings(),
      OnlineSearchChannel.qqMusic => _loadQqRankings(),
      OnlineSearchChannel.kugou => _loadKugouRankings(),
      OnlineSearchChannel.kuwo => Future.value(_kuwoRankings),
    };
  }

  Future<List<PlatformRanking>> _loadNeteaseRankings() async {
    final decoded = await _getJson(_neteaseRankingsEndpoint);
    final values = decoded is Map ? decoded['list'] : null;
    if (values is! List) throw const FormatException('网易云榜单目录无法识别');
    return values
        .whereType<Map>()
        .map((value) {
          return PlatformRanking(
            id: value['id'].toString(),
            name: _text(value['name']) ?? '未命名榜单',
            channel: OnlineSearchChannel.netease,
            artworkUri: _secureUrl(_text(value['coverImgUrl'])),
          );
        })
        .toList(growable: false);
  }

  Future<List<PlatformRanking>> _loadQqRankings() async {
    final uri = _qqRankingsEndpoint.replace(
      queryParameters: {
        ..._qqRankingsEndpoint.queryParameters,
        'format': 'json',
        'inCharset': 'utf8',
        'outCharset': 'utf-8',
      },
    );
    final decoded = await _getJson(uri);
    final data = decoded is Map ? decoded['data'] : null;
    final values = data is Map ? data['topList'] : null;
    if (values is! List) throw const FormatException('QQ 音乐榜单目录无法识别');
    return values
        .whereType<Map>()
        .map((value) {
          return PlatformRanking(
            id: value['id'].toString(),
            name: _text(value['topTitle']) ?? '未命名榜单',
            channel: OnlineSearchChannel.qqMusic,
            artworkUri: _secureUrl(_text(value['picUrl'])),
          );
        })
        .toList(growable: false);
  }

  Future<List<PlatformRanking>> _loadKugouRankings() async {
    final uri = _kugouRankingsEndpoint.replace(
      queryParameters: {
        ..._kugouRankingsEndpoint.queryParameters,
        'version': '9108',
        'plat': '0',
        'showtype': '2',
        'parentid': '0',
        'apiver': '6',
        'area_code': '1',
      },
    );
    final decoded = await _getJson(uri);
    final data = decoded is Map ? decoded['data'] : null;
    final values = data is Map ? data['info'] : null;
    if (values is! List) throw const FormatException('酷狗音乐榜单目录无法识别');
    return values
        .whereType<Map>()
        .map((value) {
          final artwork = _text(value['imgurl'] ?? value['banner7url']);
          return PlatformRanking(
            id: value['rankid'].toString(),
            name: _text(value['rankname']) ?? '未命名榜单',
            channel: OnlineSearchChannel.kugou,
            artworkUri: _secureUrl(artwork?.replaceFirst('{size}', '400')),
          );
        })
        .toList(growable: false);
  }

  @override
  Future<List<Track>> loadRankingTracks(PlatformRanking ranking) async {
    return switch (ranking.channel) {
      OnlineSearchChannel.netease => _loadNeteaseRankingTracks(ranking),
      OnlineSearchChannel.qqMusic => _loadQqRankingTracks(ranking),
      OnlineSearchChannel.kugou => _loadKugouRankingTracks(ranking),
      OnlineSearchChannel.kuwo => _loadKuwoRankingTracks(ranking),
    };
  }

  Future<List<Track>> _loadNeteaseRankingTracks(PlatformRanking ranking) async {
    final uri = _playlistDetailEndpoint.replace(
      queryParameters: {
        ..._playlistDetailEndpoint.queryParameters,
        'id': ranking.id,
      },
    );
    final decoded = await _getJson(uri);
    final result = decoded is Map ? decoded['result'] : null;
    final values = result is Map ? result['tracks'] : null;
    if (values is! List) throw const FormatException('网易云榜单缺少歌曲');
    return values
        .whereType<Map>()
        .map(_parseNeteaseTrack)
        .whereType<Track>()
        .take(100)
        .toList(growable: false);
  }

  Future<List<Track>> _loadQqRankingTracks(PlatformRanking ranking) async {
    final uri = _qqRankingTracksEndpoint.replace(
      queryParameters: {
        ..._qqRankingTracksEndpoint.queryParameters,
        'format': 'json',
        'topid': ranking.id,
        'page': 'detail',
        'type': 'top',
        'song_begin': '0',
        'song_num': '100',
      },
    );
    final decoded = await _getJson(uri);
    final values = decoded is Map ? decoded['songlist'] : null;
    if (values is! List) throw const FormatException('QQ 音乐榜单缺少歌曲');
    return values
        .whereType<Map>()
        .map((value) => value['data'])
        .whereType<Map>()
        .map(_parseQqRankingTrack)
        .whereType<Track>()
        .toList(growable: false);
  }

  Future<List<Track>> _loadKugouRankingTracks(PlatformRanking ranking) async {
    final uri = _kugouRankingTracksEndpoint.replace(
      queryParameters: {
        ..._kugouRankingTracksEndpoint.queryParameters,
        'rankid': ranking.id,
        'page': '1',
        'pagesize': '100',
        'version': '9108',
        'plat': '0',
      },
    );
    final decoded = await _getJson(uri);
    final data = decoded is Map ? decoded['data'] : null;
    final values = data is Map ? data['info'] : null;
    if (values is! List) throw const FormatException('酷狗音乐榜单缺少歌曲');
    return values
        .whereType<Map>()
        .map(_parseKugouRankingTrack)
        .whereType<Track>()
        .toList(growable: false);
  }

  Future<List<Track>> _loadKuwoRankingTracks(PlatformRanking ranking) async {
    final uri = _kuwoRankingTracksEndpoint.replace(
      queryParameters: {
        ..._kuwoRankingTracksEndpoint.queryParameters,
        'from': 'pc',
        'fmt': 'json',
        'type': 'bang',
        'data': 'content',
        'id': ranking.id,
        'pn': '0',
        'rn': '100',
      },
    );
    final decoded = await _getJson(uri);
    final values = decoded is Map ? decoded['musiclist'] : null;
    if (values is! List) throw const FormatException('酷我音乐榜单缺少歌曲');
    return values
        .whereType<Map>()
        .map(_parseKuwoRankingTrack)
        .whereType<Track>()
        .toList(growable: false);
  }

  static const _kuwoRankings = [
    PlatformRanking(id: '93', name: '酷我飙升榜', channel: OnlineSearchChannel.kuwo),
    PlatformRanking(id: '17', name: '酷我新歌榜', channel: OnlineSearchChannel.kuwo),
    PlatformRanking(id: '16', name: '酷我热歌榜', channel: OnlineSearchChannel.kuwo),
    PlatformRanking(
      id: '158',
      name: '抖音热歌榜',
      channel: OnlineSearchChannel.kuwo,
    ),
  ];

  @override
  Future<Track?> matchTrackToSources(
    Track track,
    Set<String> sourceKeys,
  ) async {
    if (!sourceKeys.contains('wy')) return null;
    final matches = await _searchNetease('${track.title} ${track.artist}', 10);
    final expectedTitle = _normalized(track.title);
    final expectedArtist = _normalized(track.artist);
    Track? best;
    var bestScore = 0;
    for (final match in matches) {
      final candidateTitle = _normalized(match.title);
      final candidateArtist = _normalized(match.artist);
      var score = 0;
      if (candidateTitle == expectedTitle) {
        score += 4;
      } else if (candidateTitle.contains(expectedTitle) ||
          expectedTitle.contains(candidateTitle)) {
        score += 2;
      }
      if (expectedArtist.isNotEmpty &&
          (candidateArtist.contains(expectedArtist) ||
              expectedArtist.contains(candidateArtist))) {
        score += 3;
      }
      if (score > bestScore) {
        bestScore = score;
        best = track.copyWith(
          source: TrackSource.wy,
          sourceId: match.sourceId,
          quality: '网易云音乐 · 整曲',
        );
      }
    }
    return bestScore >= 4 ? best : null;
  }

  Track? _parseQqRankingTrack(Map<dynamic, dynamic> value) {
    final sourceId = _text(value['songmid'] ?? value['mid']);
    final title = _text(value['songname'] ?? value['name']);
    final singers = value['singer'];
    final artist = singers is List
        ? singers
              .whereType<Map>()
              .map((item) => _text(item['name']))
              .whereType<String>()
              .join(' / ')
        : null;
    if (sourceId == null || title == null || artist == null || artist.isEmpty) {
      return null;
    }
    final albumMid = _text(value['albummid']);
    return Track(
      id: 'tx-$sourceId',
      title: title,
      artist: artist,
      album: _text(value['albumname']) ?? '单曲',
      duration: Duration(seconds: _integer(value['interval']) ?? 0),
      uri: '',
      artworkUri: albumMid == null
          ? null
          : 'https://y.gtimg.cn/music/photo_new/T002R300x300M000$albumMid.jpg',
      source: TrackSource.tx,
      sourceId: sourceId,
      quality: 'QQ 音乐 · 整曲',
    );
  }

  Track? _parseKugouRankingTrack(Map<dynamic, dynamic> value) {
    final sourceId = _text(value['hash'] ?? value['filename']);
    final title = _cleanHtml(_text(value['songname']));
    final authors = value['authors'];
    final artist = authors is List
        ? authors
              .whereType<Map>()
              .map((item) => _text(item['author_name']))
              .whereType<String>()
              .join(' / ')
        : _cleanHtml(_text(value['filename']))?.split(' - ').first;
    if (sourceId == null || title == null || artist == null || artist.isEmpty) {
      return null;
    }
    final artwork = _text(value['album_sizable_cover'] ?? value['album_img']);
    return Track(
      id: 'kg-$sourceId',
      title: title,
      artist: artist,
      album: _text(value['remark']) ?? '单曲',
      duration: Duration(seconds: _integer(value['duration']) ?? 0),
      uri: '',
      artworkUri: _secureUrl(artwork?.replaceFirst('{size}', '400')),
      source: TrackSource.kg,
      sourceId: sourceId,
      quality: '酷狗音乐 · 整曲',
    );
  }

  Track? _parseKuwoRankingTrack(Map<dynamic, dynamic> value) {
    final sourceId = _text(value['id'] ?? value['musicrid']);
    final title = _cleanHtml(_text(value['name'] ?? value['songname']));
    final artist = _cleanHtml(_text(value['artist']));
    if (sourceId == null || title == null || artist == null) return null;
    return Track(
      id: 'kw-$sourceId',
      title: title,
      artist: artist,
      album: _cleanHtml(_text(value['album'])) ?? '单曲',
      duration: Duration(seconds: _integer(value['duration']) ?? 0),
      uri: '',
      artworkUri: _secureUrl(
        _text(value['pic'] ?? value['web_albumpic_short']),
      ),
      source: TrackSource.kw,
      sourceId: sourceId,
      quality: '酷我音乐 · 整曲',
    );
  }

  Track? _parseNeteaseTrack(Map<dynamic, dynamic> value) {
    final id = value['id']?.toString();
    final title = _text(value['name']);
    final artistsValue = value['artists'] ?? value['ar'];
    final artist = artistsValue is List
        ? artistsValue
              .whereType<Map>()
              .map((item) => _text(item['name']))
              .whereType<String>()
              .join(' / ')
        : null;
    if (id == null || title == null || artist == null || artist.isEmpty) {
      return null;
    }
    final albumValue = value['album'] ?? value['al'];
    final album = albumValue is Map ? _text(albumValue['name']) : null;
    final artwork = albumValue is Map
        ? _text(albumValue['picUrl'] ?? albumValue['blurPicUrl'])
        : null;
    final durationMs = _integer(value['duration'] ?? value['dt']);
    final publishTime = albumValue is Map
        ? _integer(albumValue['publishTime'])
        : null;
    return Track(
      id: 'wy-$id',
      title: title,
      artist: artist,
      album: album ?? '单曲',
      duration: Duration(milliseconds: durationMs ?? 0),
      uri: '',
      artworkUri: _secureUrl(artwork),
      source: TrackSource.wy,
      sourceId: id,
      quality: '网易云音乐 · 整曲',
      releaseDate: publishTime == null
          ? null
          : DateTime.fromMillisecondsSinceEpoch(publishTime, isUtc: true),
    );
  }

  Future<dynamic> _getJson(Uri uri, {bool relaxed = false}) async {
    final proxy = _proxyResolver(uri);
    try {
      return await _getJsonOnce(uri, proxy, relaxed: relaxed);
    } on Object {
      if (proxy == 'DIRECT') rethrow;
      return _getJsonOnce(uri, 'DIRECT', relaxed: relaxed);
    }
  }

  Future<dynamic> _getJsonOnce(
    Uri uri,
    String proxy, {
    required bool relaxed,
  }) async {
    final client = _clientFactory()
      ..connectionTimeout = const Duration(seconds: 10)
      ..findProxy = (_) => proxy;
    try {
      final request = await client.getUrl(uri);
      request.headers.set(HttpHeaders.acceptHeader, 'application/json');
      request.headers.set(HttpHeaders.userAgentHeader, 'WCMusic/1.0');
      request.headers.set(HttpHeaders.refererHeader, _refererFor(uri));
      final response = await request.close().timeout(
        const Duration(seconds: 15),
      );
      if (response.statusCode != HttpStatus.ok) {
        await response.drain<void>();
        throw HttpException('在线服务返回 ${response.statusCode}', uri: uri);
      }
      final body = await utf8.decoder.bind(response).join();
      final normalized = body.replaceFirst('\ufeff', '');
      return relaxed ? json5Decode(normalized) : jsonDecode(normalized);
    } finally {
      client.close(force: true);
    }
  }

  String _refererFor(Uri uri) {
    if (uri.host.contains('qq.com')) return 'https://y.qq.com/';
    if (uri.host.contains('kuwo')) return 'https://www.kuwo.cn/';
    if (uri.host.contains('kugou')) return 'https://www.kugou.com/';
    return 'https://music.163.com/';
  }

  String? _text(Object? value) {
    if (value is! String) return null;
    final text = value.trim();
    return text.isEmpty ? null : text;
  }

  int? _integer(Object? value) {
    if (value is num) return value.round();
    return int.tryParse(value?.toString() ?? '');
  }

  String? _cleanHtml(String? value) => value
      ?.replaceAll(RegExp(r'<[^>]+>'), '')
      .replaceAll('&amp;', '&')
      .replaceAll('&nbsp;', ' ')
      .replaceAll('&lt;', '<')
      .replaceAll('&gt;', '>')
      .replaceAll('&quot;', '"')
      .replaceAll('&#39;', "'")
      .trim();

  String? _secureUrl(String? value) =>
      value?.replaceFirst('http://', 'https://');

  String _normalized(String value) =>
      value.toLowerCase().replaceAll(RegExp(r'[\s\-_.,·•()\[\]{}]'), '');
}
