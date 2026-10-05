# liquidglass 复刻规格书（逐行源码级）

来源仓库：`C:/Users/Administrator/AppData/Local/Temp/liquidglass`（@ybouane/liquidglass v1.0.3，MIT）
覆盖文件：`src/defaults.ts`(79行)、`src/shaders.ts`(286行)、`src/GlassRenderer.ts`(419行)、`src/LiquidGlass.ts`(1509行)、`src/HtmlCapture.ts`(590行)、`README.md`(242行)

---

## 0. 一句话架构

```
DOM 子元素 ──html-to-image/Canvas2D──▶ 场景画布(每块玻璃一块局部裁剪区)
                                          │
                            纹理上传 + 6×9tap 高斯模糊
                                          │
                          FS_GLASS 片元着色器（折射/色散/高光/阴影）
                                          │
                              WebGL 画布 ──drawImage──▶ 玻璃元素内的 <canvas>
                                          │
                              写回场景画布 ──▶ 供 z 序更高的玻璃折射（分层合成）
```

**重要：这不是真正的"屏幕背景实时采样"库。** 它不读 `backdrop-filter`、不截屏。它自己用 Canvas2D **重绘** root 的子元素（静态的用 html-to-image 栅格化缓存，img/canvas/video 走 drawImage 快路径），拼成一张局部场景画布，再上传成 WebGL 纹理。所以"纹理怎么来的"是复刻时的第一难点。

---

## 1. `src/defaults.ts` —— 配置项全清单

### 1.1 `GlassConfig` 逐项（名字 / 类型 / 默认值 / 取值域 / 含义 / 落到哪个 uniform）

| 字段 | 类型 | 默认值 | 建议取值范围 | 含义 | 去向 |
|---|---|---|---|---|---|
| `blurAmount` | number | `0.00` | 0 ~ 1（demo 最高用 0.5） | 背景模糊强度。0 = 完全不跑模糊趟；1 = 最大 | `uploadAndBlur(blurAmount)` → 模糊核步长 |
| `refraction` | number | `0.69` | 0 ~ 1.5（demo 用 1.2） | 玻璃弯折背后图像的强度 | `u_refract` |
| `chromAberration` | number | `0.05` | 0 ~ 0.3（demo 用 0.2） | 边缘色散/彩边 | `u_chroma` |
| `edgeHighlight` | number | `0.05` | 0 ~ 0.25（demo 用 0.15/0.2） | 边缘辉光 + rim + 内描边总强度 | `u_edgeHL` |
| `specular` | number | `0.00` | 0 ~ 1 | 四光源 Blinn-Phong 高光总强度 | `u_spec` |
| `fresnel` | number | `1.00` | 0 ~ 1.5 | 掠射角反射强度 | `u_fresnel` |
| `distortion` | number | `0.00` | 0 ~ 0.2 | 逐像素微噪声抖动 | `u_distort` |
| `cornerRadius` | number | `65` | ≥0，CSS px（会被 clamp 到 min(size.x,size.y)/2） | 圆角半径 | `u_radius = cornerRadius * dpr` |
| `zRadius` | number | `40` | ≥0，CSS px | 倒角（bevel）深度 = 高度场的 z 半径 | `u_zRadius = zRadius * dpr` |
| `opacity` | number | `1.00` | 0 ~ 1 | 整体不透明度（只乘到 mask alpha） | `u_alpha` |
| `saturation` | number | `0.00` | -1 ~ 1 | 饱和度调整（-1 = 灰度） | `u_sat` |
| `tintStrength` | number | `0.00` | 0 ~ 1 | 冷蓝染色强度 | `u_tint` |
| `brightness` | number | `0.00` | -0.5 ~ 0.5 | 亮度偏移（乘 `1+brightness`） | `u_brightness` |
| `shadowOpacity` | number | `0.30` | 0 ~ 1 | 投影不透明度 | `u_shadowAlpha` |
| `shadowSpread` | number | `10` | ≥1（着色器内 `max(spread,1.0)`），CSS px | 投影扩散半径 | `u_shadowSpread = shadowSpread * dpr` |
| `shadowOffsetY` | number | `1` | 任意，CSS px（正值向下） | 投影垂直偏移 | `u_shadowOffY = shadowOffsetY * dpr` |
| `floating` | boolean | `false` | — | **可拖拽移动**（Pointer Events），不是动画 | CPU 逻辑，无 uniform |
| `button` | boolean | `false` | — | 按钮模式：hover 提亮 / 按下压平倒角+加深阴影 | CPU 改 config 后再进 uniform |
| `bevelMode` | number | `0` | 0 或 1 | 0 = 双凸药丸（双面折射）；1 = 穹顶/平凸（单面折射 + 放大镜） | `u_bevelMode` |

### 1.2 两个非配置导出常量

| 常量 | 值 | 含义 |
|---|---|---|
| `BLUR_ITERATIONS` | **6** | 高斯模糊迭代次数（每次 = 横 1 趟 + 纵 1 趟） |
| `SHADOW_PAD` | **20** | 每块玻璃四周额外渲染留白（CSS px），用于画阴影；玻璃 canvas 与纹理裁剪区都按 `size + 2*20` 计算 |

### 1.3 三个"隐藏常量"（不在 defaults.ts，但在着色器里硬编码）

| 名称 | 值 | 位置 |
|---|---|---|
| 折射率 IOR | **1.5** | `FS_GLASS` `float ior = 1.5;` |
| 内描边宽度 | **1.5 px**（设备像素） | `float borderWidth = 1.5;` |
| 场景画布底色 | **`#ffffff`** | `_prepareSceneCanvas` 用白填充（不是透明！） |

---

## 2. `src/GlassRenderer.ts` —— 完整渲染管线

### 2.1 WebGL 上下文与画布

