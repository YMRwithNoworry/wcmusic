class MusicFolder {
  const MusicFolder({
    required this.id,
    required this.name,
    required this.trackIds,
  });

  final String id;
  final String name;
  final List<String> trackIds;

  MusicFolder copyWith({String? name, List<String>? trackIds}) => MusicFolder(
    id: id,
    name: name ?? this.name,
    trackIds: trackIds ?? this.trackIds,
  );

  Map<String, dynamic> toJson() => {
    'id': id,
    'name': name,
    'trackIds': trackIds,
  };

  factory MusicFolder.fromJson(Map<String, dynamic> json) => MusicFolder(
    id: json['id']?.toString() ?? '',
    name: json['name']?.toString() ?? '未命名文件夹',
    trackIds: (json['trackIds'] as List?)?.cast<String>() ?? const [],
  );
}
