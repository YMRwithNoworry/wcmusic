//! 液态玻璃（liquid glass）。
//!
//! ## 基于什么实现
//!
//! 参照目标是 <https://github.com/ybouane/liquidglass>（TypeScript + WebGL1）。
//! 它的做法是：用 Canvas2D 把「z 序在玻璃之前的兄弟元素」重绘成场景画布 → 上传纹理 →
//! 6 趟横向 + 6 趟纵向的 9-tap 高斯模糊 → 一个片元着色器完成圆角矩形 SDF、双凸倒角高度场、
//! 双面折射（IOR 1.5）、色散、Fresnel、4 光源高光、内描边、双高斯投影与边缘加权混合。
//! 逐行规格见仓库根目录的 `liquidglass-复刻规格书.md`。
//!
//! 本项目是 Rust + GPUI（wgpu / DirectX 12）原生渲染：没有 DOM、没有 WebGL，GPUI 也不暴露
//! 「读回帧缓冲」与「自定义着色器」。但着色器是纯逐像素数学，而本页玻璃背后的内容是确定的
//! （一张封面铺满整页 + 一层压暗渐变），所以这里把整条管线**逐像素移植到 CPU**：
//!
//! * 场景纹理 = 面板裁剪区 `(w+2*pad) × (h+2*pad)` 的**全分辨率**像素，`pad = 20`；
//! * 模糊 = 参照目标的**精确核**（9-tap，权重 0.227027/0.194594/…，步长 `blurAmount*2.5`，
//!   6 趟横向 + 6 趟纵向）；
//! * 着色 = `FS_GLASS` 逐行翻译，包括只在 `sdf > 0` 时走的**双高斯投影**分支；
//! * 分层合成 = 把 z 序在前的玻璃面板的渲染结果先合成进场景，再采样。
//!
//! 于是折射、色散、模糊、Fresnel、高光、内描边、投影全都按原公式逐像素成立。
//!
//! ## 与参照目标的差异
//!
//! 见仓库根目录的 `液态玻璃-差异清单.md`（逐项比对表）。
//!
//! ## 在参照目标之上多出来的能力（不是「没还原」）
//!
//! 参照目标**没有**这些（已核对源码：`prefers-color-scheme` / `prefers-reduced-transparency`
//! 均 0 处，无 metaball / morph）：
//!
//! * 明暗模式：同一套公式按主题选一组配置（参照目标靠调用方手填 `brightness: -0.3`）；
//! * 无障碍「降低透明度」：读 Windows 的「透明效果」开关，关闭时少模糊、无色散、更不透明；
//! * 形状弹簧形变：换歌/尺寸变化时玻璃先缩后弹（参照目标唯一的形变是 button 按下瞬时跳变）。

use std::sync::{Arc, Mutex};

use gpui_kit as gpui;

use gpui::{
    Bounds, Corners, Div, Hsla, RenderImage, Styled as _, canvas, div, img,
    linear_color_stop, linear_gradient, prelude::*, px,
};
use image::{Frame, ImageBuffer, Rgba, RgbaImage};
use smallvec::SmallVec;

/// 面板在页面里的位置与尺寸（CSS px）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 参照目标的 19 项配置（`src/defaults.ts`）。
#[derive(Clone, Copy, Debug)]
pub struct LensConfig {
    /// 背景模糊强度 0..1：每趟步长 `blurAmount*2.5` px，6 趟往返等效半径 ≈ `60*blurAmount`。
    pub blur_amount: f32,
    /// 折射强度 0..1.5。
    pub refraction: f32,
    /// 边缘色散 0..0.3。
    pub chrom_aberration: f32,
    /// 边缘辉光 + rim + 内描边总强度 0..0.25。
    pub edge_highlight: f32,
    /// 4 光源高光 0..1。
    pub specular: f32,
    /// 掠射反射 0..1.5。
    pub fresnel: f32,
    /// 逐像素噪声 0..0.2。
    pub distortion: f32,
    /// 圆角半径（CSS px）。
    pub corner_radius: f32,
    /// 倒角深度（CSS px），高度场的 z 半径。
    pub z_radius: f32,
    /// 整体不透明度 0..1（只乘 mask）。
    pub opacity: f32,
    /// 饱和度 -1..1。
    pub saturation: f32,
    /// 冷蓝染色 0..1。
    pub tint_strength: f32,
    /// 亮度偏移 -0.5..0.5。
    pub brightness: f32,
    /// 投影不透明度 0..1。
    pub shadow_opacity: f32,
    /// 投影扩散。
    pub shadow_spread: f32,
    /// 投影 Y 偏移。
    pub shadow_offset_y: f32,
    /// 这块玻璃能不能拖动（参照目标的 `floating`：Pointer Events 拖拽）。
    pub floating: bool,
    /// 这块玻璃是不是按钮（参照目标的 `button`：hover 抬升 + 按下效果）。
    pub button: bool,
    /// 0 = 双凸药丸（双面折射）；1 = 穹顶（单面 + 放大镜）。
    pub bevel_mode: u8,
}

impl Default for LensConfig {
    /// 参照目标的默认值。
    fn default() -> Self {
        Self {
            blur_amount: 0.0,
            refraction: 0.69,
            chrom_aberration: 0.05,
            edge_highlight: 0.05,
            specular: 0.0,
            fresnel: 1.0,
            distortion: 0.0,
            corner_radius: 65.0,
            z_radius: 40.0,
            opacity: 1.0,
            saturation: 0.0,
            tint_strength: 0.0,
            brightness: 0.0,
            shadow_opacity: 0.30,
            shadow_spread: 10.0,
            shadow_offset_y: 1.0,
            floating: false,
            button: false,
            bevel_mode: 0,
        }
    }
}

/// 详情页面板统一的圆角半径（四角一致，与参照目标的 `cornerRadius` 对应）。
pub const PANEL_RADIUS: f32 = 22.0;

impl LensConfig {
    /// 详情页用的参数：取官网 demo 的区间，并让玻璃真的「看得出是玻璃」。
    ///
    /// 这里只改配置值，不改公式——同一组配置下与参照目标逐像素一致。
    pub fn for_theme(dark: bool, reduced: bool) -> Self {
        let mut config = Self {
            blur_amount: 0.32,
            refraction: 1.05,
            chrom_aberration: 0.10,
            edge_highlight: 0.16,
            specular: 0.55,
            fresnel: 1.0,
            distortion: 0.0,
            corner_radius: PANEL_RADIUS,
            z_radius: PANEL_RADIUS * 1.6,
            opacity: 1.0,
            saturation: -0.12,
            tint_strength: 0.38,
            brightness: if dark { -0.62 } else { 0.30 },
            shadow_opacity: 0.42,
            shadow_spread: 26.0,
            shadow_offset_y: 14.0,
            // 详情页的两块面板可以拖动（对应参照目标里给元素加 `floating`）。
            floating: true,
            // 它们不是按钮，所以不吃 hover/pressed（与参照目标的默认一致）。
            button: false,
            bevel_mode: 0,
        };
        if reduced {
            // 「降低透明度」：少模糊、不色散、提亮/压暗到能直接当纯色面用。
            config.blur_amount = 0.10;
            config.chrom_aberration = 0.0;
            config.refraction *= 0.5;
            config.brightness += if dark { -0.22 } else { 0.20 };
            config.saturation *= 0.4;
        }
        config
    }
}

// ---------------------------------------------------------------- 基础数学

/// 参照目标里的常量。
const IOR: f32 = 1.5;
/// `1 - 1/IOR`。
const REFR_POW: f32 = 1.0 - 1.0 / IOR;
/// 内描边宽度（设备像素）。
const BORDER_WIDTH: f32 = 1.5;
/// 法线的中心差分步长（设备像素）。
const NORMAL_STEP: f32 = 2.0;
/// 面板四周为投影预留的留白（参照目标 `SHADOW_PAD`）。
pub const SHADOW_PAD: f32 = 20.0;
/// `floating` 拖拽的边界约束：不允许拖出 root 内缩这么多像素（参照目标 `margin = 10`）。
const DRAG_MARGIN: f32 = 10.0;
/// 模糊趟数（参照目标 `BLUR_ITERATIONS`）：横 6 趟 + 纵 6 趟。
const BLUR_ITERATIONS: usize = 6;
/// 每趟步长系数：`spread = blurAmount * 2.5` 像素。
const BLUR_SPREAD_SCALE: f32 = 2.5;
/// 9-tap 高斯核权重（中心 tap 与两侧对称 tap）。
const BLUR_TAPS: [f32; 5] = [0.227027, 0.194594, 0.121622, 0.054054, 0.016216];

#[derive(Clone, Copy, Debug, Default)]
struct Vec2 {
    x: f32,
    y: f32,
}