| 项 | 值 |
|---|---|
| 画布 | 离屏 `<canvas>`，`style.display='none'`，**append 到 document.body** |
| 上下文参数 | `alpha:true, premultipliedAlpha:false, antialias:false, preserveDrawingBuffer:true` |
| GL 版本 | WebGL 1（GLSL ES 1.0） |
| 上下文丢失 | 监听 `webglcontextlost`（`preventDefault` + `contextLost=true`）；`webglcontextrestored` → 重建 program/buffer、释放并清空 FBO 池、`bgTex=null` |
| 画布尺寸策略 | **只增不减**：`if (canvas.width < w) canvas.width = max(canvas.width, w)`；`resize()` 里会先置 0 以允许缩小 |

### 2.2 三个 program

| program | VS | FS | uniforms |
|---|---|---|---|
| blit | `VS_QUAD` | `FS_BLIT` | `u_tex, u_scale, u_offset` |
| blur | `VS_QUAD` | `FS_BLUR` | `u_tex, u_dir` |
| glass | `VS_GLASS` | `FS_GLASS` | `u_bgTex, u_blurTex, u_center, u_size, u_radius, u_res, u_pad, u_refract, u_chroma, u_edgeHL, u_spec, u_fresnel, u_distort, u_alpha, u_sat, u_tint, u_zRadius, u_brightness, u_shadowAlpha, u_shadowSpread, u_shadowOffY, u_bevelMode`（22 个） |

两个几何缓冲：

- `quadBuf`：`[-1,-1, 1,-1, -1,1, 1,1]`（全屏 NDC 四边形，TRIANGLE_STRIP 4 顶点）
- `panelBuf`：`[-.5,-.5, .5,-.5, -.5,.5, .5,.5]`（单位面板，供 `VS_GLASS` 放大）

### 2.3 顶点着色器 `VS_GLASS`

```
total = u_size + 2*u_pad
v_localPx = a_pos * total                       // 以面板中心为原点的设备像素坐标
px        = u_center + a_pos * total            // root 左上角原点的设备像素坐标
v_screenUV = (px.x/u_res.x, 1 - px.y/u_res.y)   // 转成纹理 UV（Y 翻转）
ndc = (px/u_res)*2 - 1;  ndc.y = -ndc.y
```

注意：因为 `a_pos ∈ [-0.5, 0.5]`，`v_localPx ∈ [-total/2, +total/2]`，所以片元着色器里 `u_size` 直接就是"完整面板尺寸"（不需要再乘 2）。**这是复刻时最容易搞错的一处。**

### 2.4 纹理怎么来的：`uploadAndBlur()`

输入：`sourceCanvas`(场景画布)、`sourceX/sourceY`(恒为 0,0)、`width/height`(= 玻璃 canvas 的 **设备像素** 尺寸，即 `(offsetW+40)*dpr × (offsetH+40)*dpr`)、`blurAmount`。

逐步：

1. `_setActiveSize(w,h)`：失败(w/h≤0)直接返回；否则设置画布尺寸、按 key `"WxH"` 从 **FBO 池**取或新建 `FBOSet{bg, blurA, blurB}`（每个 FBO 都是 RGBA8 纹理 + FRAMEBUFFER，LINEAR + CLAMP_TO_EDGE）。
2. 把 `sourceCanvas` 用 2D 上下文拷进 `cropCanvas`（同尺寸，先 `clearRect`，再 `drawImage(sourceCanvas, -sourceX, -sourceY)`）。
3. 上传纹理：`UNPACK_FLIP_Y_WEBGL = true` → `texImage2D(RGBA, RGBA, UNSIGNED_BYTE, cropCanvas)` → 设 LINEAR/CLAMP → 复位 FLIP_Y=false。
4. **blit 源→bgFBO**：绑定 `bg.fbo`，viewport(0,0,W,H)，`u_scale=(1,1)`，`u_offset=(0,0)`，画 quad。
5. **blit bgFBO→blurA**（纹理换成 `bg.tex`，viewport 用 blurA 尺寸；scale/offset 仍是 1/0）。
6. **模糊（`blurAmount > 0` 才跑）**：
   ```
   spread = blurAmount * 2.5          // 单位：像素
   重复 BLUR_ITERATIONS(=6) 次：
       绑 blurB.fbo, viewport(bw,bh), 输入 blurA.tex, u_dir = (spread/bw, 0)   // 横向
       绑 blurA.fbo,                 输入 blurB.tex, u_dir = (0, spread/bh)   // 纵向
   ```
   → **共 6 趟水平 + 6 趟纵向 = 12 个 draw call**，每趟 9 tap。

**模糊尺度精确换算**：每次 1D 趟覆盖 ±4×spread 像素；6 次 H+V 往返等效半径 ≈ `4 × spread × 6 = 60 × blurAmount` 像素（设备像素）。`blurAmount=0.25` → spread=0.625px，等效半径约 15px；`blurAmount=1` → spread=2.5px，等效半径约 60px。

**降采样：完全没有。** blurA/blurB 与 bg 尺寸完全一致（= 玻璃 canvas 设备像素尺寸），不做任何 mip/半分辨率。模糊成本与面板面积成正比。

9-tap 高斯核（权重，和为 0.999999 ≈ 1）：

| 偏移(tap) | 权重 |
|---|---|
| 0 | 0.227027 |
| ±1 | 0.194594 |
| ±2 | 0.121622 |
| ±3 | 0.054054 |
| ±4 | 0.016216 |

### 2.5 玻璃面板渲染：`renderGlassPanel()`

1. 开混合：`BLEND` + `blendFunc(SRC_ALPHA, ONE_MINUS_SRC_ALPHA)`
2. 绑定纹理单元：TEXTURE0 = `fboSet.bg.tex`（`u_bgTex=0`，**清晰版**）、TEXTURE1 = `fboSet.blurA.tex`（`u_blurTex=1`，**模糊版**）
3. 绑默认帧缓冲（`null`），`viewport(0, canvas.height - H, W, H)`（翻转 Y，让 GL 原点在左下而内容从画布顶部开始）
4. 传 uniform（见下表）
5. `drawQuad(panelBuf)`，然后关混合

