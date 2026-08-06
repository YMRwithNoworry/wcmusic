# 封面加载调试指南

## 问题：首页歌单显示有机封面而不是真实封面

### 可能原因

1. **网易云 API 返回的封面 URL 为空**
2. **RemoteArtwork 加载失败但回退到有机封面**
3. **使用旧版本应用**（需要重新构建）
4. **网络请求被防火墙拦截**

---

## 调试步骤

### 1. 添加调试日志

在 `lib/data/services/online_search_service.dart` 的 `discoverPlaylists` 方法中添加日志：

```dart
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
  print('🎵 歌单 API 响应: ${decoded.runtimeType}');
  
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
    
    // 添加调试日志
    print('📀 歌单: $name');
    print('   ID: $id');
    print('   封面 URL: $artwork');
    
    if (id == null || name == null || artwork == null) {
      print('   ❌ 跳过：缺少必要字段');
      continue;
    }
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
  print('✅ 成功加载 ${playlists.length} 个歌单');
  return playlists;
}
```

### 2. 在 RemoteArtwork 中添加日志

在 `lib/ui/core/remote_artwork.dart` 中：

```dart
Future<Uint8List> _load(String url) {
  print('🖼️  开始加载封面: $url');
  final cached = _cache[url];
  if (cached != null) {
    print('   ✅ 使用缓存');
    return cached;
  }
  if (_cache.length >= 64) _cache.remove(_cache.keys.first);
  late final Future<Uint8List> tracked;
  tracked = _loader
      .load(url)
      .then(
        (bytes) {
          print('   ✅ 加载成功: ${bytes.length} 字节');
          return bytes;
        },
        onError: (Object error, StackTrace stackTrace) {
          print('   ❌ 加载失败: $error');
          if (identical(_cache[url], tracked)) _cache.remove(url);
          Error.throwWithStackTrace(error, stackTrace);
        },
      );
  _cache[url] = tracked;
  return tracked;
}
```

### 3. 运行应用并查看日志

```bash
flutter run -d windows --verbose
```

查看控制台输出，检查：
- 歌单 API 是否返回了封面 URL
- 封面 URL 格式是否正确
- 封面加载是否成功

---

## 常见问题

### 问题 1：API 返回了封面 URL 但加载失败

**可能原因**：网易云防盗链

**解决方案**：已经在代码中处理，RemoteArtwork 会自动添加正确的请求头：
- User-Agent: Mozilla/5.0
- Referer: https://music.163.com/

### 问题 2：所有歌单都显示有机封面

**可能原因**：使用了旧版本应用

**解决方案**：
```bash
# 删除旧构建
rm -rf build/windows

# 重新构建
flutter build windows --release

# 运行新版本
build\windows\x64\runner\Release\wcmusic.exe
```

### 问题 3：部分歌单有封面，部分没有

**可能原因**：网易云 API 返回的部分歌单没有封面 URL

**解决方案**：这是正常的，OrganicArtwork 就是用来填补空白的

### 问题 4：加载很慢或超时

**可能原因**：网络连接问题或代理设置

**解决方案**：
1. 检查网络连接
2. 检查代理设置（RemoteArtwork 会自动尝试代理和直连）
3. 增加超时时间（在 remote_artwork.dart 中）

---

## 测试网易云 API

可以直接在浏览器中测试 API：

```
https://music.163.com/api/playlist/list?cat=全部&order=hot&limit=20&offset=0
```

检查返回的 JSON 中 `playlists[].coverImgUrl` 字段是否存在。

---

## 手动测试封面 URL

如果获取到了封面 URL，可以在浏览器中直接访问测试：

```
https://p1.music.126.net/xxx/xxx.jpg
```

如果浏览器能显示，说明 URL 是有效的。

---

## 检查当前版本

查看应用编译时间：

```bash
ls -l build\windows\x64\runner\Release\wcmusic.exe
```

如果修改时间是 15 小时前，说明需要重新构建。

---

## 快速验证

**最简单的验证方法**：

1. 构建新版本
2. 运行应用
3. 进入首页
4. 点击"刷新"按钮（平台热门歌单右上角）
5. 观察是否有加载指示器（灰色背景 + 旋转圈）
6. 如果有，说明正在加载；如果直接显示有机封面，说明加载失败

---

## 性能优化

如果封面加载太慢：

1. **使用国内镜像**（已在代码中处理）
   - 网易云会自动切换 p1/p2/p3.music.126.net

2. **增加缓存**
   ```dart
   if (_cache.length >= 128) // 从 64 增加到 128
   ```

3. **预加载**
   可以在进入首页前就开始加载封面

---

## 联系方式

如果问题依然存在，请提供：
- 控制台完整日志
- 应用构建时间
- 网络环境（是否使用代理）
- 测试的网易云 API 响应