impl Vec2 {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if value < edge0 { 0.0 } else { 1.0 };
    }
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn mix(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// 圆角矩形有向距离场：内部为负、外部为正（对应着色器的 `rrSDF`）。
fn rounded_rect_sdf(point: Vec2, half: Vec2, radius: f32) -> f32 {
    let radius = radius.min(half.x).min(half.y).max(0.0);
    let qx = point.x.abs() - half.x + radius;
    let qy = point.y.abs() - half.y + radius;
    let outside = Vec2::new(qx.max(0.0), qy.max(0.0)).length();
    qx.max(qy).min(0.0) + outside - radius
}

/// 倒角高度场：半径 `z_radius` 的圆截面（对应 `bevelHeight`）。
fn bevel_height(depth: f32, z_radius: f32) -> f32 {
    if depth <= 0.0 {
        return 0.0;
    }
    if depth >= z_radius {
        return z_radius;
    }
    (depth * (2.0 * z_radius - depth)).max(0.0).sqrt()
}

/// 着色器里的 `hash`：`fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453)`。
fn hash21(point: Vec2) -> f32 {
    let value = (point.x * 127.1 + point.y * 311.7).sin() * 43758.5453;
    value - value.floor()
}

fn normalize3(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let length = (x * x + y * y + z * z).sqrt();
    if length <= f32::EPSILON {
        (0.0, 0.0, 1.0)
    } else {
        (x / length, y / length, z / length)
    }
}

fn dot3(ax: f32, ay: f32, az: f32, bx: f32, by: f32, bz: f32) -> f32 {
    ax * bx + ay * by + az * bz
}

/// 参照目标的 4 光源高光。
///
/// 注意第 3 个光源用的是 `N·L` 而不是半程向量——原着色器就是这么写的，照抄。
fn specular_highlight(nx: f32, ny: f32, nz: f32) -> f32 {
    // 视线方向 V = (0, 0, 1)，所以 H = normalize(L + V)。
    let (h1x, h1y, h1z) = normalize3(0.4, 0.7, 1.0 + 1.0);
    let sp1 = dot3(nx, ny, nz, h1x, h1y, h1z).max(0.0).powi(90);
    let (h2x, h2y, h2z) = normalize3(-0.3, -0.5, 1.0 + 1.0);
    let sp2 = dot3(nx, ny, nz, h2x, h2y, h2z).max(0.0).powi(50) * 0.3;
    let (l3x, l3y, l3z) = normalize3(0.1, 0.3, 1.0);
    let sp3 = dot3(nx, ny, nz, l3x, l3y, l3z).max(0.0).powi(6) * 0.1;
    let (h4x, h4y, h4z) = normalize3(0.0, 0.9, 0.4 + 1.0);
    let sp4 = dot3(nx, ny, nz, h4x, h4y, h4z).max(0.0).powi(120) * 0.6;
    sp1 + sp2 + sp3 + sp4
}

fn to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// 可用的并行度（逐像素着色与模糊都按这个切分）。
fn worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

// ---------------------------------------------------------------- 场景

/// 玻璃背后的一张「封面元素」：页面上独立绘制的那张封面（同一个文件，另一个矩形 + 圆角）。
#[derive(Clone, Copy)]
pub struct SceneArtwork {
    /// 页面坐标里的矩形。
    pub rect: PanelRect,
    /// 圆角半径（页面上是 `rounded(..) + overflow_hidden`）。
    pub radius: f32,
}

/// 玻璃面板背后的场景。
///
/// 参照目标用 Canvas2D 把「z 序在玻璃之前的兄弟元素」重绘进场景画布：`<img>/<video>/<canvas>`
/// 走 `drawImage` 快路径，其余元素走 html-to-image 快照。本页玻璃背后的可重建内容是确定的——
/// 一张封面按 `object-fit: cover` 铺满整页，再叠一层从上到下的压暗渐变，以及页面上那张独立
/// 绘制的封面元素——所以这里直接按同样的公式算。
///
/// 文字与歌词**无法**进场景：GPUI 运行期不提供把元素光栅化成纹理的能力（见差异清单第四节）。
#[derive(Clone)]
pub struct Scene {
    cover: Option<Arc<RgbaImage>>,
    base: [f32; 3],
    cover_opacity: f32,
    shade_top: f32,
    shade_bottom: f32,
    page_width: f32,
    page_height: f32,
    /// 页面上那张封面元素的矩形（同一个文件）。
    artwork: Option<SceneArtwork>,
}

impl Scene {
    /// `base` 是主题底色，`cover_opacity` 与 `shade_*` 对应页面上的封面层与压暗渐变。
    pub fn new(
        cover: Option<Arc<RgbaImage>>,
        base: [f32; 3],
        cover_opacity: f32,
        shade_top: f32,
        shade_bottom: f32,
        page_width: f32,
        page_height: f32,
    ) -> Self {
        Self {
            cover,
            base,
            cover_opacity,
            shade_top,
            shade_bottom,
            page_width: page_width.max(1.0),
            page_height: page_height.max(1.0),
            artwork: None,
        }
    }

    /// 把页面上那张封面元素也纳入场景（它用同一个文件，铺在自己的矩形里）。
    pub fn with_artwork(mut self, artwork: Option<SceneArtwork>) -> Self {
        self.artwork = artwork;
        self
    }

    /// 场景画布上某个页面坐标处的颜色。
    pub fn sample(&self, page_x: f32, page_y: f32) -> [f32; 3] {
        let mut color = self.base;
        if let Some(cover) = &self.cover {
            let u = page_x / self.page_width;
            let v = 1.0 - page_y / self.page_height;
            let fitted = self.cover_color(cover, u, v);
            for channel in 0..3 {
                color[channel] = mix(color[channel], fitted[channel], self.cover_opacity);
            }
        }
        // 封面元素：同一个文件按 `object-fit: cover` 铺进它自己的矩形，带圆角遮罩。
        if let (Some(artwork), Some(cover)) = (&self.artwork, &self.cover) {
            let local_x = page_x - artwork.rect.x;
            let local_y = page_y - artwork.rect.y;
            if local_x >= 0.0
                && local_y >= 0.0
                && local_x < artwork.rect.width
                && local_y < artwork.rect.height
            {
                let half = Vec2::new(artwork.rect.width * 0.5, artwork.rect.height * 0.5);
                let centre = Vec2::new(local_x - half.x, local_y - half.y);
                if rounded_rect_sdf(centre, half, artwork.radius) <= 0.0 {
                    color = cover_fit(
                        cover,
                        local_x,
                        local_y,
                        artwork.rect.width,
                        artwork.rect.height,
                    );
                }
            }
        }
        let t = (page_y / self.page_height).clamp(0.0, 1.0);
        let shade = mix(self.shade_top, self.shade_bottom, t);
        for channel in 0..3 {
            color[channel] *= 1.0 - shade;
        }
        color
    }

    /// 封面按 `object-fit: cover` 铺满页面后，归一化坐标 `(u, v)` 处取到的颜色。
    fn cover_color(&self, cover: &RgbaImage, u: f32, v: f32) -> [f32; 3] {
        cover_fit(
            cover,
            u * self.page_width,
            (1.0 - v) * self.page_height,
            self.page_width,
            self.page_height,
        )
    }
}

/// 图片按 `object-fit: cover` 铺满 `width × height` 的矩形后，矩形内局部坐标 `(x, y)` 处的颜色。
fn cover_fit(image: &RgbaImage, x: f32, y: f32, width: f32, height: f32) -> [f32; 3] {
    let image_width = image.width().max(1) as f32;
    let image_height = image.height().max(1) as f32;
    let scale = (width / image_width).max(height / image_height);
    let scaled_width = image_width * scale;
    let scaled_height = image_height * scale;
    let source_x = (x + (scaled_width - width) * 0.5) / scale;
    let source_y = (y + (scaled_height - height) * 0.5) / scale;
    bilinear_image(
        image,
        source_x.clamp(0.0, image_width - 1.0),
        source_y.clamp(0.0, image_height - 1.0),
    )
}
/// 图片的双线性采样。
fn bilinear_image(image: &RgbaImage, x: f32, y: f32) -> [f32; 3] {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let x0 = (x.floor() as usize).min(width - 1);
    let y0 = (y.floor() as usize).min(height - 1);
    let x1 = (x0 + 1).min(width - 1);
    let y1 = (y0 + 1).min(height - 1);
    let fx = x - x0 as f32;
    let fy = y - y0 as f32;
    let raw = image.as_raw();
    let at = |px: usize, py: usize| -> [f32; 3] {
        let index = (py * width + px) * 4;
        [
            raw[index] as f32 / 255.0,
            raw[index + 1] as f32 / 255.0,
            raw[index + 2] as f32 / 255.0,
        ]
    };
    let top = at(x0, y0);
    let bottom = at(x0, y1);
    let top_right = at(x1, y0);
    let bottom_right = at(x1, y1);
    let mut out = [0.0f32; 3];
    for channel in 0..3 {
        let a = mix(top[channel], top_right[channel], fx);
        let b = mix(bottom[channel], bottom_right[channel], fx);
        out[channel] = mix(a, b, fy);
    }
    out
}

// ---------------------------------------------------------------- 裁剪区纹理

/// 一块面板的裁剪区纹理：清晰版与模糊版，尺寸都是 `(w+2*pad) × (h+2*pad)`（设备像素）。
struct CropTextures {
    sharp: Vec<f32>,
    blur: Vec<f32>,
    width: usize,
    height: usize,
}

impl CropTextures {
    /// 双线性采样；`u`/`v` 是裁剪区的归一化坐标（v 向上为正，与着色器一致）。
    fn sample_bilinear(pixels: &[f32], width: usize, height: usize, u: f32, v: f32) -> [f32; 3] {
        let x = (u * width as f32 - 0.5).clamp(0.0, width as f32 - 1.0);
        let y = ((1.0 - v) * height as f32 - 0.5).clamp(0.0, height as f32 - 1.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(width - 1);
        let y1 = (y0 + 1).min(height - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let at = |px: usize, py: usize| -> [f32; 3] {
            let index = (py * width + px) * 3;
            [pixels[index], pixels[index + 1], pixels[index + 2]]
        };
        let top = at(x0, y0);
        let bottom = at(x0, y1);
        let top_right = at(x1, y0);
        let bottom_right = at(x1, y1);
        let mut out = [0.0f32; 3];
        for channel in 0..3 {
            let a = mix(top[channel], top_right[channel], fx);
            let b = mix(bottom[channel], bottom_right[channel], fx);
            out[channel] = mix(a, b, fy);
        }
        out
    }

    fn sample_sharp(&self, u: f32, v: f32) -> [f32; 3] {
        Self::sample_bilinear(&self.sharp, self.width, self.height, u, v)
    }

    /// 模糊版已经很平滑，最近邻与双线性在视觉上没有区别，但快一倍。
    fn sample_blur(&self, u: f32, v: f32) -> [f32; 3] {
        let x = ((u * self.width as f32) as isize).clamp(0, self.width as isize - 1) as usize;
        let y = (((1.0 - v) * self.height as f32) as isize).clamp(0, self.height as isize - 1)
            as usize;
        let index = (y * self.width + x) * 3;
        [
            self.blur[index],
            self.blur[index + 1],
            self.blur[index + 2],
        ]
    }
}

/// 参照目标的精确模糊：`BLUR_ITERATIONS` 趟横向 + 同样多趟纵向，每趟 9-tap 高斯，
/// 步长 `spread = blurAmount * 2.5` 像素。**没有降采样**，纹理与场景同尺寸。
///
/// 横纵两趟可交换，所以这里把 6 趟横向连着做完，再转置一次、把纵向也变成横向做完，
/// 最后转置回来：结果与参照目标的 H/V 交替完全一致（线性算子可交换），
/// 但避免了跨列访问造成的 cache miss —— 实测这一步值 4 倍。
fn blur_exact(pixels: &mut [f32], width: usize, height: usize, spread: f32) {
    if spread <= 0.0 || width == 0 || height == 0 {
        return;
    }
    let mut scratch = vec![0.0f32; pixels.len()];
    for _ in 0..BLUR_ITERATIONS {
        blur_rows(pixels, &mut scratch, width, height, spread);
        blur_rows(&scratch, pixels, width, height, spread);
    }
    let mut transposed = vec![0.0f32; pixels.len()];
    transpose(pixels, &mut transposed, width, height);
    for _ in 0..BLUR_ITERATIONS {
        blur_rows(&transposed, &mut scratch, height, width, spread);
        blur_rows(&scratch, &mut transposed, height, width, spread);
    }
    transpose(&transposed, pixels, height, width);
}

/// 一趟横向 9-tap 高斯。每个输出像素只读源纹理，所以能按行并行。
///
/// 采样只用横向的线性插值（纵向取整），比双线性少一半读取；核对称，正负两侧共用权重。
///
/// `spread` 对整趟是固定的，所以每个 tap 的整数偏移与小数部分是常量：内部像素走
/// 「两次读 + 一次插值」的快速路径，只有每行两端约 `4*spread` 列需要按 `CLAMP_TO_EDGE`
/// 走慢路径。两条路径的算式完全一致（同样的 `i0` / `i1` / `f`），结果逐位相同。
fn blur_rows(source: &[f32], target: &mut [f32], width: usize, height: usize, spread: f32) {
    let row_bytes = width * 3;
    // 每个 tap 的整数偏移与两个方向的小数部分（对整行都一样）。
    let mut taps = [(0usize, 0.0f32, 0.0f32); BLUR_TAPS.len() - 1];
    for (index, tap) in taps.iter_mut().enumerate() {
        let offset = spread * (index + 1) as f32;
        let lower = offset.floor();
        *tap = (lower as usize, offset - lower, 1.0 - (offset - lower));
    }
    // 这些列上所有 tap 的索引都不会越界。
    let margin = (spread * (BLUR_TAPS.len() - 1) as f32).ceil() as usize + 1;
    let rows_per_chunk = height.div_ceil(worker_count()).max(1);
    std::thread::scope(|scope| {
        let mut first_row = 0usize;
        for chunk in target.chunks_mut(rows_per_chunk * row_bytes) {
            let start_row = first_row;
            first_row += chunk.len() / row_bytes;
            scope.spawn(move || {
                for (index, out_row) in chunk.chunks_mut(row_bytes).enumerate() {
                    let y = start_row + index;
                    let row = y * width;
                    for x in 0..width {
                        let mut sum = [0.0f32; 3];
                        let centre = (row + x) * 3;
                        for channel in 0..3 {
                            sum[channel] = source[centre + channel] * BLUR_TAPS[0];
                        }
                        if x >= margin && x + margin < width {
                            for (tap_index, (lower, forward, backward)) in taps.iter().enumerate() {
                                let weight = BLUR_TAPS[tap_index + 1];
                                let ahead = (row + x + lower) * 3;
                                let behind = (row + x - lower - 1) * 3;
                                for channel in 0..3 {
                                    let positive = mix(
                                        source[ahead + channel],
                                        source[ahead + channel + 3],
                                        *forward,
                                    );
                                    let negative = mix(
                                        source[behind + channel],
                                        source[behind + channel + 3],
                                        *backward,
                                    );
                                    sum[channel] += (positive + negative) * weight;
                                }
                            }
                        } else {
                            for tap in 1..BLUR_TAPS.len() {
                                let offset = spread * tap as f32;
                                let positive = sample_row(source, row, width, x as f32 + offset);
                                let negative = sample_row(source, row, width, x as f32 - offset);
                                let weight = BLUR_TAPS[tap];
                                for channel in 0..3 {
                                    sum[channel] += (positive[channel] + negative[channel]) * weight;
                                }
                            }
                        }
                        let base = x * 3;
                        out_row[base] = sum[0];
                        out_row[base + 1] = sum[1];
                        out_row[base + 2] = sum[2];
                    }
                }
            });
        }
    });
}
/// 在一行内做线性插值采样 + `CLAMP_TO_EDGE`。
#[inline]
fn sample_row(source: &[f32], row: usize, width: usize, x: f32) -> [f32; 3] {
    let clamped = x.clamp(0.0, width as f32 - 1.0);
    let i0 = clamped.floor() as usize;
    let i1 = (i0 + 1).min(width - 1);
    let f = clamped - i0 as f32;
    let a = (row + i0) * 3;
    let b = (row + i1) * 3;
    [
        mix(source[a], source[b], f),
        mix(source[a + 1], source[b + 1], f),
        mix(source[a + 2], source[b + 2], f),
    ]
}

/// 转置 `width × height` 的 RGB 缓冲。
///
/// 按源行分块（`BLOCK` 行一组）再逐列搬运：这样内层循环读到的 `BLOCK` 行会一直留在
/// L1 里，避开了「读一列、跨 11KB 步长」那种每次都是 cache miss 的写法。
fn transpose(source: &[f32], target: &mut [f32], width: usize, height: usize) {
    const BLOCK: usize = 32;
    let source_row_bytes = width * 3;
    let target_row_bytes = height * 3;
    let rows_per_chunk = width.div_ceil(worker_count()).max(1);
    std::thread::scope(|scope| {
        let mut first_column = 0usize;
        for chunk in target.chunks_mut(rows_per_chunk * target_row_bytes) {
            let start_column = first_column;
            let columns = chunk.len() / target_row_bytes;
            first_column += columns;
            scope.spawn(move || {
                for block_start in (0..height).step_by(BLOCK) {
                    let block_end = (block_start + BLOCK).min(height);
                    for column in 0..columns {
                        let x = start_column + column;
                        let out_row =
                            &mut chunk[column * target_row_bytes..(column + 1) * target_row_bytes];
                        for y in block_start..block_end {
                            let source_index = y * source_row_bytes + x * 3;
                            let target_index = y * 3;
                            out_row[target_index] = source[source_index];
                            out_row[target_index + 1] = source[source_index + 1];
                            out_row[target_index + 2] = source[source_index + 2];
                        }
                    }
                }
            });
        }
    });
}
// ---------------------------------------------------------------- 着色器移植

/// 一块玻璃面板渲染出来的像素（覆盖面板 + 四周 20px 投影留白）。
///
/// 字节序是 **BGRA**：GPUI 的 `RenderImage` 要求这个顺序（见 `decode_static_image_from_decoder`
/// 里的 `pixel.swap(0, 2)`）。
pub struct PanelImage {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl PanelImage {
    /// 取某个像素的 RGB（测试与离线比对用）。
    #[cfg(test)]
    pub fn rgb(&self, x: u32, y: u32) -> [u8; 3] {
        let index = ((y * self.width + x) * 4) as usize;
        [self.bgra[index + 2], self.bgra[index + 1], self.bgra[index]]
    }

    /// 取某个像素的透明度。
    #[cfg(test)]
    pub fn alpha(&self, x: u32, y: u32) -> u8 {
        self.bgra[((y * self.width + x) * 4 + 3) as usize]
    }

    /// 转成 GPUI 的 `RenderImage`。
    pub fn to_render_image(&self) -> Arc<RenderImage> {
        let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width, self.height, self.bgra.clone())
                .expect("面板像素数与尺寸一致");
        Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)))
    }
}