uniform 赋值（注意 `u_res = (W,H)` 是**裁剪区尺寸**，不是屏幕尺寸）：

| uniform | 值 |
|---|---|
| `u_res` | `(W, H)` = 玻璃 canvas 设备像素宽高（含 padding） |
| `u_center` | `(W*0.5, H*0.5)` |
| `u_size` | `(offsetWidth*dpr, offsetHeight*dpr)`（不含 padding） |
| `u_pad` | `SHADOW_PAD * dpr` = 20*dpr |
| `u_radius` | `cornerRadius * dpr` |
| `u_zRadius` | `zRadius * dpr` |
| `u_shadowSpread` | `shadowSpread * dpr` |
| `u_shadowOffY` | `shadowOffsetY * dpr` |
| 其余 | 直接取 config 原值（`u_refract, u_chroma, u_edgeHL, u_spec, u_fresnel, u_distort, u_alpha, u_sat, u_tint, u_brightness, u_bevelMode`） |

### 2.6 每帧做什么（`_renderLoop` / `_renderFrame` / `_renderGlassElement`）

每帧顺序：

1. **FPS 统计**（1 秒窗口写入 `instance.fps`）。
2. **尺寸检查** `_checkGlassSizeChanges()`：比较 `offsetWidth/Height` 与上次（阈值 0.5px，用 offset* 而非 getBoundingClientRect 以免 CSS transform 造成假变化）。变化则重算 canvas 尺寸、清该玻璃的位置缓存、清 html-to-image 缓存、标脏。
3. **内容重捕获**：若 `_glassContentDirty` 非空且当前无捕获在跑，快照并清空该集合，异步 `_captureGlassContent(targets)`。
4. **`_renderFrame()`**：
   - a. 把 `_userMarkedChanged` 的每个元素 fan-out 成"与之相交的玻璃"脏标记；
   - b. 把 `_globalDirty` 展开成"所有玻璃脏"，并清标志；
   - c. 若 `_glassDirty 为空 且 无 data-dynamic/video 且 非拖拽中` → **本帧直接 return（空转零成本）**；
   - d. 快照并清空 `_glassDirty`；
   - e. 按**排序后的子元素顺序**遍历，对每个玻璃调用 `_renderGlassElement`。
5. 对每块玻璃判定是否需要重跑着色器：

```
needsShaderRender = 拖拽中 ?
      (是本块被拖 || 显式脏 || 更早玻璃本帧重绘过且相交 || 有动态贡献者)
    : (无位置缓存 || 中心移动>0.5px || 显式脏 || 更早玻璃本帧重绘过且相交 || 有动态贡献者)
```

6. 需要时执行渲染三连：
   - `_composeSceneForGlass()`：**只绘制 z 序在该玻璃之前**的兄弟元素，拼成本地场景；
   - `renderer.uploadAndBlur(sceneCanvas, 0, 0, canvas.width, canvas.height, blurAmount)`；
   - `renderer.clear()`（scissor 清裁剪区）+ `renderer.renderGlassPanel(config, offsetW, offsetH, dpr)`；
   - `glassCanvas.getContext('2d').clearRect(...)` 后 `drawImage(renderer.canvas, 0,0,W,H → 0,0,W,H)`。
   - 位置缓存写入 `{centerX, centerY}`，并 push 到 `renderedThisFrame`。

> 用 `glassCanvas.width/height` 而不是 `sampleRect.w/h` 作为渲染尺寸，是为了避免 `getBoundingClientRect`（CSS 尺寸）与 `offsetWidth`（布局尺寸）四舍五入差 1 设备像素导致最右/最下一列残留旧像素的细线。

### 2.7 分层合成（layered compositing）怎么工作

核心在 `_composeSceneForGlass` + `_drawPriorGlassToScene`：

```
_composeSceneForGlass(当前玻璃, sampleRect, rootRect, dpr):
    _prepareSceneCanvas(sampleRect.w, sampleRect.h)   // 尺寸变化则 resize，否则 clearRect；然后填白 #ffffff
    for child in _sortedChildren:                      // 绘制顺序 = 自下而上
        if child == 当前玻璃: break                     // 只画"在它下面"的
        if child 是玻璃: _drawPriorGlassToScene(child)
        else:           _drawNonGlassChildToScene(child)
```

`_drawPriorGlassToScene(child)` 依次画两样东西（都先做矩形相交剔除）：

1. `child` 的**着色器输出 canvas**，按 `_getPixelRect(elRect, rootRect, dpr, SHADOW_PAD)`（含 20px 阴影留白）贴到 `shaderRect - sampleRect` 偏移处；
2. `child` 的**内容图** `_glassContentImages.get(child)`（玻璃自身 DOM 内容，用 html-to-image 捕获、并**排除注入的着色器 canvas**），按无 padding 的矩形贴合。

因此 z 序更高的玻璃在做折射时，其清晰纹理 `u_bgTex` 里已经包含下方玻璃的最终像素（含阴影与其内容）。**这是"分层合成"的全部含义**——没有任何额外的混合 pass 或 render target 链。

子元素绘制顺序 `_getSortedChildren()`：重实现 CSS 层叠上下文规则 —— 非 stacking context 的排在前面（DOM 序），有 stacking context 的按 `zIndex` 升序、同 z 按 DOM 序。识别为 stacking context 的条件：非 static position；flex/grid 父元素下 `z-index ≠ auto`；`opacity<1`；`transform/filter/perspective/clip-path/mix-blend-mode/isolation/backdrop-filter/mask-image` 非 none；`contain` 含 layout|paint|strict|content；`will-change` 列出上述任一属性。

### 2.8 非玻璃子元素怎么进场景

