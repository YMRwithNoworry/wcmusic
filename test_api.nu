#!/usr/bin/env nu

# 测试网易云歌单 API
print "🎵 测试网易云热门歌单 API..."

let response = http get "https://music.163.com/api/playlist/list?cat=全部&order=hot&limit=5&offset=0"

print $"✅ API 响应成功"
print $"歌单数量: ($response.playlists | length)"
print ""

for playlist in $response.playlists {
    print $"📀 歌单: ($playlist.name)"
    print $"   ID: ($playlist.id)"
    print $"   平台: 网易云音乐"
    
    let artwork = if "coverImgUrl" in $playlist {
        $playlist.coverImgUrl
    } else if "picUrl" in $playlist {
        $playlist.picUrl
    } else {
        ""
    }
    
    if ($artwork | is-empty) {
        print $"   ❌ 封面: 无"
    } else {
        let secure_url = ($artwork | str replace "http://" "https://")
        print $"   ✅ 封面: ($secure_url)"
    }
    print ""
}