/// 分层合成用的一层：z 序在本面板之前的玻璃，连同它已经渲染好的位图。
#[derive(Clone)]
pub struct LowerLayer {
    pub rect: PanelRect,
    pub image: Arc<RenderImage>,
}

/// 渲染一块玻璃面板（对应参照目标的 `_composeSceneForGlass` + `uploadAndBlur` +
/// `renderGlassPanel` 三步）。
///
/// 输出覆盖 `panel` 加上四周 [`SHADOW_PAD`] 的留白——投影就画在这圈留白里，
/// 与参照目标把阴影交给同一个片元着色器完全一致。
pub fn render_panel(
    scene: &Scene,
    panel: PanelRect,
    config: &LensConfig,
    lower: &[LowerLayer],
    dpr: f32,
) -> PanelImage {
    // 设 `WCMUSIC_GLASS_PROFILE=1` 会把三段耗时打到 stderr（调优用）。
    let profile_start = std::time::Instant::now();
    // 参照目标整条管线都跑在**设备像素**上（uniform 全部乘过 dpr），这里照做：
    // 面板尺寸与 20px 留白都按 dpr 放大，输出位图的分辨率随之提高。
    let dpr = dpr.clamp(0.5, 4.0);
    let pad = SHADOW_PAD * dpr;
    let width = ((panel.width * dpr + pad * 2.0).round() as usize).max(1);
    let height = ((panel.height * dpr + pad * 2.0).round() as usize).max(1);

    // ① 场景光栅化：裁剪区里每个像素取一次场景颜色。
    let mut sharp = vec![0.0f32; width * height * 3];
    {
        let row_bytes = width * 3;
        let rows_per_chunk = height.div_ceil(worker_count()).max(1);
        let scene = scene.clone();
        std::thread::scope(|scope| {
            let mut first_row = 0usize;
            for chunk in sharp.chunks_mut(rows_per_chunk * row_bytes) {
                let start_row = first_row;
                first_row += chunk.len() / row_bytes;
                let scene = scene.clone();
                scope.spawn(move || {
                    for (index, out_row) in chunk.chunks_mut(row_bytes).enumerate() {
                        let y = start_row + index;
                        let page_y = panel.y - SHADOW_PAD + (y as f32 + 0.5) / dpr;
                        for x in 0..width {
                            let page_x = panel.x - SHADOW_PAD + (x as f32 + 0.5) / dpr;
                            let color = scene.sample(page_x, page_y);
                            out_row[x * 3] = color[0];
                            out_row[x * 3 + 1] = color[1];
                            out_row[x * 3 + 2] = color[2];
                        }
                    }
                });
            }
        });
    }

    let raster_done = profile_start.elapsed();

    // ② 分层合成：把 z 序在前的玻璃的最终像素（含它的阴影与内容）叠进场景。
    for layer in lower {
        composite_lower(&mut sharp, width, height, panel, dpr, layer);
    }

    // ③ 模糊：参照目标的精确核。
    let mut blur = sharp.clone();
    blur_exact(
        &mut blur,
        width,
        height,
        config.blur_amount * BLUR_SPREAD_SCALE,
    );
    let blur_done = profile_start.elapsed();
    let textures = CropTextures {
        sharp,
        blur,
        width,
        height,
    };

    // ④ 着色。
    let out = shade_panel(&textures, panel, config, dpr);
    if std::env::var_os("WCMUSIC_GLASS_PROFILE").is_some() {
        let total = profile_start.elapsed();
        eprintln!(
            "[glass] 并行度 {} / {}x{} 光栅 {:.1}ms 模糊 {:.1}ms 着色 {:.1}ms 合计 {:.1}ms",
            worker_count(),
            width,
            height,
            raster_done.as_secs_f32() * 1000.0,
            (blur_done - raster_done).as_secs_f32() * 1000.0,
            (total - blur_done).as_secs_f32() * 1000.0,
            total.as_secs_f32() * 1000.0,
        );
    }
    out
}

/// 把一层下层玻璃的位图按 alpha 叠进场景缓冲。
fn composite_lower(
    target: &mut [f32],
    width: usize,
    height: usize,
    panel: PanelRect,
    dpr: f32,
    layer: &LowerLayer,
) {
    let Some(bytes) = layer.image.as_bytes(0) else {
        return;
    };
    let size = layer.image.size(0);
    let layer_width = i32::from(size.width);
    let layer_height = i32::from(size.height);
    if layer_width <= 0 || layer_height <= 0 {
        return;
    }
    // 下层位图覆盖的是 `rect + 2*pad`，所以它的左上角在页面坐标里是 rect - pad。
    // 两层都在设备像素坐标系里对齐。
    let origin_x = (layer.rect.x - SHADOW_PAD) * dpr;
    let origin_y = (layer.rect.y - SHADOW_PAD) * dpr;
    for y in 0..height {
        let page_y = (panel.y - SHADOW_PAD) * dpr + y as f32 + 0.5;
        let layer_y = page_y - origin_y;
        if layer_y < 0.0 || layer_y >= layer_height as f32 {
            continue;
        }
        for x in 0..width {
            let page_x = (panel.x - SHADOW_PAD) * dpr + x as f32 + 0.5;
            let layer_x = page_x - origin_x;
            if layer_x < 0.0 || layer_x >= layer_width as f32 {
                continue;
            }
            let index = ((layer_y as i32 * layer_width + layer_x as i32) * 4) as usize;
            let alpha = bytes[index + 3] as f32 / 255.0;
            if alpha <= 0.0 {
                continue;
            }
            let base = (y * width + x) * 3;
            for channel in 0..3 {
                // RenderImage 是 BGRA，取色时把通道换回来。
                let source = bytes[index + 2 - channel] as f32 / 255.0;
                target[base + channel] = mix(target[base + channel], source, alpha);
            }
        }
    }
}

/// `FS_GLASS` 的 CPU 翻译。
fn shade_panel(
    textures: &CropTextures,
    panel: PanelRect,
    config: &LensConfig,
    dpr: f32,
) -> PanelImage {
    let width = textures.width;
    let height = textures.height;
    let res = Vec2::new(width as f32, height as f32);
    // 局部坐标与裁剪区尺寸都是设备像素。
    let half = Vec2::new(panel.width * dpr * 0.5, panel.height * dpr * 0.5);
    let safe_half = Vec2::new(half.x.max(1.0), half.y.max(1.0));
    // 像素类的配置在参照目标里也是乘过 dpr 的（`u_radius = cornerRadius * dpr` 等）。
    let radius = (config.corner_radius * dpr).min(half.x).min(half.y).max(0.0);
    let z_radius = (config.z_radius * dpr).max(1.0);
    let max_depth = half.x.min(half.y);
    let shadow_spread = (config.shadow_spread * dpr).max(1.0);
    let contact_falloff = (shadow_spread * 0.04).max(0.01);
    let px_to_uv = Vec2::new(1.0 / res.x, -1.0 / res.y);
    let mut bgra = vec![0u8; width * height * 4];
    let row_bytes = width * 4;
    let rows_per_chunk = height.div_ceil(worker_count()).max(1);
    let context = ShadeContext {
        textures,
        config: *config,
        res,
        half,
        safe_half,
        radius,
        z_radius,
        max_depth,
        shadow_spread,
        contact_falloff,
        px_to_uv,
        dpr,
    };
    let context = &context;
    std::thread::scope(|scope| {
        let mut first_row = 0usize;
        for chunk in bgra.chunks_mut(rows_per_chunk * row_bytes) {
            let start_row = first_row;
            first_row += chunk.len() / row_bytes;
            scope.spawn(move || {
                for (index, out_row) in chunk.chunks_mut(row_bytes).enumerate() {
                    context.shade_row(out_row, (start_row + index) as u32);
                }
            });
        }
    });
    PanelImage {
        width: width as u32,
        height: height as u32,
        bgra,
    }
}

/// 着色一行的全部上下文。
struct ShadeContext<'a> {
    textures: &'a CropTextures,
    config: LensConfig,
    res: Vec2,
    half: Vec2,
    safe_half: Vec2,
    radius: f32,
    z_radius: f32,
    max_depth: f32,
    shadow_spread: f32,
    contact_falloff: f32,
    px_to_uv: Vec2,
    /// 设备像素比：投影偏移也是像素量，要跟着缩放。
    dpr: f32,
}