- `CANVAS / IMG / VIDEO` 直接 `drawImage` 快路径（`_drawMediaElement`）。IMG 要求 `complete && naturalWidth>0`；VIDEO 要求 `readyState ≥ 1`；尺寸 ≤0 直接跳过。img/video 走 `_drawMediaFitted` 复刻 `object-fit`：
  - `fill`（或 `scale-down` 且原图小于盒子）→ 整图拉伸；
  - `cover` → `scale = max(boxW/natW, boxH/natH)`，取居中的 `sw=boxW/scale, sh=boxH/scale`，按 object-position 百分比偏移；
  - `contain` / `scale-down` → 整图 + 由 drawImage 目标矩形完成 contain；
  - `none` → `sw=boxW, sh=boxH` 按 position 取窗口；
  - 最后 clamp 到图像边界。
- 其它元素（wrapper）：先做"绘制溢出 pad"相交剔除（`_getPaintOverflowPad`：是玻璃→20；有 `box-shadow/text-shadow/filter/backdrop-filter/mask-image/mix-blend-mode` → 20；否则 0），再 ① 递归画出内部所有 img/video/canvas（跳过注入的玻璃 canvas），② 用 `captureElement` 的缓存 html-to-image 快照 `drawCachedElement` 贴上去。
- 已知局限：绝对定位逃出 wrapper 盒子的后代不会被画（需给独立 wrapper 或 `data-dynamic`）。

### 2.9 纹理尺寸汇总表（复刻必看）

| 对象 | 尺寸 |
|---|---|
| root 场景裁剪区 `_sceneCanvas` | `sampleRect.w × sampleRect.h` = `round((rect.w + 40) * dpr) × round((rect.h + 40) * dpr)` |
| 玻璃 canvas（DOM 里那个） | `round((offsetW+40)*dpr) × round((offsetH+40)*dpr)`，CSS `left/top = -20px`，宽高 `offset+40` |
| FBO bg / blurA / blurB | 与玻璃 canvas 完全相同（无降采样） |
| WebGL 离屏 canvas | 至少为上述最大值，只增不减 |

---

## 3. `FS_GLASS` 完整算法（逐行中文伪代码，精确到常数）

约定：以下所有距离单位都是**设备像素**（uniform 已经乘过 dpr）。`v_localPx` 以面板中心为原点，范围 `[-(size+2*pad)/2, +(size+2*pad)/2]`。

### 3.0 入口准备

```
half_ = u_size * 0.5                                  // 面板半宽半高
r     = min(u_radius, min(half_.x, half_.y))          // 圆角被 clamp
sdf   = rrSDF(v_localPx, half_, r)
```

**圆角矩形 SDF 公式**（`rrSDF(p, b, r)`，p=点，b=半尺寸，r=半径）：

```
q = abs(p) - b + vec2(r)            // 两个分量都加 r
return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - r
```
即：内部为负（到边界的负距离），外部为正（到边界的最小距离），边界为 0。

### 3.1 投影（只对 sdf > 0 的像素，即面板外）

```
if (sdf > 0.0):
    sdfShadow = rrSDF(v_localPx - vec2(0.0, u_shadowOffY), half_, r)   // 形状整体下移 offsetY
    d         = max(sdfShadow - 1.0, 0.0)                              // 从阴影形状边缘外 1px 起算
    spread    = max(u_shadowSpread, 1.0)
    falloff   = 1.0 / (spread * spread)
    outerShadow   = exp(-d*d*falloff) * 0.65
    contactShadow = exp(-d * 0.08 / max(spread*0.04, 0.01)) * 0.35
    shadow        = (outerShadow + contactShadow) * u_shadowAlpha
    gl_FragColor  = vec4(0, 0, 0, shadow)      // 纯黑，预乘 alpha = 0 → 普通混合
    return
```

要点：
- 两个高斯：`outer` 衰减长度 ≈ `spread`（默认 10px → 在 d=10 处衰减到 37%），峰值 0.65；`contact` 衰减长度 ≈ `1/(0.08/(spread*0.04)) = spread*0.5`（默认 5px），峰值 0.35。
- 阴影只出现在**面板外**；面板内部不画阴影。
- 阴影颜色恒为黑色，透明度乘 `shadowOpacity`。注意输出 `alpha = shadow` 而 RGB = 0，配合 `SRC_ALPHA/ONE_MINUS_SRC_ALPHA` 是正确的黑色半透明。

### 3.2 抗锯齿 mask

```
mask = 1.0 - smoothstep(-1.5, 0.5, sdf)     // 边界过渡带宽 2px（-1.5 → +0.5）
```

### 3.3 边缘权重与深度

```
maxD   = min(half_.x, half_.y)
inside = -sdf                                // 内部深度（正数）
edge   = smoothstep(maxD * 0.35, 0.0, inside)
```
即 `inside = 0`（边界）时 `edge = 1`，`inside ≥ 0.35*maxD` 时 `edge = 0`，中间平滑过渡。

### 3.4 bevel 高度场与法线

高度场（`d` = 距边缘的向内距离，`zR` = u_zRadius）：

```
bevelHeight(d, zR):
    if d <= 0:      return 0
    if d >= zR:     return zR
    return sqrt(d * (2*zR - d))       // 半径 zR 的圆截面：h² + (d-zR)² = zR²
```
（在 d=zR 处函数值与导数都连续：h=zR，h'=0；在 d=0 处 h=0，h'=1 —— 45° 接边。）

法线（中心差分，步长 `e = 2.0` 设备像素）：

```
dC = inside
dR = -rrSDF(v_localPx + (e,0), half_, r)     // 注意：这里用的是"平移后的 SDF"，不是 dC+e
dL = -rrSDF(v_localPx - (e,0), half_, r)
dU = -rrSDF(v_localPx + (0,e), half_, r)
dD = -rrSDF(v_localPx - (0,e), half_, r)
hC = bevelHeight(dC, zR); hR = bevelHeight(dR, zR); hL = ...; hU = ...; hD = ...
hGrad = vec2(hR - hL, hU - hD) / (2.0 * e)      // 除以 4.0
N     = normalize(vec3(-hGrad, 1.0))            // z 分量恒为 1，xy 取负梯度
```
（注释里说"用平移后的 SDF 近似"对圆角矩形是正确的，因为圆角矩形内部 SDF 与真实距离一致。）

