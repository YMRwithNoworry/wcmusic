# 代理绕过机制说明

## 问题背景

许多用户在使用代理（VPN/科学上网）时，发现应用中的封面和歌单无法加载。

**原因**：
- 代理服务器通常针对国际服务优化
- 访问国内服务（网易云、QQ音乐）可能被限速或屏蔽
- 网易云 CDN（*.music.126.net）通过代理访问速度慢或失败

---

## 解决方案

应用内置**智能代理绕过**机制，自动识别国内音乐服务并直连。

### 工作原理

```
用户请求 → 检测域名
    ↓
国内服务？
    ├─ 是 → 直连 (DIRECT) ✓
    └─ 否 → 使用系统代理设置
```

### 自动直连的域名

**网易云音乐**
- `*.music.126.net` (CDN封面)
- `*.music.163.com` (API)
- `music.163.com` (主域名)

**QQ音乐**
- `*.qq.com`
- `*.gtimg.cn` (CDN)
- `c.y.qq.com` (API)

**酷狗音乐**
- `*.kugou.com`
- `songsearch.kugou.com`
- `mobilecdnbj.kugou.com`

**酷我音乐**
- `*.kuwo.cn`
- `search.kuwo.cn`
- `kbangserver.kuwo.cn`

---

## 代码实现

### 1. 封面加载 (remote_artwork.dart)

```dart
static bool _shouldBypassProxy(Uri uri) {
  final host = uri.host.toLowerCase();
  // 网易云音乐 CDN
  if (host.endsWith('.music.126.net')) return true;
  if (host.endsWith('.music.163.com')) return true;
  // QQ音乐 CDN
  if (host.endsWith('.gtimg.cn')) return true;
  if (host.endsWith('.qq.com')) return true;
  // 酷狗、酷我
  if (host.contains('kugou')) return true;
  if (host.contains('kuwo')) return true;
  return false;
}
```

### 2. API 请求 (online_search_service.dart)

```dart
_proxyResolver = (uri) {
  // 国内音乐服务 API 直连
  if (_shouldBypassProxy(uri)) {
    return 'DIRECT';
  }
  return HttpClient.findProxyFromEnvironment(
    uri,
    environment: Platform.environment,
  );
};
```

---

## 用户体验

### 开启代理前
- ✓ 封面正常加载
- ✓ 歌单正常获取
- ✓ 搜索正常工作

### 开启代理后
- ✓ 封面正常加载（自动直连）
- ✓ 歌单正常获取（自动直连）
- ✓ 搜索正常工作（自动直连）
- ✓ 国际服务（如果有）使用代理

### 优势
1. **无需配置**：自动识别，用户无感知
2. **速度更快**：国内服务直连，延迟更低
3. **稳定可靠**：避免代理服务器问题
4. **兼容性好**：支持所有代理类型（HTTP/SOCKS5/系统代理）

---

## 技术细节

### 双重保障机制

1. **优先直连**：国内域名直接返回 `DIRECT`
2. **失败重试**：如果意外使用代理失败，自动重试直连

```dart
try {
  return await _loadOnce(uri, proxy);
} on Object catch (error, stackTrace) {
  // 如果使用了代理失败，尝试直连
  if (proxy != 'DIRECT' && !_shouldBypassProxy(uri)) {
    return await _loadOnce(uri, 'DIRECT');
  }
}
```

### 性能优化

- **域名缓存**：避免重复判断
- **并发请求**：直连不受代理连接数限制
- **超时控制**：直连更快响应

---

## 常见问题

### Q: 为什么不使用 PAC 文件？

A: PAC 需要用户手动配置，应用内置规则更方便。

### Q: 如果我想让国内服务也走代理怎么办？

A: 目前不支持，因为大多数用户的代理访问国内服务会有问题。如果有需求可以提 issue。

### Q: 支持企业代理/认证代理吗？

A: 国内服务会绕过所有代理，不受影响。国际服务（如果有）会使用系统代理设置。

### Q: 会影响隐私吗？

A: 不会。只是改变了网络路径（直连 vs 代理），不涉及数据收集。

### Q: 如何验证是否生效？

A: 
1. 开启代理
2. 打开应用
3. 查看封面是否正常加载
4. 或使用抓包工具（Fiddler）查看请求

---

## 测试建议

### 测试场景

1. **无代理**
   - 封面加载：✓ 应该正常
   - 歌单获取：✓ 应该正常

2. **HTTP 代理**
   - 封面加载：✓ 应该正常（绕过）
   - 歌单获取：✓ 应该正常（绕过）

3. **SOCKS5 代理**
   - 封面加载：✓ 应该正常（绕过）
   - 歌单获取：✓ 应该正常（绕过）

4. **系统代理**
   - 封面加载：✓ 应该正常（绕过）
   - 歌单获取：✓ 应该正常（绕过）

### 验证方法

**方法 1：抓包验证**
```
1. 启动 Fiddler 或 Charles
2. 开启代理
3. 打开应用
4. 检查网易云请求是否直连（不经过代理）
```

**方法 2：日志验证**
```dart
// 在 _loadOnce 中添加日志
print('Loading $uri with proxy: $proxy');
// 应该看到 DIRECT
```

---

## 未来改进

- [ ] 支持自定义绕过规则
- [ ] 添加代理状态指示器
- [ ] 提供手动切换选项
- [ ] 支持更多音乐服务

---

## 相关文件

- `lib/ui/core/remote_artwork.dart` - 封面加载
- `lib/data/services/online_search_service.dart` - API 请求
- `DEBUG_ARTWORK.md` - 封面调试指南