impl ShadeContext<'_> {
    /// 算出一整行像素（BGRA）。
    fn shade_row(&self, row: &mut [u8], iy: u32) {
        let config = &self.config;
        let width = (row.len() / 4) as u32;
        for ix in 0..width {
            // v_localPx：以裁剪区中心为原点，范围 ±(size + 2*pad)/2。
            let local = Vec2::new(
                (ix as f32 + 0.5) - self.res.x * 0.5,
                (iy as f32 + 0.5) - self.res.y * 0.5,
            );
            let sdf = rounded_rect_sdf(local, self.half, self.radius);
            let index = ix as usize * 4;

            // ---- 3.1 投影：只对 sdf > 0 的像素（面板外），画完就 return ----
            if sdf > 0.0 {
                let shadow_sdf = rounded_rect_sdf(
                    Vec2::new(local.x, local.y - config.shadow_offset_y * self.dpr),
                    self.half,
                    self.radius,
                );
                let d = (shadow_sdf - 1.0).max(0.0);
                let outer =
                    (-d * d / (self.shadow_spread * self.shadow_spread)).exp() * 0.65;
                let contact = (-d * 0.08 / self.contact_falloff).exp() * 0.35;
                let alpha = (outer + contact) * config.shadow_opacity;
                row[index] = 0;
                row[index + 1] = 0;
                row[index + 2] = 0;
                row[index + 3] = to_byte(alpha);
                continue;
            }

            // ---- 3.2 抗锯齿 mask（边界 2px 过渡带）----
            let mask = 1.0 - smoothstep(-1.5, 0.5, sdf);
            // ---- 3.3 边缘权重与内部深度 ----
            let inside = -sdf;
            let edge = smoothstep(self.max_depth * 0.35, 0.0, inside);

            // ---- 3.4 倒角高度场与法线（中心差分，步长 2px）----
            let center_height = bevel_height(inside, self.z_radius);
            let height_right = bevel_height(
                -rounded_rect_sdf(
                    Vec2::new(local.x + NORMAL_STEP, local.y),
                    self.half,
                    self.radius,
                ),
                self.z_radius,
            );
            let height_left = bevel_height(
                -rounded_rect_sdf(
                    Vec2::new(local.x - NORMAL_STEP, local.y),
                    self.half,
                    self.radius,
                ),
                self.z_radius,
            );
            let height_up = bevel_height(
                -rounded_rect_sdf(
                    Vec2::new(local.x, local.y + NORMAL_STEP),
                    self.half,
                    self.radius,
                ),
                self.z_radius,
            );
            let height_down = bevel_height(
                -rounded_rect_sdf(
                    Vec2::new(local.x, local.y - NORMAL_STEP),
                    self.half,
                    self.radius,
                ),
                self.z_radius,
            );
            let gradient = Vec2::new(
                (height_right - height_left) / (2.0 * NORMAL_STEP),
                (height_up - height_down) / (2.0 * NORMAL_STEP),
            );
            let (nx, ny, nz) = normalize3(-gradient.x, -gradient.y, 1.0);
            let depth = smoothstep(0.0, self.z_radius, inside);

            // ---- 3.5 / 3.6 折射 ----
            let refraction_px = if config.bevel_mode == 0 {
                let thickness = center_height * 2.0;
                let thickness_norm = thickness / (self.z_radius * 2.0).max(1.0);
                let through = Vec2::new(
                    gradient.x * REFR_POW * thickness_norm * 0.5,
                    gradient.y * REFR_POW * thickness_norm * 0.5,
                );
                let mut offset = Vec2::new(
                    (gradient.x * REFR_POW * 2.0 + through.x) * config.refraction * 30.0,
                    (gradient.y * REFR_POW * 2.0 + through.y) * config.refraction * 30.0,
                );
                offset.x += (-local.x / self.safe_half.x) * config.refraction * 4.0 * depth;
                offset.y += (-local.y / self.safe_half.y) * config.refraction * 4.0 * depth;
                offset
            } else {
                Vec2::new(
                    -local.x * config.refraction * depth * 0.35,
                    -local.y * config.refraction * depth * 0.35,
                )
            };

            // ---- 3.7 微噪声 ----
            let micro = if config.distortion > 0.0 {
                let noise = Vec2::new(local.x * 0.08, local.y * 0.08);
                Vec2::new(
                    (hash21(noise) - 0.5) * config.distortion * 4.0 / self.res.x,
                    (hash21(Vec2::new(noise.x + 37.0, noise.y)) - 0.5) * config.distortion * 4.0
                        / self.res.y,
                )
            } else {
                Vec2::new(0.0, 0.0)
            };

            // ---- 3.8 色散 ----
            let chroma_scale = config.chrom_aberration * 18.0 * (edge * 0.7 + 0.3) * 2.0;
            let chroma = Vec2::new(
                nx * chroma_scale * self.px_to_uv.x,
                ny * chroma_scale * self.px_to_uv.y,
            );

            // ---- 3.9 双纹理采样 ----
            let u = (ix as f32 + 0.5) / self.res.x;
            let v = 1.0 - (iy as f32 + 0.5) / self.res.y;
            let base_u = u + refraction_px.x * self.px_to_uv.x + micro.x;
            let base_v = v + refraction_px.y * self.px_to_uv.y + micro.y;
            let sharp_right = self.textures.sample_sharp(base_u + chroma.x, base_v + chroma.y);
            let sharp_mid = self.textures.sample_sharp(base_u, base_v);
            let sharp_left = self.textures.sample_sharp(base_u - chroma.x, base_v - chroma.y);
            let blur_right = self.textures.sample_blur(base_u + chroma.x, base_v + chroma.y);
            let blur_mid = self.textures.sample_blur(base_u, base_v);
            let blur_left = self.textures.sample_blur(base_u - chroma.x, base_v - chroma.y);
            let sharp = [sharp_right[0], sharp_mid[1], sharp_left[2]];
            let blurred = [blur_right[0], blur_mid[1], blur_left[2]];
            // 边缘加权混合：中心几乎全用模糊，边缘最多掺 15% 清晰。
            let edge_mix = 1.0 - edge * 0.15;
            let mut color = [0.0f32; 3];
            for channel in 0..3 {
                color[channel] = mix(sharp[channel], blurred[channel], edge_mix);
            }

            // ---- 3.10 亮度 / 饱和度 / 冷色染色 / 深度提亮 ----
            let brightness = 1.0 + config.brightness;
            for channel in 0..3 {
                color[channel] *= brightness;
            }
            let luminance = color[0] * 0.299 + color[1] * 0.587 + color[2] * 0.114;
            for channel in 0..3 {
                color[channel] = mix(luminance, color[channel], 1.0 + config.saturation);
            }
            let tinted = [color[0] * 0.92, color[1] * 0.95, color[2] * 1.05];
            for channel in 0..3 {
                color[channel] = mix(color[channel], tinted[channel], config.tint_strength);
            }
            let depth_gain = 1.0 + 0.06 * depth;
            for channel in 0..3 {
                color[channel] *= depth_gain;
            }

            // ---- 3.11 Fresnel 与 3.12 四光源高光 ----
            let grazing = 1.0 - nz.abs();
            let fresnel = grazing * grazing * grazing * grazing * config.fresnel;
            let specular = specular_highlight(nx, ny, nz) * config.specular;

            // ---- 3.13 内描边 / rim / 内发光 / 伪环境反射 ----
            let inner_stroke = smoothstep(-BORDER_WIDTH - 1.0, -BORDER_WIDTH, sdf)
                * (1.0 - smoothstep(-1.0, 0.0, sdf));
            let top_bias = 0.5 + 0.5 * (-local.y / self.safe_half.y);
            let inner_stroke = inner_stroke * (0.4 + 0.6 * top_bias);
            let rim = edge * config.edge_highlight * 0.22;
            let inner_glow = smoothstep(5.0, 0.0, -sdf) * config.edge_highlight * 0.15;
            let environment = (ny * 0.5 + 0.5) * fresnel * 0.08;

            // ---- 3.14 最终合成与 alpha ----
            let light = specular
                + rim
                + inner_glow
                + inner_stroke * config.edge_highlight * 0.55
                + environment;
            for channel in 0..3 {
                color[channel] += light;
            }
            for channel in 0..3 {
                color[channel] = mix(color[channel], 1.0, fresnel * 0.2);
            }
            let alpha = mask * config.opacity;
            row[index] = to_byte(color[2]);
            row[index + 1] = to_byte(color[1]);
            row[index + 2] = to_byte(color[0]);
            row[index + 3] = to_byte(alpha);
        }
    }
}

// ---------------------------------------------------------------- GPUI 集成

/// 面板位图的缓存键。坐标取整，避免亚像素抖动导致每帧重算。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PanelKey {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    /// 配置指纹：`button` 模式会改 brightness / z_radius / shadow_spread。
    config_hash: u64,
    reduced: bool,
    solid: bool,
}

/// 页面背景画布在窗口里的位置与尺寸，用来把面板的窗口坐标换算成页面坐标。
#[derive(Clone, Copy)]
struct PageBounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// 面板形状的弹簧状态：内容/尺寸变化时被踢一脚，随后回落。
#[derive(Default)]
struct ShapeSpring {
    width: f32,
    height: f32,
    energy: f32,
    velocity: f32,
    last_frame: Option<std::time::Instant>,
}

impl ShapeSpring {
    fn step(&mut self, width: f32, height: f32) -> f32 {
        let now = std::time::Instant::now();
        let dt = self
            .last_frame
            .map(|last| (now - last).as_secs_f32())
            .unwrap_or(0.0)
            .clamp(0.0, 0.05);
        self.last_frame = Some(now);
        if (self.width - width).abs() > 0.5 || (self.height - height).abs() > 0.5 {
            self.width = width;
            self.height = height;
            self.energy = 1.0;
            self.velocity = 0.0;
        }
        // 欠阻尼弹簧：稍微过冲一点，看起来才像液体而不是缓动。
        let stiffness = 190.0f32;
        let damping = 11.0f32;
        self.velocity += (-stiffness * self.energy - damping * self.velocity) * dt;
        self.energy = (self.energy + self.velocity * dt).clamp(0.0, 1.4);
        if self.energy.abs() < 0.002 && self.velocity.abs() < 0.01 {
            self.energy = 0.0;
            self.velocity = 0.0;
        }
        self.energy
    }
}

#[derive(Default)]
struct RuntimeInner {
    scene: Mutex<Option<(String, Scene)>>,
    panels: Mutex<std::collections::HashMap<PanelKey, Arc<RenderImage>>>,
    cover: Mutex<Option<(std::path::PathBuf, Arc<RgbaImage>)>>,
    page: Mutex<Option<PageBounds>>,
    springs: Mutex<std::collections::HashMap<SpringKey, ShapeSpring>>,
    last_pulse: Mutex<Option<String>>,
    /// 本帧已经渲染过的玻璃面板，按 prepaint 顺序 —— 分层合成要用。
    rendered: Mutex<Vec<(PanelRect, Arc<RenderImage>)>>,
    /// 每块面板的交互状态。
    interaction: Mutex<std::collections::HashMap<String, PanelInteraction>>,
    /// 每块面板最近一次的布局矩形（页面坐标），拖拽的边界约束要用。
    panel_bounds: Mutex<std::collections::HashMap<String, (f32, f32, f32, f32)>>,
    /// 正在后台渲染的面板，避免同一块重复排队。
    inflight: Mutex<std::collections::HashSet<PanelKey>>,
    /// 后台渲染完成后置位，render 里据此再要一帧。
    needs_redraw: std::sync::atomic::AtomicBool,
    /// 页面上那张封面元素的矩形（同一个文件），由页面在 prepaint 时登记。
    artwork: Mutex<Option<SceneArtwork>>,
    /// 场景里是否有动态内容（参照目标的 `data-dynamic` / `<video>`）：
    /// 置位后每帧重算面板，等价于参照目标的「每帧重跑着色器」。
    scene_dynamic: std::sync::atomic::AtomicBool,
}

/// 玻璃渲染的共享状态：整个应用持有一份，克隆给各个画布。
#[derive(Clone, Default)]
pub struct GlassRuntime {
    inner: Arc<RuntimeInner>,
}