深度因子：

```
depth = smoothstep(0.0, zR, inside)     // 边缘 0 → 距边 zR 处 1
```

### 3.5 双面折射（bevelMode = 0，双凸药丸）

```
pxToUV   = vec2(1.0, -1.0) / u_res        // 像素偏移 → UV 偏移（Y 取反）
ior      = 1.5
refrPow  = 1.0 - 1.0/ior = 0.3333333      // 1 - 1/1.5
thickness   = hC * 2.0                    // 中心厚度
thickNorm   = thickness / max(zR*2.0, 1.0)
exitRefr    = hGrad * refrPow             // 出射面偏折
entryRefr   = hGrad * refrPow             // 入射面偏折
throughRefr = entryRefr * thickNorm * 0.5 // 穿过厚度带来的横向位移
refrPx = (exitRefr + entryRefr + throughRefr) * u_refract * 30.0
centerDir = -v_localPx / max(half_, vec2(1.0))   // 指向中心，归一化到面板半尺寸
refrPx += centerDir * u_refract * 4.0 * depth    // 额外的向心放大项
refr = refrPx * pxToUV
```
默认参数（zRadius=40, half≈100）：`thickNorm = hC/40`（hC 最大 40 → 最大 1），所以三项系数大致是 `0.333+0.333+0.167 = 0.833`，再乘 `refraction*30`；`refraction=0.69` 时边缘梯度 1 处 ≈ 17 像素的位移量级，中心向心项再贡献最多 `0.69*4=2.76` 像素。

### 3.6 穹顶折射（bevelMode = 1，平凸放大镜）

```
refrPx = -v_localPx * u_refract * depth * 0.35
refr   = refrPx * pxToUV
```
即"把采样点朝中心收缩" → 内容被放大。最大收缩比例 = `refract*0.35`（`refract=1.2` → 42% of 半尺寸）。配合 `cornerRadius == zRadius` 得到半球放大镜。

> 分支条件是 `if (u_bevelMode < 0.5)`，即任何 <0.5 的值都走双凸分支。

### 3.7 微噪声抖动

```
hash(p) = fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453)
ns     = v_localPx * 0.08
absPxToUV = vec2(1.0) / u_res
micro  = (vec2(hash(ns), hash(ns + vec2(37.0))) - 0.5) * u_distort * 4.0 * absPxToUV
```
即每像素抖动幅度 = `u_distort * 4.0 * (hash-0.5)` 设备像素（`distortion=1` 时 ±2px）。默认 0。

### 3.8 色散（chromatic aberration）

```
caS = u_chroma * 18.0 * (edge * 0.7 + 0.3) * 2.0
caD = N.xy * caS * pxToUV
```
即：中心（edge=0）偏移 = `chroma*18*0.3*2 = 10.8*chroma` 像素；边缘（edge=1）= `chroma*18*1.0*2 = 36*chroma` 像素。方向由法线 xy 决定，所以色散沿倒角法线方向（径向）。

### 3.9 边缘加权混合（清晰 vs 模糊）

```
base = v_screenUV + refr + micro
sharp.r = tex(u_bgTex,  base + caD).r
sharp.g = tex(u_bgTex,  base).g
sharp.b = tex(u_bgTex,  base - caD).b
blur.r  = tex(u_blurTex, base + caD).r
blur.g  = tex(u_blurTex, base).g
blur.b  = tex(u_blurTex, base - caD).b
edgeMix = 1.0 - edge * 0.15          // 中心 1.0，边缘 0.85
col     = mix(sharp, blur, edgeMix)
```
**注意方向**：`mix(sharp, blur, edgeMix)`，edgeMix 越接近 1 越取 blur。所以**中心几乎全用模糊纹理，边缘反而偏向清晰纹理**（保留折射边缘的锐利）。边缘最多只让清晰版占 15%。

### 3.10 亮度 / 饱和度 / 冷色染色

```
col *= 1.0 + u_brightness
lum  = dot(col, vec3(0.299, 0.587, 0.114))
col  = mix(vec3(lum), col, 1.0 + u_sat)
col  = mix(col, col * vec3(0.92, 0.95, 1.05), u_tint)   // 冷蓝染色
col *= 1.0 + 0.06 * depth                                // 内部微提亮，最多 +6%
```

### 3.11 Fresnel

```
fres = pow(1.0 - abs(N.z), 4.0) * u_fresnel
```
指数 4；`N.z` 在中心为 1（fres=0），边缘趋近 0（fres→u_fresnel）。默认 fresnel=1。

### 3.12 多光源高光（Blinn-Phong，4 个光源）

```
V = (0, 0, 1)

光源1: L1 = normalize(0.4, 0.7, 1.0);  H1 = normalize(L1+V);  sp1 = pow(max(dot(N,H1),0), 90.0)
光源2: L2 = normalize(-0.3,-0.5, 1.0); H2 = normalize(L2+V);  sp2 = pow(max(dot(N,H2),0), 50.0) * 0.3
光源3: L3 = normalize(0.1, 0.3, 1.0);                         spB = pow(max(dot(N,L3),0), 6.0)  * 0.1   // 宽柔光，直接 N·L
光源4: L4 = normalize(0.0, 0.9, 0.4);  H4 = normalize(L4+V);  sp4 = pow(max(dot(N,H4),0),120.0) * 0.6

totalSpec = (sp1 + sp2 + spB + sp4) * u_spec
```

| 光源 | 方向（未归一化） | 类型 | 指数 | 权重 |
|---|---|---|---|---|
| 1 | (0.4, 0.7, 1.0) 右上偏前 | 半程向量 | 90 | 1.0 |
| 2 | (-0.3, -0.5, 1.0) 左下偏前 | 半程向量 | 50 | 0.3 |
| 3 | (0.1, 0.3, 1.0) | **N·L 漫反射式**（非半程） | 6 | 0.1 |
| 4 | (0.0, 0.9, 0.4) 顶部掠射 | 半程向量 | 120 | 0.6 |

（源码注释称"multi-light Blinn-Phong"，但第 3 项实际用的是 `dot(N, L3)`，复刻时按代码走。）

### 3.13 内描边 / rim / inner glow / 伪环境反射

```
// 内描边（inner stroke）
borderWidth = 1.5
innerStroke = smoothstep(-borderWidth - 1.0, -borderWidth, sdf)      // sdf ∈ [-2.5, -1.5] 渐入
            * (1.0 - smoothstep(-1.0, 0.0, sdf))                      // sdf ∈ [-1, 0] 渐出
topBias     = 0.5 + 0.5 * (-v_localPx.y / half_.y)   // 顶部=1，底部=0
innerStroke *= (0.4 + 0.6 * topBias)                 // 顶部乘 1.0，底部乘 0.4

// rim 与 inner glow
rim       = edge * u_edgeHL * 0.22
innerGlow = smoothstep(5.0, 0.0, -sdf) * u_edgeHL * 0.15    // 距边界 5px 内渐弱，sdf=0 处最强

// 伪环境反射
envRefl = (N.y * 0.5 + 0.5) * fres * 0.08
```

于是"内描边"是一条宽约 1.5px、位于边界内侧、**上亮下暗（顶部 1.0 / 底部 0.4 倍）**的高光带。

### 3.14 最终合成与 alpha

```
fin  = col
fin += vec3(totalSpec)
fin += vec3(rim + innerGlow)
fin += vec3(innerStroke * u_edgeHL * 0.55)
fin += vec3(envRefl)
fin  = mix(fin, vec3(1.0), fres * 0.2)      // 最多 20% 向白色提亮

gl_FragColor = vec4(fin, mask * u_alpha)
```

**alpha 合成语义**：
- 颜色是**非预乘**的（上下文 `premultipliedAlpha:false`），alpha = `mask * opacity`；
- 所有光效（spec/rim/glow/stroke/envRefl）是**加到颜色上但被 alpha 一并缩放**的，所以 `opacity` 降低时整体（含高光）一起变透明；
- 高光项不受 mask 单独调制，只由 alpha 统一控制（面板外像素走 3.1 的阴影分支提前 return，不会漏光）；
- 输出走标准 `SRC_ALPHA, ONE_MINUS_SRC_ALPHA` 混合。

### 3.15 常数速查表（复刻用）

| 常数 | 值 | 出处 |
|---|---|---|
| AA 边界带 | `smoothstep(-1.5, 0.5, sdf)` | mask |
| edge 过渡 | `smoothstep(maxD*0.35, 0.0, inside)` | edge |
| 法线差分步长 | 2.0 px | hGrad |
| IOR | 1.5（refrPow = 0.3333） | 折射 |
| 双凸位移总系数 | `(exit+entry+through) * refract * 30.0` | 折射 |
| 向心放大项 | `refract * 4.0 * depth` | 折射 |
| 穹顶收缩 | `refract * depth * 0.35` | 折射 |
| 噪声频率 | `localPx * 0.08`，hash 偏移 `+37.0`，幅度 `distort*4.0` | micro |
| 色散 | `chroma*18.0*(edge*0.7+0.3)*2.0` px | caD |
| 模糊/清晰混合 | `edgeMix = 1 - edge*0.15` | mix |
| 深度提亮 | `1 + 0.06*depth` | tint |
| 冷色染色向量 | `(0.92, 0.95, 1.05)` | tint |
| 亮度系数 | `1 + brightness` | brightness |
| 饱和度 | `mix(lum, col, 1 + sat)`，lum 权重 (0.299,0.587,0.114) | sat |
| Fresnel | `pow(1-N.z, 4)` | fres |
| 高光 | 90/50/6/120 指数，1.0/0.3/0.1/0.6 权重 | spec |
| 内描边宽 | 1.5 px，上偏 `0.4+0.6*topBias` | stroke |
| rim | `edge*edgeHL*0.22` | rim |
| inner glow | `smoothstep(5,0,-sdf)*edgeHL*0.15` | glow |
| 内描边权重 | `innerStroke*edgeHL*0.55` | composite |
| 环境反射 | `(N.y*0.5+0.5)*fres*0.08` | envRefl |
| 白化 | `mix(fin, 1.0, fres*0.2)` | composite |
| 阴影 | `exp(-d²/spread²)*0.65 + exp(-d*0.08/(spread*0.04))*0.35`，再乘 shadowOpacity | shadow |

---

## 4. `src/LiquidGlass.ts`

### 4.1 `floating` 做了什么动画？

**结论：`floating` 不是动画，是"拖拽交互"。没有任何补间/惯性/回弹/形变。**

具体实现（Pointer Events）：

| 阶段 | 行为 |
|---|---|
| 初始化 | 若 `floating:true` → `el.style.touchAction = 'none'`（阻止浏览器抢手势） |
| 命中测试 | `pointerdown` 时按**逆 z 序**遍历玻璃；用 `offsetWidth/Height` + 由 boundingRect 反推的视觉原点（把 offset 尺寸在 rect 中居中）做矩形命中 |
| 开始拖 | 记录起点 clientX/Y 与当前 `translate(tx,ty)`；`cursor:'grabbing'`；`setPointerCapture`；`preventDefault()` |
| 移动 | `newTx = origTx + dx`；用 boundingRect 反推"布局基位"（`baseLeft = visualLeft - rootRect.left - curTx`），然后**边界约束 margin = 10px**（左右上下都不允许超出 root 内缩 10px）；写 `el.style.transform = translate(...)` |
| 移动时重绘 | 先 `_markGlassAndDependents(el, oldRect)`（清旧足迹），改 transform，再 `_markGlassAndDependents(el)`（拾取新足迹） |
| hover | 未拖拽时逐玻璃做同样的命中测试，命中设 `cursor:'grab'`，否则清空 |
| 结束 | `pointerup` 恢复 cursor、再标一次脏做最终渲染 |