impl GlassRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// 系统是否关闭了「透明效果」（Windows 设置 → 个性化 → 颜色）。
    ///
    /// 这是参照目标完全没有的无障碍适配。
    pub fn reduced_transparency() -> bool {
        static CACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *CACHED.get_or_init(read_reduced_transparency)
    }

    /// 换歌时踢一脚形状弹簧：面板会先缩一下再弹回。
    pub fn pulse_on_track_change(&self, track_id: &str) {
        let mut last = self.inner.last_pulse.lock().unwrap_or_else(|e| e.into_inner());
        if last.as_deref() == Some(track_id) {
            return;
        }
        *last = Some(track_id.to_owned());
        let mut springs = self.inner.springs.lock().unwrap_or_else(|e| e.into_inner());
        for spring in springs.values_mut() {
            spring.energy = 1.0;
            spring.velocity = 0.0;
        }
    }

    /// 是否有面板的形状弹簧还在动（帧请求必须由 `render` 发出）。
    pub fn is_animating(&self) -> bool {
        self.inner
            .springs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .any(|spring| spring.energy.abs() > 0.002)
    }

    fn page_bounds(&self) -> Option<PageBounds> {
        *self.inner.page.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn cover_image(&self, path: &std::path::Path) -> Option<Arc<RgbaImage>> {
        let mut cache = self.inner.cover.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached, image)) = cache.as_ref() {
            if cached == path {
                return Some(image.clone());
            }
        }
        let image = Arc::new(image::open(path).ok()?.to_rgba8());
        *cache = Some((path.to_path_buf(), image.clone()));
        Some(image)
    }

    /// 建立（或复用）场景：详情页玻璃背后的内容。
    fn scene(&self, cover: Option<&std::path::Path>, base: Hsla, page: PageBounds) -> Scene {
        // 封面元素的矩形由页面登记；它变了场景也要重算，所以进缓存键。
        let artwork = *self.inner.artwork.lock().unwrap_or_else(|e| e.into_inner());
        let key = format!(
            "{}|{}x{}|{}",
            cover.map(|path| path.display().to_string()).unwrap_or_default(),
            page.width.round() as i32,
            page.height.round() as i32,
            match artwork {
                Some(artwork) => format!(
                    "{:.0},{:.0},{:.0},{:.0},{:.0}",
                    artwork.rect.x,
                    artwork.rect.y,
                    artwork.rect.width,
                    artwork.rect.height,
                    artwork.radius
                ),
                None => String::new(),
            }
        );
        {
            let cache = self.inner.scene.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((cached, scene)) = cache.as_ref() {
                if cached == &key {
                    return scene.clone();
                }
            }
        }
        let cover_image = cover.and_then(|path| self.cover_image(path));
        let rgba = gpui::Rgba::from(base);
        let scene = Scene::new(
            cover_image,
            [rgba.r, rgba.g, rgba.b],
            // 与 `backdrop()` 画的那三层保持一致：封面 0.85、压暗 0.34 → 0.66。
            0.85,
            0.34,
            0.66,
            page.width,
            page.height,
        )
        .with_artwork(artwork);
        let mut cache = self.inner.scene.lock().unwrap_or_else(|e| e.into_inner());
        *cache = Some((key, scene.clone()));
        scene
    }

    /// 渲染（或复用）一块面板的位图。`bounds` 覆盖「面板 + 四周 20px 投影留白」。
    fn panel_image(
        &self,
        bounds: Bounds<gpui::Pixels>,
        cover: Option<&std::path::Path>,
        base: Hsla,
        config: &LensConfig,
        reduced: bool,
        drag: (f32, f32),
        dpr: f32,
    ) -> Option<Arc<RenderImage>> {
        let page = self.page_bounds()?;
        // bounds 含投影留白；再叠加 `floating` 拖拽位移得到面板的视觉位置。
        let panel = PanelRect {
            x: f32::from(bounds.origin.x) - page.x + SHADOW_PAD + drag.0,
            y: f32::from(bounds.origin.y) - page.y + SHADOW_PAD + drag.1,
            width: f32::from(bounds.size.width) - SHADOW_PAD * 2.0,
            height: f32::from(bounds.size.height) - SHADOW_PAD * 2.0,
        };
        if panel.width < 2.0 || panel.height < 2.0 {
            return None;
        }
        let key = PanelKey {
            x: panel.x.max(0.0).round() as u32,
            y: panel.y.max(0.0).round() as u32,
            width: panel.width.round() as u32,
            height: panel.height.round() as u32,
            config_hash: config.fingerprint(),
            reduced,
            solid: cover.is_none(),
        };
        if self
            .inner
            .scene_dynamic
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            // 场景里有动态内容：每帧重算，等价于参照目标对 `data-dynamic` 的每帧重捕获。
            self.inner
                .panels
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
        }
        {
            let panels = self.inner.panels.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(image) = panels.get(&key) {
                return Some(image.clone());
            }
        }
        let scene = self.scene(cover, base, page);
        // 分层合成：z 序在前的玻璃（已经渲染好的）先合成进场景。
        let lower: Vec<LowerLayer> = self
            .inner
            .rendered
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(rect, image)| LowerLayer {
                rect: *rect,
                image: image.clone(),
            })
            .collect();
        // 这条逐像素管线一次要几百毫秒（12 趟高斯 + 逐像素着色），放到后台线程算：
        // 这一帧先不画玻璃，算完置脏标记、下一帧再画，UI 不会被冻住。
        {
            let mut inflight = self.inner.inflight.lock().unwrap_or_else(|e| e.into_inner());
            if !inflight.insert(key) {
                return None;
            }
        }
        let runtime = self.clone();
        let config = *config;
        std::thread::spawn(move || {
            let rendered = render_panel(&scene, panel, &config, &lower, dpr);
            let image = rendered.to_render_image();
            {
                let mut panels = runtime.inner.panels.lock().unwrap_or_else(|e| e.into_inner());
                if panels.len() > 24 {
                    panels.clear();
                }
                panels.insert(key, image.clone());
            }
            runtime
                .inner
                .rendered
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((panel, image));
            runtime
                .inner
                .inflight
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&key);
            runtime
                .inner
                .needs_redraw
                .store(true, std::sync::atomic::Ordering::Relaxed);
        });
        None
    }

    /// 标记场景里是否存在动态内容（对应参照目标的 `data-dynamic` / `<video>`）。
    ///
    /// 置位后每帧都会重算面板，与参照目标「动态内容每帧重捕获、玻璃每帧重跑着色器」
    /// 的行为一致；取消置位后恢复缓存。详情页玻璃背后是静态的封面与渐变，所以默认不置位。
    #[allow(dead_code)] // 详情页玻璃背后是静态内容，这个能力由测试覆盖
    pub fn mark_scene_dynamic(&self, dynamic: bool) {
        self.inner
            .scene_dynamic
            .store(dynamic, std::sync::atomic::Ordering::Relaxed);
        if dynamic {
            self.invalidate();
        }
    }

    /// 按窗口坐标更新某块面板的拖拽位移（带参照目标的 10px 边界约束）。
    ///
    /// 元素级的 `on_mouse_move` 与拖拽期间的窗口级监听共用这一个入口。
    fn update_drag(&self, id: &str, pointer_x: f32, pointer_y: f32) {
        let page = self.page_bounds();
        let layout = self
            .inner
            .panel_bounds
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .copied();
        self.with_interaction(id, |state| {
            let Some(origin) = state.drag_origin else {
                return;
            };
            let dx = pointer_x - origin.0;
            let dy = pointer_y - origin.1;
            let mut next = (state.drag_base.0 + dx, state.drag_base.1 + dy);
            if let (Some(page), Some((x, y, width, height))) = (page, layout) {
                let min_x = DRAG_MARGIN - x;
                let max_x = (page.width - DRAG_MARGIN - (x + width)).max(min_x);
                let min_y = DRAG_MARGIN - y;
                let max_y = (page.height - DRAG_MARGIN - (y + height)).max(min_y);
                next.0 = next.0.clamp(min_x, max_x);
                next.1 = next.1.clamp(min_y, max_y);
            }
            state.drag = next;
        });
    }

    /// 登记页面上那张封面元素的矩形，让它进入玻璃背后的场景。
    ///
    /// 参照目标的 `_composeSceneForGlass` 会把面板之前的兄弟元素一并画进场景，封面就是其中一个；
    /// 这里由页面在 prepaint 时把它的页面坐标矩形登记进来。
    pub fn set_artwork(&self, artwork: Option<SceneArtwork>) {
        let mut current = self.inner.artwork.lock().unwrap_or_else(|e| e.into_inner());
        // 亚像素的布局抖动不算变化：否则每帧都会清缓存、面板每帧重算。
        let changed = match (&*current, &artwork) {
            (Some(old), Some(new)) => {
                (old.rect.x - new.rect.x).abs() > 0.5
                    || (old.rect.y - new.rect.y).abs() > 0.5
                    || (old.rect.width - new.rect.width).abs() > 0.5
                    || (old.rect.height - new.rect.height).abs() > 0.5
            }
            (None, None) => false,
            _ => true,
        };
        *current = artwork;
        if changed {
            // 场景变了，面板位图也要重算。
            self.inner
                .panels
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
        }
    }

    /// 覆盖在页面上某个元素上，把它的页面坐标矩形登记给场景（见 [`SceneArtwork`]）。
    ///
    /// 用法：作为那个元素的 `child` 加进去，它会绝对定位铺满父元素。参照目标的
    /// `_composeSceneForGlass` 会自动把兄弟元素画进场景，这里需要页面显式登记。
    pub fn artwork_probe(&self, radius: f32) -> impl IntoElement {
        let runtime = self.clone();
        canvas(
            move |bounds, _window, _cx| {
                let Some(page) = runtime.page_bounds() else {
                    return;
                };
                runtime.set_artwork(Some(SceneArtwork {
                    rect: PanelRect {
                        x: f32::from(bounds.origin.x) - page.x,
                        y: f32::from(bounds.origin.y) - page.y,
                        width: f32::from(bounds.size.width),
                        height: f32::from(bounds.size.height),
                    },
                    radius,
                }));
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
    }
    /// 让所有缓存失效（对应参照目标的 `_globalDirty`：resize、结构变化、上下文恢复）。
    pub fn invalidate(&self) {
        self.inner
            .scene
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        self.inner
            .panels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.inner
            .rendered
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    /// 后台渲染完成，需要再画一帧。
    pub fn take_needs_redraw(&self) -> bool {
        self.inner
            .needs_redraw
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }
}

impl GlassRuntime {
    /// 背景层：铺满整页的封面 + 压暗渐变，对应参照目标的「场景画布」。
    ///
    /// 它同时负责两件事：记录页面原点（面板的 `canvas` 要用它把自己的窗口坐标换算成
    /// 页面坐标），以及**每帧清空**「本帧已渲染玻璃」列表（分层合成要用）。
    pub fn backdrop(
        &self,
        cover: Option<std::path::PathBuf>,
        base: Hsla,
        shade: Hsla,
    ) -> Div {
        let mut layer = div().absolute().inset_0().bg(base);
        if let Some(path) = cover {
            layer = layer.child(
                img(path)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(gpui::ObjectFit::Cover)
                    .opacity(0.85),
            );
        }
        layer = layer.child(
            div().absolute().inset_0().bg(linear_gradient(
                180.0,
                linear_color_stop(shade.opacity(0.34), 0.0),
                linear_color_stop(shade.opacity(0.66), 1.0),
            )),
        );
        let runtime = self.clone();
        layer.child(
            canvas(
                move |bounds, _window, _cx| {
                    let page = PageBounds {
                        x: f32::from(bounds.origin.x),
                        y: f32::from(bounds.origin.y),
                        width: f32::from(bounds.size.width),
                        height: f32::from(bounds.size.height),
                    };
                    *runtime.inner.page.lock().unwrap_or_else(|e| e.into_inner()) = Some(page);
                    runtime
                        .inner
                        .rendered
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clear();
                },
                |_bounds, _state, _window, _cx| {},
            )
            .absolute()
            .inset_0(),
        )
    }

    /// 取某块面板的交互状态（`floating` 拖拽与 `button` 的 hover/pressed）。
    pub fn interaction(&self, id: &str) -> PanelInteraction {
        self.inner
            .interaction
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .copied()
            .unwrap_or_default()
    }

    /// 一块玻璃面板。
    ///
    /// 面板本体（含四周 20px 的投影留白）由 CPU 端移植的整条管线算出来，通过 `canvas` +
    /// `paint_image` 画上去；投影也在这张位图里，与参照目标把阴影交给同一个着色器一致。
    pub fn panel(
        &self,
        id: &str,
        cover: Option<std::path::PathBuf>,
        config: LensConfig,
        reduced: bool,
        base: Hsla,
        interaction: PanelInteraction,
    ) -> Div {
        let radius = px(config.corner_radius);
        // 参照目标：`floating` 门控拖拽，`button` 门控 hover/pressed。
        let floating = config.floating;
        let button = config.button;
        let id = id.to_owned();
        let prepaint_runtime = self.clone();
        let paint_runtime = self.clone();
        let paint_id = id.clone();
        let drag = interaction.drag;
        let hover_id = id.clone();
        let press_id = id.clone();
        let move_id = id.clone();
        let release_id = id.clone();
        let release_id2 = id.clone();
        let hover_runtime = self.clone();
        let press_runtime = self.clone();
        let move_runtime = self.clone();
        let release_runtime = self.clone();
        let release_runtime2 = self.clone();
        let inner = div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .id(gpui::ElementId::Name(format!("glass-panel-{id}").into()))
            // `button` 模式：hover 时 brightness += 0.2。
            .on_hover(move |hovered, window, _cx| {
                if !button {
                    return;
                }
                let changed =
                    hover_runtime.with_interaction(&hover_id, |state| state.hovered = *hovered);
                if changed {
                    window.refresh();
                }
            })
            .on_mouse_down(gpui::MouseButton::Left, move |event, window, _cx| {
                if !floating && !button {
                    return;
                }
                let changed = press_runtime.with_interaction(&press_id, |state| {
                    if button {
                        state.pressed = true;
                    }
                    if floating {
                        state.drag_origin =
                            Some((f32::from(event.position.x), f32::from(event.position.y)));
                        state.drag_base = state.drag;
                    }
                });
                if changed {
                    window.refresh();
                }
            })
            // `floating`：拖拽时按累计位移平移（参照目标用 `transform: translate`）。
            .on_mouse_move(move |event, _window, _cx| {
                if !floating {
                    return;
                }
                move_runtime.update_drag(
                    &move_id,
                    f32::from(event.position.x),
                    f32::from(event.position.y),
                );
            })
            .on_mouse_up(gpui::MouseButton::Left, move |_event, window, _cx| {
                if !floating && !button {
                    return;
                }
                let changed = release_runtime.with_interaction(&release_id, |state| {
                    state.pressed = false;
                    state.drag_origin = None;
                });
                if changed {
                    window.refresh();
                }
            })
            .on_mouse_up_out(gpui::MouseButton::Left, move |_event, _window, _cx| {
                if !floating && !button {
                    return;
                }
                release_runtime2.with_interaction(&release_id2, |state| {
                    state.pressed = false;
                    state.drag_origin = None;
                });
            })
            .child(
                canvas(
                    move |bounds, window, _cx| {
                        // 参照目标按设备像素渲染，这里跟着窗口的缩放比走。
                        let scale_factor = window.scale_factor();
                        let width = f32::from(bounds.size.width);
                        let height = f32::from(bounds.size.height);
                        let morph = prepaint_runtime
                            .inner
                            .springs
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry(SpringKey::new(&id, width, height))
                            .or_default()
                            .step(width, height);
                        let effective = button_config(config, interaction);
                        if let Some(page) = prepaint_runtime.page_bounds() {
                        prepaint_runtime
                            .inner
                            .panel_bounds
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(
                                id.clone(),
                                (
                                    f32::from(bounds.origin.x) - page.x + SHADOW_PAD + interaction.drag.0,
                                    f32::from(bounds.origin.y) - page.y + SHADOW_PAD + interaction.drag.1,
                                    width - SHADOW_PAD * 2.0,
                                    height - SHADOW_PAD * 2.0,
                                ),
                            );
                    }
                    let image = prepaint_runtime.panel_image(
                        bounds,
                        cover.as_deref(),
                        base,
                        &effective,
                        reduced,
                        interaction.drag,
                        scale_factor,
                    );
                        (image, morph)
                    },
                    move |bounds, (image, morph), window, _cx| {
                        // 拖拽中注册窗口级鼠标监听：指针移出面板也收得到位移，
                        // 对应参照目标的 `setPointerCapture`（GPUI 没有这个 API）。
                        if paint_runtime
                            .interaction(&paint_id)
                            .drag_origin
                            .is_some()
                        {
                            let runtime = paint_runtime.clone();
                            let id = paint_id.clone();
                            window.on_mouse_event(
                                move |event: &gpui::MouseMoveEvent, phase, _window, _cx| {
                                    if phase != gpui::DispatchPhase::Bubble {
                                        return;
                                    }
                                    runtime.update_drag(
                                        &id,
                                        f32::from(event.position.x),
                                        f32::from(event.position.y),
                                    );
                                },
                            );
                        }
                        let Some(image) = image else {
                            return;
                        };
                        // 形变：内容变化时玻璃先缩一下再弹回。
                        let shrink_x = bounds.size.width * 0.035 * morph;
                        let shrink_y = bounds.size.height * 0.02 * morph;
                        let image_bounds = Bounds {
                            origin: gpui::point(
                                bounds.origin.x + px(drag.0) + shrink_x,
                                bounds.origin.y + px(drag.1) + shrink_y,
                            ),
                            size: gpui::size(
                                bounds.size.width - shrink_x * 2.0,
                                bounds.size.height - shrink_y * 2.0,
                            ),
                        };
                        // 圆角与投影都已经烘进位图，这里不再重复裁剪。
                        let _ = window.paint_image(
                            bounds,
                            image_bounds,
                            Corners::all(px(0.0)),
                            image,
                            0,
                            false,
                        );
                    },
                )
                .absolute()
                .inset(px(-SHADOW_PAD)),
            );
        // 作为背景层铺满父容器（调用方不用再补定位）。
        div().absolute().inset_0().child(inner)
    }

    fn with_interaction(&self, id: &str, update: impl FnOnce(&mut PanelInteraction)) -> bool {
        let mut map = self
            .inner
            .interaction
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let state = map.entry(id.to_owned()).or_default();
        let mut changed = false;
        let before = *state;
        update(state);
        if state.hovered != before.hovered || state.pressed != before.pressed {
            changed = true;
        }
        changed
    }
}

/// `button` 模式：hover 提亮 0.2；按下把倒角压平、投影扩散放大（与参照目标一致）。
fn button_config(config: LensConfig, interaction: PanelInteraction) -> LensConfig {
    let mut effective = config;
    // 参照目标用 `config.button` 门控：不是按钮的面板既不吃 hover 也不吃 pressed。
    if !config.button {
        return effective;
    }
    if interaction.pressed {
        effective.z_radius *= 0.8;
        effective.shadow_spread *= 1.2;
    } else if interaction.hovered {
        effective.brightness += 0.2;
    }
    effective
}

/// 一块面板的交互状态（`floating` 拖拽与 `button` 的 hover/pressed）。
#[derive(Default, Clone, Copy)]
pub struct PanelInteraction {
    pub hovered: bool,
    pub pressed: bool,
    /// `floating` 拖拽的累计位移（参照目标写的是 `transform: translate(...)`）。
    pub drag: (f32, f32),
    drag_origin: Option<(f32, f32)>,
    drag_base: (f32, f32),
}

/// 形状弹簧的键：同一块面板（按调用方给的 id）在同一个尺寸下共用一个弹簧。
#[derive(Clone, PartialEq, Eq, Hash)]
struct SpringKey {
    id: String,
    width: u32,
    height: u32,
}

impl SpringKey {
    fn new(id: &str, width: f32, height: f32) -> Self {
        Self {
            id: id.to_owned(),
            width: width.round() as u32,
            height: height.round() as u32,
        }
    }
}

impl LensConfig {
    /// 配置指纹：`button` 模式会改 brightness / z_radius / shadow_spread，缓存要分开。
    fn fingerprint(&self) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for value in [
            self.blur_amount,
            self.refraction,
            self.chrom_aberration,
            self.edge_highlight,
            self.specular,
            self.fresnel,
            self.distortion,
            self.corner_radius,
            self.z_radius,
            self.opacity,
            self.saturation,
            self.tint_strength,
            self.brightness,
            self.shadow_opacity,
            self.shadow_spread,
            self.shadow_offset_y,
            f32::from(self.floating as u8),
            f32::from(self.button as u8),
        ] {
            hash ^= value.to_bits() as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= self.bevel_mode as u64;
        hash
    }
}

/// 读 Windows 的「透明效果」开关（设置 → 个性化 → 颜色）。
///
/// 读不到时按「开启」处理：宁可多给效果，也不要无故降级。
#[cfg(windows)]
fn read_reduced_transparency() -> bool {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW,
    };
    const SUBKEY: &str = "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
    const VALUE: &str = "EnableTransparency";
    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain(std::iter::once(0)).collect() };
    unsafe {
        let mut key: HKEY = std::ptr::null_mut();
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            wide(SUBKEY).as_ptr(),
            0,
            KEY_READ,
            &mut key,
        ) != 0
        {
            return false;
        }
        let mut value: u32 = 1;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut kind = 0u32;
        let status = RegQueryValueExW(
            key,
            wide(VALUE).as_ptr(),
            std::ptr::null_mut(),
            &mut kind,
            &mut value as *mut u32 as *mut u8,
            &mut size,
        );
        RegCloseKey(key);
        status == 0 && value == 0
    }
}