拖拽期间（`isDragging=true`）**所有玻璃每帧都进 `needsRender` 分支**（整页重绘），每块玻璃再按 `是本块被拖 || 显式脏 || 更早玻璃相交重绘 || 有动态贡献者` 决定是否真的跑着色器。

### 4.2 `data-dynamic` 的重捕获机制

```
_hasDynamic = root 内存在 data-dynamic 元素（且本身不是玻璃） || 存在 <video>（且不是玻璃）
```

- 每个 **root 的直接子元素**若带 `data-dynamic`，其 html-to-image 快照**每帧重做**（`captureElement(child, force=true)`）；`<video>` 自动视为 dynamic（且走 drawImage 快路径）。
- 判定某块玻璃是否受影响：`_glassHasDynamicContributors(glass)` —— ① 玻璃自身子树含 `data-dynamic`/`video` → 是；② 遍历 z 序在它之前的**非玻璃**子元素，若其自身或其后代含 `data-dynamic`/`video` 且与玻璃 sampleRect 相交 → 是。命中则该玻璃每帧重跑着色器。
- 并发去重：`HtmlCapture._capturing` 集合保证同一元素每帧最多一次捕获；`_capturingGlassContent` 保证玻璃内容捕获不重入（重入时脏集保留到下一帧）。
- 捕获完成回调 `onCacheUpdate` → `_markGlassesIntersecting(el)` → 只标与它相交的玻璃脏。
- 玻璃**自身**的内容图（用于把标签文字合成到着色器输出之上）由 `captureToCanvas(el, w, h, [glassCanvas])` 生成，通过 html-to-image 的 `filter` 把注入的着色器 canvas 剪掉（不改动真实 DOM，无闪烁）。

### 4.3 有没有形状变形 / 融合（metaball、morph）能力？

**没有。完全没有。**

- 源码里没有任何 metaball / goo / blur-threshold 融合、也没有形状 morph/形变插值；
- 圆角是**每块玻璃独立的静态 uniform**（`cornerRadius`），没有任何动画通道；
- 唯一的"形变"是 `button` 模式下按下瞬间把 `zRadius × 0.8`、`shadowSpread × 1.2`，以及 hover 时 `brightness + 0.2`——都是**瞬时跳变，无过渡动画**；
- 玻璃之间的"融合感"只来自分层合成（下方玻璃的像素被上方玻璃折射），**不是**形状合并。

### 4.4 `button` 模式的精确数值

在 `_getConfig` 里，基于 pointerover/out/down/up/cancel 维护的 `{hover, pressed}`：

```
pressed:  zRadius      *= 0.8      // 倒角压平
          shadowSpread *= 1.2      // 阴影加深
          （brightness 不提升）
else hover: brightness += 0.2
```
另外给元素加 class `liquid-glass-button`（唯一 CSS：`cursor:pointer`），并注入一次 `<style id="liquid-glass-button-styles">`。

### 4.5 脏标记体系（复刻时的性能骨架）

| 集合/标志 | 触发源 | 消费方式 |
|---|---|---|
| `_globalDirty` | resize、root 子列表结构变化、WebGL 上下文恢复、`_start` 末尾、`markChanged()` 无参 | 每帧展开为"所有玻璃进 `_glassDirty`"后清零 |
| `_glassDirty` | 位置缓存失效、显式标记、`_markGlassAndDependents`、`_markGlassesIntersecting` | 每帧快照+清空 |
| `_glassContentDirty` | 玻璃子树 mutation、尺寸变化、resize、data-config 变化 | 每帧快照 → 异步 `_captureGlassContent` |
| `_userMarkedChanged` | 公开 API `markChanged(el)` | 每帧 fan-out 成交集玻璃脏 |
| `renderedThisFrame` | 本帧真正渲染过的玻璃（带 rect） | 供 z 序更后的玻璃判断"下方玻璃刚变过 → 我也要重绘" |

位置缓存阈值：中心位移 > **0.5 px** 视为移动。

### 4.6 生命周期与初始化顺序（`_start`）

1. root 设 `user-select:none`（含 `-webkit-`）；
2. `_setupGlassElements()`：非 root 直接子元素的玻璃**警告并剔除**；`position:static` → 改成 `relative`；`overflow:visible`；`floating` → `touch-action:none`；`button` → 加 class + 监听器；**把 canvas 插到元素第一个子节点**（`position:absolute;inset:0;width:100%;height:100%;pointer-events:none;z-index:-1`）；
3. `_detectDynamic()`；`_getSortedChildren()`；`_handleResize()`；
4. `await capture.prefetchFontEmbedCSS()`（全页面 @font-face → base64 内联，模块级共享缓存）；
5. `await _captureGlassContent()`（所有玻璃内容图）；
6. `await _prewarmStaticCaptures()`（逐个非玻璃子元素 html-to-image 预热，跳过 CANVAS/IMG/VIDEO 与 `data-dynamic`）；
7. 注册 resize / pointer 监听、两个 MutationObserver（root `childList`；每个玻璃子树 `childList+subtree+characterData+attributes(data-config)`）；
8. `_running = true; _globalDirty = true;` 启动 rAF 循环。

`destroy()`：停循环、移除监听、断开 observer、删除注入 canvas、还原 `position/overflow/touch-action` 与 class、清所有 Map、移除注入 style、销毁 capture 与 renderer（删 texture/buffer/program、移除 canvas）。
已知缺陷：`destroy()` **不会**把被改成 `relative` 的 `position` 还原成 `static`。

---

## 5. 明暗模式 / 无障碍适配

**结论：库里没有任何明暗模式或无障碍适配代码。** 具体核查：

| 检查项 | 结果 |
|---|---|
| `prefers-color-scheme` | 源码中 **0 处**。唯一出现 "dark" 的地方是官网 demo 的 `.liquid-glass-preview-dark` 类名，其做法只是 `data-config` 里设 `brightness: -0.3` |
| `prefers-reduced-transparency` | **0 处**，无 `backdrop-filter` 降级路径 |
| `prefers-reduced-motion` | **0 处**。不过库本身也没有动画（唯一的连续运动是拖拽），所以实际影响小 |
| `forced-colors` / `prefers-contrast` | **0 处** |
| ARIA / 焦点管理 / 键盘操作 | **0 处**。`button:true` 只是视觉反馈 + `cursor:pointer`，不设 `role`/`tabindex`/键盘事件 |
| 降级（无 WebGL） | 只有 `throw new Error('LiquidGlass: WebGL is not supported in this browser.')`，无优雅降级 |
| 无障碍相关副作用 | ① root 被设 `user-select:none`（会阻止选中文本）；② `floating` 元素设 `touch-action:none`（会阻止触摸滚动）；③ 玻璃元素被强制 `overflow:visible` 与可能的 `position:relative` |

"暗色玻璃"在官方 demo 里的实现方式（可照抄给别的栈）：`brightness: -0.3` + `blurAmount: 0.25`（+ 可选 `tintStrength`）。

---

## 6. README 里容易被漏掉的部分

README 的选项表与源码一致（无缺项），但下面这些**只在正文/限制章节**出现，容易漏：

1. **`LiquidGlass.init()` 是 async 且"重"的**：必须等字体预取 + 玻璃内容预捕获 + 静态内容预热全部完成，新页面通常 100–500ms。复刻时若同步初始化会闪一帧白。
2. **字体必须在 `init()` 之前加载**，且 CORS 友好；否则捕获栅格用系统字体，导致玻璃下的折射文字与真实 DOM 字形不重合。动态加载字体后要调 `invalidateFontEmbedCache()` 并重新 init。
3. **跨域 `<img>` 必须带 `crossorigin="anonymous"`**，否则画布被污染，整个 root 的玻璃效果失效。
4. **root 自身永远不会被捕获**：root 的背景图/内边距/边框对玻璃不可见，必须把背景放进 root 内部的兄弟元素。
5. **玻璃元素必须是 root 的直接子元素**（嵌套会被警告剔除；需要嵌套就再起一个实例）。
6. **不同实例之间无法共享折射**（各自独立的合成画布）。
7. **阴影光晕会溢出玻璃元素盒子**，任何祖先的 `overflow:hidden` 都会裁掉它——所以注入的 canvas 才要 `-20px` 偏移并放大 40px。
8. **`:first-child` 选择器会命中注入的 canvas**（它是玻璃元素的第一个子节点），要避免。
9. **`data-dynamic` 只对 root 的直接子元素有效**；wrapper 内部的动态内容不会被自动检测（要么给 wrapper 加 `data-dynamic`，要么调 `markChanged()`）。
10. **`markChanged(el)` 的语义是"与 el 相交的玻璃"**（不是 el 内部），无参调用才标记全部；对 `data-dynamic` 元素调用是安全的 no-op。
11. **每个实例一个 WebGL 上下文**，浏览器上限通常 16 个，不要开几十个实例。
12. **窗口 resize 会重捕获全部内容**，不要在 resize 循环里驱动布局。
13. **静置页面几乎零开销**：无 video、无 `data-dynamic` 时渲染循环会整体短路（`needsRender` 为 false 直接 return）。
14. **性能提示**：`data-dynamic` 是唯一能击穿按元素脏追踪的机制，要少用；wrapper 越浅越小越好（html-to-image 的成本在样式内联 + SVG foreignObject 解码）。
15. **暴露的 API 只有三个**：`LiquidGlass.init()`、实例上的 `.fps` / `.destroy()` / `.markChanged(el?)`、以及模块级 `invalidateFontEmbedCache()`。**没有** `setConfig()`、`add()`/`remove()`、事件回调、`on('render')` 之类的钩子——改配置只能改 `data-config`（MutationObserver 会自动重读）。
16. **`data-config` 必须是 JSON 对象**；非法 JSON 或非对象只打印警告并被忽略。
17. **`destroy()` 不还原 `position`**（见 4.6）。
18. README 未给出 `refraction / chromAberration / edgeHighlight / zRadius / shadowSpread / shadowOffsetY` 的推荐区间——可参考官方 demo 的实际取值：`refraction` 0.69–1.2、`chromAberration` 0.05–0.2、`edgeHighlight` 0.05–0.2、`blurAmount` 0–0.5、`zRadius` 常与 `cornerRadius` 相等（放大镜场景）。

---

## 7. 给"别的渲染栈"的复刻检查清单

1. **输入纹理自备**：原库靠"Canvas2D 重绘 DOM 子元素"造纹理。若目标栈已有场景纹理（如 GPU 场景图、屏幕后缓冲），可直接跳过整章第 2.8 节的 DOM 栅格化，但必须保留 **"只采样该面板之前的层"** 这一约束，才能得到分层折射。
2. **两个纹理版本**：清晰版 + 模糊版（6×(H+V) 9-tap，半径 ≈ 60×blurAmount 像素，无降采样）。
3. **几何**：面板四边形带 20px（×dpr）padding，片元局部坐标以中心为原点，`half = size/2`。
4. **22 个 uniform** 一个都不能少，尤其 `u_pad`、`u_res`（含 padding 的裁剪区尺寸）、`u_center`。
5. **两个分支**：`bevelMode<0.5` 双凸（双面折射 + 向心项）与穹顶（纯向心收缩）。
6. **顺序不能乱**：投影早退 → mask → edge/depth → 法线 → 折射 → 噪声 → 色散 → 双纹理采样 → 亮度 → 饱和度 → 染色 → Fresnel → 高光 → 内描边/rim/glow → 环境反射 → 白化 → 输出 alpha。
7. **alpha 语义**：非预乘，`alpha = mask * opacity`，标准 SRC_ALPHA 混合；光效被 alpha 一起缩放。
8. **不需要实现的**：metaball/morph/形变动画、明暗模式、`prefers-reduced-transparency`、降采样模糊、多趟合成链——原库都没有。