#[cfg(not(windows))]
fn read_reduced_transparency() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 横向渐变（任何横向位移都会改变取值）+ 细条纹（用来测模糊）。
    fn test_cover(width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let ramp = (x as f32 / width as f32 * 190.0) as u8;
                let stripe = if (x / 2) % 2 == 0 { 0u8 } else { 35 };
                let vertical = (y as f32 / height as f32 * 60.0) as u8;
                image.put_pixel(
                    x,
                    y,
                    Rgba([
                        ramp.saturating_add(stripe),
                        vertical,
                        205u8.saturating_sub(ramp / 2),
                        255,
                    ]),
                );
            }
        }
        image
    }

    fn test_scene() -> Scene {
        Scene::new(
            Some(Arc::new(test_cover(320, 320))),
            [0.10, 0.10, 0.12],
            0.85,
            0.34,
            0.66,
            1280.0,
            720.0,
        )
    }

    fn test_panel() -> PanelRect {
        PanelRect {
            x: 120.0,
            y: 90.0,
            width: 600.0,
            height: 400.0,
        }
    }

    fn panel_of(config: &LensConfig) -> PanelImage {
        render_panel(&test_scene(), test_panel(), config, &[], 1.0)
    }

    /// 纯横向渐变：没有任何高频，适合测「位移类」效果。
    fn smooth_cover(width: u32, height: u32) -> RgbaImage {
        let mut image = RgbaImage::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let ramp = (x as f32 / width as f32 * 200.0) as u8;
                let vertical = (y as f32 / height as f32 * 100.0) as u8;
                image.put_pixel(x, y, Rgba([ramp, vertical, 120, 255]));
            }
        }
        image
    }

    fn smooth_scene() -> Scene {
        Scene::new(
            Some(Arc::new(smooth_cover(320, 320))),
            [0.10, 0.10, 0.12],
            0.85,
            0.34,
            0.66,
            1280.0,
            720.0,
        )
    }

    fn smooth_panel(config: &LensConfig) -> PanelImage {
        render_panel(&smooth_scene(), test_panel(), config, &[], 1.0)
    }

    /// 输出必须覆盖「面板 + 四周 20px 投影留白」。
    #[test]
    fn the_output_covers_the_panel_plus_shadow_padding() {
        let panel = panel_of(&LensConfig::default());
        assert_eq!(panel.width, 600 + 40);
        assert_eq!(panel.height, 400 + 40);
    }

    /// 圆角落在面板本体上：面板四角透明、中心不透明；留白区是投影。
    #[test]
    fn the_panel_is_rounded_with_transparent_corners() {
        let config = LensConfig {
            shadow_opacity: 0.0,
            ..Default::default()
        };
        let panel = panel_of(&config);
        let pad = SHADOW_PAD as u32;
        assert_eq!(panel.alpha(pad, pad), 0, "面板左上角应当是透明的");
        assert_eq!(panel.alpha(panel.width - pad - 1, pad), 0);
        assert_eq!(panel.alpha(pad, panel.height - pad - 1), 0);
        assert_eq!(panel.alpha(panel.width - pad - 1, panel.height - pad - 1), 0);
        assert_eq!(panel.alpha(panel.width / 2, panel.height / 2), 255);
        // 留白区（面板外）在 shadowOpacity = 0 时也必须是全透明。
        assert_eq!(panel.alpha(0, 0), 0);
    }

    /// 投影只画在面板外，且是黑色半透明。
    #[test]
    fn the_shadow_lives_in_the_padding_and_is_black() {
        let config = LensConfig {
            shadow_opacity: 0.6,
            shadow_spread: 20.0,
            shadow_offset_y: 10.0,
            ..Default::default()
        };
        let panel = panel_of(&config);
        // 面板正下方（留白里）应当有可观的阴影。
        let below = panel.alpha(panel.width / 2, panel.height - 6);
        assert!(below > 20, "面板下方应当有投影，实际 alpha={below}");
        // 面板内部不画阴影。
        assert_eq!(panel.alpha(panel.width / 2, panel.height / 2), 255);
        let rgb = panel.rgb(panel.width / 2, panel.height - 6);
        assert_eq!(rgb, [0, 0, 0], "投影必须是纯黑");
    }

    /// 折射：开启后采样点整体偏移，画面与不开折射时不同。
    #[test]
    fn refraction_moves_the_sampled_background() {
        let flat = LensConfig {
            refraction: 0.0,
            chrom_aberration: 0.0,
            blur_amount: 0.0,
            ..Default::default()
        };
        let bent = LensConfig {
            refraction: 1.2,
            ..flat
        };
        let flat_panel = panel_of(&flat);
        let bent_panel = panel_of(&bent);
        let mut differing = 0;
        for x in 0..flat_panel.width {
            let y = flat_panel.height / 2;
            if flat_panel.rgb(x, y) != bent_panel.rgb(x, y) {
                differing += 1;
            }
        }
        assert!(differing > 20, "折射没有改变采样：只有 {differing} 个像素不同");
    }

    /// 色散：强度按 edge 加权，边缘的变化必须大于中心。
    #[test]
    fn chromatic_aberration_splits_the_colour_channels() {
        let base = LensConfig {
            chrom_aberration: 0.0,
            blur_amount: 0.0,
            refraction: 0.0,
            ..Default::default()
        };
        let plain = panel_of(&base);
        let split = panel_of(&LensConfig {
            chrom_aberration: 0.3,
            ..base
        });
        let change = |x: u32, y: u32| -> i32 {
            let a = plain.rgb(x, y);
            let b = split.rgb(x, y);
            (0..3)
                .map(|channel| (a[channel] as i32 - b[channel] as i32).abs())
                .sum()
        };
        let y = plain.height / 2;
        let pad = SHADOW_PAD as u32;
        let edge = change(pad + 3, y).max(change(plain.width - pad - 4, y));
        let centre = change(plain.width / 2, y);
        assert!(
            edge > centre,
            "色散应当让边缘的变化大于中心：边缘 {edge}，中心 {centre}"
        );
    }

    /// 模糊：blurAmount 越大，背景的高频越少。
    #[test]
    fn more_blur_means_less_detail() {
        let roughness = |blur: f32| -> f32 {
            let config = LensConfig {
                blur_amount: blur,
                refraction: 0.0,
                chrom_aberration: 0.0,
                ..Default::default()
            };
            let panel = panel_of(&config);
            let mut total = 0.0;
            let y = panel.height / 2;
            for x in (SHADOW_PAD as u32 + 1)..(panel.width - SHADOW_PAD as u32) {
                let a = panel.rgb(x - 1, y);
                let b = panel.rgb(x, y);
                total += (a[0] as f32 - b[0] as f32).abs();
            }
            total / panel.width as f32
        };
        let sharp = roughness(0.0);
        let blurred = roughness(0.5);
        assert!(
            blurred < sharp,
            "模糊后相邻像素差应当更小：清晰 {sharp:.1}，模糊 {blurred:.1}"
        );
    }

    /// 模糊核必须与参照目标的 9-tap 权重逐位一致（用独立的朴素实现做对照）。
    #[test]
    fn the_blur_kernel_matches_the_reference_weights() {
        let width = 9usize;
        let height = 3usize;
        let mut source = vec![0.0f32; width * height * 3];
        for y in 0..height {
            for x in 0..width {
                // 单点脉冲，便于直接读出核。
                let value = if x == 4 && y == 1 { 1.0 } else { 0.0 };
                let index = (y * width + x) * 3;
                source[index] = value;
                source[index + 1] = value;
                source[index + 2] = value;
            }
        }
        let spread = 1.0f32;
        let mut target = vec![0.0f32; source.len()];
        // 只跑一趟横向，直接与权重表对照。
        blur_rows(&source, &mut target, width, height, spread);
        let center = (1 * width + 4) * 3;
        let left1 = (1 * width + 3) * 3;
        let right1 = (1 * width + 5) * 3;
        let left2 = (1 * width + 2) * 3;
        assert!((target[center] - BLUR_TAPS[0]).abs() < 1e-6);
        assert!((target[left1] - BLUR_TAPS[1]).abs() < 1e-6);
        assert!((target[right1] - BLUR_TAPS[1]).abs() < 1e-6);
        assert!((target[left2] - BLUR_TAPS[2]).abs() < 1e-6);
        // 权重和应当约等于 1（参照目标的表加起来是 0.999999）。
        let sum: f32 = BLUR_TAPS[0] + 2.0 * (BLUR_TAPS[1] + BLUR_TAPS[2] + BLUR_TAPS[3] + BLUR_TAPS[4]);
        assert!((sum - 1.0).abs() < 1e-5, "核权重和 {sum}");
    }

    /// 内描边：顶部比底部亮（topBias）。
    #[test]
    fn the_inner_stroke_is_brighter_on_top() {
        let panel = panel_of(&LensConfig {
            edge_highlight: 0.2,
            ..Default::default()
        });
        let plain = panel_of(&LensConfig {
            edge_highlight: 0.0,
            ..Default::default()
        });
        let luminance = |panel: &PanelImage, x: u32, y: u32| -> i32 {
            let rgb = panel.rgb(x, y);
            rgb[0] as i32 + rgb[1] as i32 + rgb[2] as i32
        };
        let pad = SHADOW_PAD as u32;
        let x = panel.width / 2;
        // 描边带只覆盖距边界 1.5~2.5 个像素，取最外圈那一行。
        let top_gain = luminance(&panel, x, pad) - luminance(&plain, x, pad);
        let bottom_gain = luminance(&panel, x, panel.height - pad - 1)
            - luminance(&plain, x, plain.height - pad - 1);
        assert!(
            top_gain > bottom_gain,
            "顶部内描边应当比底部亮：顶 +{top_gain}，底 +{bottom_gain}"
        );
    }

    /// 分层合成：z 序在前的玻璃的位图必须出现在场景里（被折射到）。
    #[test]
    fn lower_glass_layers_are_composited_into_the_scene() {
        // 造一张纯红的「下层玻璃」位图，覆盖面板左上角一块。
        let lower_rect = PanelRect {
            x: 120.0,
            y: 90.0,
            width: 600.0,
            height: 400.0,
        };
        let lower_width = (lower_rect.width + SHADOW_PAD * 2.0) as u32;
        let lower_height = (lower_rect.height + SHADOW_PAD * 2.0) as u32;
        let mut bytes = vec![0u8; (lower_width * lower_height * 4) as usize];
        for pixel in bytes.chunks_exact_mut(4) {
            // BGRA：红 = B0 G0 R255 A255
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 255;
            pixel[3] = 255;
        }
        let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(lower_width, lower_height, bytes).expect("尺寸一致");
        let image = Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)));
        let layer = LowerLayer {
            rect: lower_rect,
            image,
        };
        let config = LensConfig {
            refraction: 0.0,
            chrom_aberration: 0.0,
            blur_amount: 0.0,
            brightness: 0.0,
            saturation: 0.0,
            tint_strength: 0.0,
            edge_highlight: 0.0,
            specular: 0.0,
            fresnel: 0.0,
            ..Default::default()
        };
        let plain = render_panel(&test_scene(), test_panel(), &config, &[], 1.0);
        let composited =
            render_panel(
                &test_scene(),
                test_panel(),
                &config,
                std::slice::from_ref(&layer),
                1.0,
            );
        let x = composited.width / 2;
        let y = composited.height / 2;
        assert_ne!(
            plain.rgb(x, y),
            composited.rgb(x, y),
            "下层玻璃的像素应当出现在场景里"
        );
        let rgb = composited.rgb(x, y);
        assert!(rgb[0] > 200 && rgb[1] < 60, "应当是下层那层红色：{rgb:?}");
    }

    /// `button` 模式：hover 提亮、按下压平倒角并放大投影。
    #[test]
    fn button_mode_matches_the_reference_values() {
        // `button: true` 才吃 hover/pressed（参照目标的 `config.button` 门控）。
        let base = LensConfig {
            z_radius: 40.0,
            shadow_spread: 10.0,
            brightness: 0.0,
            button: true,
            ..Default::default()
        };
        let hovered = button_config(
            base,
            PanelInteraction {
                hovered: true,
                ..Default::default()
            },
        );
        assert!((hovered.brightness - 0.2).abs() < 1e-6, "hover 应当提亮 0.2");
        assert!((hovered.z_radius - 40.0).abs() < 1e-6);

        let pressed = button_config(
            base,
            PanelInteraction {
                hovered: true,
                pressed: true,
                ..Default::default()
            },
        );
        assert!((pressed.z_radius - 32.0).abs() < 1e-6, "按下应当把 zRadius 压到 0.8 倍");
        assert!((
            pressed.shadow_spread - 12.0
        )
        .abs()
            < 1e-6,
            "按下应当把 shadowSpread 放大到 1.2 倍"
        );
        assert!((pressed.brightness - 0.0).abs() < 1e-6, "按下不提亮");
    }

    /// 无障碍：降低透明度时折射与色散都要收敛。
    #[test]
    fn reduced_transparency_tones_the_lens_down() {
        let full = LensConfig::for_theme(true, false);
        let reduced = LensConfig::for_theme(true, true);
        assert!(reduced.chrom_aberration < full.chrom_aberration);
        assert!(reduced.refraction < full.refraction);
        assert!(reduced.blur_amount < full.blur_amount);
        assert!(reduced.brightness < full.brightness, "暗色主题下降透明度应当更暗");
    }

    /// 明暗模式：暗色主题压暗、亮色主题提亮。
    #[test]
    fn light_and_dark_themes_differ() {
        assert!(LensConfig::for_theme(true, false).brightness < 0.0);
        assert!(LensConfig::for_theme(false, false).brightness > 0.0);
    }

    /// 穹顶模式（`bevelMode = 1`）走另一条折射分支：位移是向心收缩（放大镜）。
    #[test]
    fn the_dome_bevel_mode_uses_the_other_refraction_branch() {
        let base = LensConfig {
            refraction: 1.2,
            chrom_aberration: 0.0,
            blur_amount: 0.0,
            bevel_mode: 0,
            ..Default::default()
        };
        let pill = smooth_panel(&base);
        let dome = smooth_panel(&LensConfig {
            bevel_mode: 1,
            ..base
        });
        let pad = SHADOW_PAD as u32;
        let y = pill.height / 2;
        let mut differing = 0;
        for x in pad..(pill.width - pad) {
            if pill.rgb(x, y) != dome.rgb(x, y) {
                differing += 1;
            }
        }
        assert!(differing > 20, "穹顶模式应当改变画面：只有 {differing} 个像素不同");
        // 穹顶是向心收缩：右半边取到更靠左的采样点，横向渐变的取值应当更小。
        let probe = pad + (pill.width - pad * 2) * 3 / 4;
        assert!(
            dome.rgb(probe, y)[0] < pill.rgb(probe, y)[0],
            "穹顶应当把画面向心收缩：双凸 {} vs 穹顶 {}",
            pill.rgb(probe, y)[0],
            dome.rgb(probe, y)[0]
        );
    }

    /// 微噪声：`distortion > 0` 时采样点带上亚像素抖动。
    #[test]
    fn distortion_adds_subpixel_noise() {
        let base = LensConfig {
            distortion: 0.0,
            blur_amount: 0.0,
            refraction: 0.0,
            chrom_aberration: 0.0,
            ..Default::default()
        };
        let plain = panel_of(&base);
        let noisy = panel_of(&LensConfig {
            distortion: 0.2,
            ..base
        });
        let pad = SHADOW_PAD as u32;
        let mut differing = 0;
        for y in (pad..(plain.height - pad)).step_by(7) {
            for x in pad..(plain.width - pad) {
                if plain.rgb(x, y) != noisy.rgb(x, y) {
                    differing += 1;
                }
            }
        }
        assert!(differing > 100, "噪声应当改变画面：只有 {differing} 个像素不同");
    }

    /// alpha 是直通（非预乘）的：面板内不透明、投影区纯黑半透明、AA 带单调过渡。
    #[test]
    fn the_output_alpha_is_straight_not_premultiplied() {
        let panel = panel_of(&LensConfig {
            shadow_opacity: 0.5,
            shadow_spread: 20.0,
            shadow_offset_y: 10.0,
            ..Default::default()
        });
        let pad = SHADOW_PAD as u32;
        // 面板内部：完全不透明。
        assert_eq!(panel.alpha(panel.width / 2, panel.height / 2), 255);
        // 投影区：纯黑 + 半透明（预乘的话 RGB 会跟着 alpha 变，这里必须恒为 0）。
        let shadow_alpha = panel.alpha(panel.width / 2, panel.height - 4);
        assert!(
            shadow_alpha > 0 && shadow_alpha < 255,
            "投影应当是半透明：{shadow_alpha}"
        );
        assert_eq!(panel.rgb(panel.width / 2, panel.height - 4), [0, 0, 0]);
        // AA 带：沿面板左上角对角线，alpha 单调不减，最后到达 255。
        let mut previous = 0u8;
        for step in 0..24u32 {
            let alpha = panel.alpha(pad + step, pad + step);
            assert!(alpha >= previous, "AA 带的 alpha 应当单调不减");
            previous = alpha;
        }
        assert_eq!(previous, 255, "对角线走到底应当进入不透明区");
    }

    /// 动态场景（对应参照目标的 `data-dynamic`）：置位会清缓存，取消后恢复缓存。
    #[test]
    fn a_dynamic_scene_invalidates_the_panel_cache() {
        let runtime = GlassRuntime::new();
        let key = PanelKey {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
            config_hash: 5,
            reduced: false,
            solid: false,
        };
        let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(1, 1, vec![0u8, 0, 0, 255]).expect("1×1");
        let image = Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)));

        runtime
            .inner
            .panels
            .lock()
            .unwrap()
            .insert(key, image.clone());
        assert_eq!(runtime.inner.panels.lock().unwrap().len(), 1);

        runtime.mark_scene_dynamic(true);
        assert!(runtime
            .inner
            .scene_dynamic
            .load(std::sync::atomic::Ordering::Relaxed));
        assert!(
            runtime.inner.panels.lock().unwrap().is_empty(),
            "置位动态场景应当清空面板缓存"
        );

        runtime
            .inner
            .panels
            .lock()
            .unwrap()
            .insert(key, image);
        runtime.mark_scene_dynamic(false);
        assert!(!runtime
            .inner
            .scene_dynamic
            .load(std::sync::atomic::Ordering::Relaxed));
        assert_eq!(
            runtime.inner.panels.lock().unwrap().len(),
            1,
            "取消动态场景不应当清缓存"
        );

        runtime.invalidate();
        assert!(runtime.inner.panels.lock().unwrap().is_empty());
    }

    /// 设备像素比：整条管线按 dpr 放大（参照目标的 uniform 全部乘过 dpr）。
    #[test]
    fn the_pipeline_renders_at_device_pixel_ratio() {
        let config = LensConfig::default();
        let single = render_panel(&test_scene(), test_panel(), &config, &[], 1.0);
        let double = render_panel(&test_scene(), test_panel(), &config, &[], 2.0);
        assert_eq!(single.width, 600 + 40);
        assert_eq!(double.width, (600 + 40) * 2);
        assert_eq!(double.height, (400 + 40) * 2);
        // 几何不变，只是分辨率变高：内部照样不透明、留白照样透明。
        assert_eq!(single.alpha(single.width / 2, single.height / 2), 255);
        assert_eq!(double.alpha(double.width / 2, double.height / 2), 255);
        assert_eq!(single.alpha(SHADOW_PAD as u32, SHADOW_PAD as u32), 0);
        assert_eq!(
            double.alpha((SHADOW_PAD * 2.0) as u32, (SHADOW_PAD * 2.0) as u32),
            0
        );
    }

    /// 页面上那张封面元素也要进场景：它自己的矩形内取到的是封面，矩形外还是原背景。
    #[test]
    fn the_artwork_is_part_of_the_scene() {
        let rect = PanelRect {
            x: 100.0,
            y: 200.0,
            width: 280.0,
            height: 280.0,
        };
        let plain = test_scene();
        let with_artwork = test_scene().with_artwork(Some(SceneArtwork {
            rect,
            radius: PANEL_RADIUS,
        }));

        // 矩形中心：封面元素铺的是另一段裁切，取值应当不同。
        let centre = (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5);
        assert_ne!(
            plain.sample(centre.0, centre.1),
            with_artwork.sample(centre.0, centre.1),
            "封面元素应当改变它自己矩形内的采样"
        );
        // 矩形外的像素不受影响。
        assert_eq!(
            plain.sample(10.0, 10.0),
            with_artwork.sample(10.0, 10.0),
            "封面元素矩形外不应当有变化"
        );
        // 圆角外（矩形左上角顶点附近）也应当是原背景。
        assert_eq!(
            plain.sample(rect.x + 1.0, rect.y + 1.0),
            with_artwork.sample(rect.x + 1.0, rect.y + 1.0),
            "圆角外应当是原背景"
        );
    }

    /// `floating` / `button` 是门控开关（参照目标 `if (!config.floating) continue;`）。
    #[test]
    fn floating_and_button_are_gating_switches() {
        // 默认值必须与参照目标一致。
        let defaults = LensConfig::default();
        assert!(!defaults.floating, "参照目标默认 floating: false");
        assert!(!defaults.button, "参照目标默认 button: false");

        // 不是按钮的面板：hover / 按下都不改变渲染参数。
        let hovered = PanelInteraction {
            hovered: true,
            ..Default::default()
        };
        let pressed = PanelInteraction {
            pressed: true,
            ..Default::default()
        };
        let plain = LensConfig::default();
        assert_eq!(button_config(plain, hovered).brightness, plain.brightness);
        assert_eq!(button_config(plain, pressed).z_radius, plain.z_radius);
        assert_eq!(
            button_config(plain, pressed).shadow_spread,
            plain.shadow_spread
        );

        // 是按钮的面板：hover 提亮 0.2、按下压平倒角并放大投影。
        let button = LensConfig {
            button: true,
            ..plain
        };
        assert!((button_config(button, hovered).brightness - (button.brightness + 0.2)).abs() < 1e-6);
        assert!((button_config(button, pressed).z_radius - button.z_radius * 0.8).abs() < 1e-6);
        assert!(
            (button_config(button, pressed).shadow_spread - button.shadow_spread * 1.2).abs() < 1e-6
        );

        // 详情页的两块面板可拖动，但不是按钮。
        let themed = LensConfig::for_theme(false, false);
        assert!(themed.floating);
        assert!(!themed.button);
    }

    /// 场景输入变化时缓存自动失效（对应参照目标的 `markChanged` / `_globalDirty`）。
    #[test]
    fn changing_the_scene_inputs_invalidates_the_caches() {
        let runtime = GlassRuntime::new();
        let key = PanelKey {
            x: 1,
            y: 2,
            width: 3,
            height: 4,
            config_hash: 5,
            reduced: false,
            solid: false,
        };
        let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(1, 1, vec![0u8, 0, 0, 255]).expect("1×1");
        let image = Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)));
        let artwork = |x: f32, y: f32| {
            Some(SceneArtwork {
                rect: PanelRect {
                    x,
                    y,
                    width: 280.0,
                    height: 280.0,
                },
                radius: PANEL_RADIUS,
            })
        };

        runtime.set_artwork(artwork(10.0, 20.0));
        runtime
            .inner
            .panels
            .lock()
            .unwrap()
            .insert(key, image.clone());
        // 亚像素抖动不算变化：缓存保留，避免每帧重算。
        runtime.set_artwork(artwork(10.2, 20.1));
        assert_eq!(
            runtime.inner.panels.lock().unwrap().len(),
            1,
            "亚像素抖动不应当清缓存"
        );
        // 明显移动 → 自动失效。
        runtime.set_artwork(artwork(60.0, 20.0));
        assert!(
            runtime.inner.panels.lock().unwrap().is_empty(),
            "封面元素移动后缓存应当自动失效"
        );
    }

    /// 把一块面板渲染成 PNG，方便肉眼比对
    /// （`cargo test --release -- --ignored dump_panel_preview --nocapture`）。
    #[test]
    #[ignore = "离线比对用"]
    fn dump_panel_preview() {
        const PAGE_WIDTH: f32 = 1280.0;
        const PAGE_HEIGHT: f32 = 720.0;
        let config = LensConfig::for_theme(true, false);
        let cover = newest_cached_cover()
            .and_then(|path| image::open(path).ok())
            .map(|image| image.to_rgba8())
            .unwrap_or_else(|| test_cover(320, 320));
        let scene = Scene::new(
            Some(Arc::new(cover)),
            [0.09, 0.09, 0.11],
            0.85,
            0.34,
            0.66,
            PAGE_WIDTH,
            PAGE_HEIGHT,
        );
        let rect = PanelRect {
            x: 352.0,
            y: 28.0,
            width: 880.0,
            height: 620.0,
        };
        let started = std::time::Instant::now();
        let panel = render_panel(&scene, rect, &config, &[], 1.0);
        println!(
            "面板渲染：{}×{}（含 20px 投影留白），耗时 {:.1} ms",
            panel.width,
            panel.height,
            started.elapsed().as_secs_f32() * 1000.0
        );

        // 背景：直接按场景公式光栅化整页。
        let mut canvas = RgbaImage::new(PAGE_WIDTH as u32, PAGE_HEIGHT as u32);
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let color = scene.sample(x as f32 + 0.5, y as f32 + 0.5);
                canvas.put_pixel(
                    x,
                    y,
                    Rgba([
                        to_byte(color[0]),
                        to_byte(color[1]),
                        to_byte(color[2]),
                        255,
                    ]),
                );
            }
        }
        let mut layer = RgbaImage::new(panel.width, panel.height);
        for y in 0..panel.height {
            for x in 0..panel.width {
                let rgb = panel.rgb(x, y);
                layer.put_pixel(x, y, Rgba([rgb[0], rgb[1], rgb[2], panel.alpha(x, y)]));
            }
        }
        let backdrop_path = std::env::temp_dir().join("wcmusic-glass-backdrop.png");
        canvas.save(&backdrop_path).expect("保存背景图");
        let panel_path = std::env::temp_dir().join("wcmusic-glass-panel.png");
        layer.save(&panel_path).expect("保存面板图");
        println!("背景图：{}", backdrop_path.display());
        println!("面板图：{}", panel_path.display());
        println!("面板放置：{},{}（含 20px 留白）", rect.x - SHADOW_PAD, rect.y - SHADOW_PAD);
    }

    /// 封面缓存里最新的一张 `-320.jpg`。
    fn newest_cached_cover() -> Option<std::path::PathBuf> {
        let dir = std::env::temp_dir().join("wcmusic-artwork");
        let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            let name = path.file_name()?.to_string_lossy().into_owned();
            if !name.ends_with("-320.jpg") {
                continue;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            if newest.as_ref().is_none_or(|(best, _)| modified > *best) {
                newest = Some((modified, path));
            }
        }
        newest.map(|(_, path)| path)
    }
}
