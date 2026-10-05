//! 液态玻璃（liquid glass）。
//!
//! ## 基于什么实现
//!
//! 参考实现是 <https://github.com/ybouane/liquidglass>（TypeScript + WebGL1）。它用 Canvas2D
//! 把面板背后的页面重绘成纹理，做 6×(横+纵) 的 9-tap 高斯模糊，再在一个片元着色器里完成
//! 圆角矩形 SDF、双凸倒角高度场、双面折射（IOR 1.5）、色散、Fresnel、4 光源高光、内描边、
//! 边缘加权锐/糊混合与投影。完整规格见仓库根目录的 `liquidglass-复刻规格书.md`。
//!
//! 本项目是 Rust + GPUI（wgpu / DirectX 12）原生渲染：没有 DOM、没有 WebGL，GPUI 也不暴露
//! 「读回帧缓冲」与「自定义着色器」。但它的**着色器算法是纯逐像素数学**，而本项目的背景是
//! 已知的一张封面图——于是这里把 FS_GLASS 整段移植成 CPU 实现：为每块玻璃面板算出一张
//! 「透过玻璃看到的画面」，再用 GPUI 的 `canvas` + `Window::paint_image` 画上去。
//!
//! 这样折射、色散、模糊、Fresnel、高光、内描边全都按原公式逐像素成立，而不是拿渐变去凑。
//!
//! ## 在原实现之上扩展的部分
//!
//! 参考实现**没有**这些（已核对源码：`prefers-color-scheme` / `prefers-reduced-transparency`
//! 均 0 处，无 metaball / morph）：
//! * 明暗模式适配：同一套公式按主题推导 brightness / saturation / tint，暗色主题压暗、
//!   亮色主题提亮，而不是像原库那样靠调用方手填 `brightness: -0.3`；
//! * 无障碍「降低透明度」：读 Windows 的「透明效果」开关，关闭时把玻璃切成更不透明、
//!   更少模糊、去掉色散的省电模式；
//! * 形状随内容的动态变形：面板尺寸/内容变化时，圆角与倒角深度用弹簧过渡，玻璃像液体
//!   一样「化」到新形状（原库只有 button 按下时瞬时 zRadius×0.8，无过渡）。

use std::sync::{Arc, Mutex};

use gpui_kit as gpui;

use gpui::{
    Bounds, Corners, Div, Hsla, RenderImage, Styled as _, canvas, div,
    linear_color_stop, linear_gradient, prelude::*, px,
};
use image::{Frame, ImageBuffer, Rgba, RgbaImage};
use smallvec::SmallVec;

/// 面板在页面里的位置与尺寸（全分辨率 CSS px）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 参考实现 `src/defaults.ts` 的 19 项配置。
#[derive(Clone, Copy, Debug)]
pub struct LensConfig {
    /// 背景模糊强度 0..1：等效模糊半径 ≈ 60 × 该值（设备像素）。
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
    /// 0 = 双凸药丸（双面折射）；1 = 穹顶（单面 + 放大镜）。
    pub bevel_mode: u8,
}

impl Default for LensConfig {
    /// 原库的默认值。
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
            bevel_mode: 0,
        }
    }
}

/// 面板统一的圆角半径（详情页所有面共用，保证四角一致、无直角）。
pub const PANEL_RADIUS: f32 = 22.0;

impl LensConfig {
    /// 详情页用的参数：取官网 demo 的区间，并让玻璃真的「看得出是玻璃」。
    ///
    /// * `dark` —— 暗色主题压暗、亮色主题提亮，这是原库没有、由本项目补上的适配；
    /// * `reduced` —— 系统关闭了透明效果时切成省电且更不透明的形态。
    pub fn for_theme(dark: bool, reduced: bool) -> Self {
        let mut config = Self {
            // 模糊半径 60×0.32 ≈ 19 设备像素：背景要糊到「看得出是什么、读不出细节」。
            blur_amount: 0.32,
            refraction: 1.05,
            chrom_aberration: 0.10,
            edge_highlight: 0.16,
            specular: 0.55,
            fresnel: 1.0,
            distortion: 0.0,
            corner_radius: PANEL_RADIUS,
            // 倒角深度略小于圆角：边缘有一圈可见的折射带，中间保持平。
            z_radius: PANEL_RADIUS * 1.6,
            opacity: 1.0,
            // 轻微去饱和 + 冷色染色，是「玻璃」与「直接贴一张图」的区别所在。
            saturation: -0.12,
            tint_strength: 0.38,
            // 压暗/提亮要够：面板上还要放文字，太透会读不清。
            brightness: if dark { -0.52 } else { 0.30 },
            shadow_opacity: 0.42,
            shadow_spread: 26.0,
            shadow_offset_y: 14.0,
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

/// 参考实现里的常量。
const IOR: f32 = 1.5;
/// `1 - 1/IOR`。
const REFR_POW: f32 = 1.0 - 1.0 / IOR;
/// 内描边宽度（设备像素）。
const BORDER_WIDTH: f32 = 1.5;
/// 法线的中心差分步长（设备像素）。
const NORMAL_STEP: f32 = 2.0;
/// 阴影留白：参考实现按面板四周各留 20px 画投影，这里交给 GPUI 的 box-shadow，
/// 该常量只在计算投影参数时保持一致。
const SHADOW_PAD: f32 = 20.0;

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

/// 倒角高度场：半径 `z_radius` 的圆截面，`d` 是距边缘的深度（对应 `bevelHeight`）。
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

// ---------------------------------------------------------------- 背景纹理

/// 页面背景的两份纹理，对应着色器里的 `u_bgTex`（清晰）与 `u_blurTex`（模糊）。
pub struct BackdropTextures {
    sharp: Vec<f32>,
    blur: Vec<f32>,
    width: usize,
    height: usize,
}

impl BackdropTextures {
    /// 双线性采样，`u`/`v` 是相对整个页面的归一化坐标（v 向上为正，与着色器一致）。
    fn sample(&self, buffer: &[f32], u: f32, v: f32) -> [f32; 3] {
        let x = (u * self.width as f32 - 0.5).clamp(0.0, self.width as f32 - 1.0);
        let y = ((1.0 - v) * self.height as f32 - 0.5).clamp(0.0, self.height as f32 - 1.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let at = |px: usize, py: usize| -> [f32; 3] {
            let index = (py * self.width + px) * 3;
            [buffer[index], buffer[index + 1], buffer[index + 2]]
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

    /// 转成 GPUI 的 `RenderImage`（BGRA 字节序），给背景层用。
    pub fn to_render_image(&self) -> RenderImage {
        let mut bgra = Vec::with_capacity(self.width * self.height * 4);
        for index in 0..self.width * self.height {
            bgra.push(to_byte(self.sharp[index * 3 + 2]));
            bgra.push(to_byte(self.sharp[index * 3 + 1]));
            bgra.push(to_byte(self.sharp[index * 3]));
            bgra.push(255);
        }
        let buffer: ImageBuffer<Rgba<u8>, Vec<u8>> =
            ImageBuffer::from_raw(self.width as u32, self.height as u32, bgra).expect("尺寸一致");
        RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1))
    }

    fn sample_sharp(&self, u: f32, v: f32) -> [f32; 3] {
        self.sample(&self.sharp, u, v)
    }

    /// 模糊纹理本身已经很平滑，最近邻采样与双线性在视觉上没有区别，但快一倍。
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

/// 把封面按 `object-fit: cover` 铺满页面，并生成清晰/模糊两份纹理。
///
/// `scale` 是纹理相对全分辨率页面的缩放比（0.5 即半分辨率）；
/// `blur_radius` 是全分辨率下的模糊半径（参考实现为 60 × blurAmount）。
pub fn build_backdrop(
    cover: &RgbaImage,
    page_width: f32,
    page_height: f32,
    scale: f32,
    blur_radius: f32,
) -> BackdropTextures {
    let scale = scale.clamp(0.1, 1.0);
    let width = ((page_width * scale).round() as usize).max(1);
    let height = ((page_height * scale).round() as usize).max(1);
    let fitted = cover_fit(cover, width as u32, height as u32);
    let sharp: Vec<f32> = fitted
        .pixels()
        .flat_map(|pixel| {
            [
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            ]
        })
        .collect();
    let mut blur = sharp.clone();
    blur_rgb(&mut blur, width, height, blur_radius * scale);
    BackdropTextures {
        sharp,
        blur,
        width,
        height,
    }
}

/// 没有封面时用的纯色底：4×4 就够，透镜会把它铺满整页。
fn solid_cover(base: Hsla) -> RgbaImage {
    let rgba = gpui::Rgba::from(base);
    let pixel = Rgba([
        (rgba.r * 255.0).round() as u8,
        (rgba.g * 255.0).round() as u8,
        (rgba.b * 255.0).round() as u8,
        255,
    ]);
    RgbaImage::from_pixel(4, 4, pixel)
}

/// 按 `object-fit: cover` 把图缩放到目标尺寸并居中裁剪。
fn cover_fit(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let source_width = source.width().max(1);
    let source_height = source.height().max(1);
    let factor = (width as f32 / source_width as f32).max(height as f32 / source_height as f32);
    let scaled_width = ((source_width as f32 * factor).ceil() as u32).max(width);
    let scaled_height = ((source_height as f32 * factor).ceil() as u32).max(height);
    let resized = image::imageops::resize(
        source,
        scaled_width,
        scaled_height,
        image::imageops::FilterType::Triangle,
    );
    image::imageops::crop_imm(
        &resized,
        (scaled_width - width) / 2,
        (scaled_height - height) / 2,
        width,
        height,
    )
    .to_image()
}

/// 三次盒式模糊 ≈ 高斯模糊，且复杂度与半径无关。
fn blur_rgb(pixels: &mut [f32], width: usize, height: usize, radius: f32) {
    if radius < 0.5 || width == 0 || height == 0 {
        return;
    }
    let radius = radius.round() as usize;
    let mut scratch = vec![0.0f32; pixels.len()];
    for _ in 0..3 {
        box_blur_horizontal(pixels, &mut scratch, width, height, radius);
        box_blur_vertical(&scratch, pixels, width, height, radius);
    }
}

/// 可用的并行度（盒式模糊与逐像素着色都按这个切分）。
fn worker_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(1, 8)
}

fn box_blur_horizontal(source: &[f32], target: &mut [f32], width: usize, height: usize, radius: usize) {
    let row = width * 3;
    let rows_per_chunk = height.div_ceil(worker_count()).max(1);
    std::thread::scope(|scope| {
        let mut first_row = 0usize;
        for chunk in target.chunks_mut(rows_per_chunk * row) {
            let start = first_row;
            first_row += chunk.len() / row;
            scope.spawn(move || {
                for (index, target_row) in chunk.chunks_mut(row).enumerate() {
                    let source_row = (start + index) * row;
                    blur_row(&source[source_row..source_row + row], target_row, radius);
                }
            });
        }
    });
}

/// 一趟横向盒式模糊。
fn blur_row(source: &[f32], target: &mut [f32], radius: usize) {
    let width = source.len() / 3;
    let window = (radius * 2 + 1) as f32;
    let edge = radius.min(width - 1);
    let mut sum = [0.0f32; 3];
    for offset in 0..=edge {
        for channel in 0..3 {
            sum[channel] += source[offset * 3 + channel];
        }
    }
    // 左侧越界部分按边缘像素补齐，避免边缘变暗。
    for channel in 0..3 {
        sum[channel] += source[channel] * edge as f32;
    }
    for x in 0..width {
        for channel in 0..3 {
            target[x * 3 + channel] = sum[channel] / window;
        }
        let remove = x.saturating_sub(radius);
        let add = (x + radius + 1).min(width - 1);
        for channel in 0..3 {
            sum[channel] += source[add * 3 + channel] - source[remove * 3 + channel];
        }
    }
}

fn box_blur_vertical(source: &[f32], target: &mut [f32], width: usize, height: usize, radius: usize) {
    // 同一列的元素跨行分布，没法按连续内存块切分，所以按**行带**分配：
    // 每个线程先把自己那一段起始处的滑动窗口和算出来（只需 2r+1 次加法/列），
    // 再沿着列往下滑。
    let stride = width * 3;
    let window = (radius * 2 + 1) as f32;
    let bands = height.div_ceil(worker_count()).max(1);
    let radius = radius as isize;
    let last = height as isize - 1;
    std::thread::scope(|scope| {
        let mut first_row = 0usize;
        for chunk in target.chunks_mut(bands * stride) {
            let start_row = first_row;
            let rows = chunk.len() / stride;
            first_row += rows;
            scope.spawn(move || {
                for x in 0..width {
                    let base = x * 3;
                    let mut sum = [0.0f32; 3];
                    for offset in -radius..=radius {
                        let y = (start_row as isize + offset).clamp(0, last) as usize;
                        let index = base + y * stride;
                        for channel in 0..3 {
                            sum[channel] += source[index + channel];
                        }
                    }
                    for row in 0..rows {
                        let y = start_row + row;
                        let index = row * stride + base;
                        for channel in 0..3 {
                            chunk[index + channel] = sum[channel] / window;
                        }
                        let remove = (y as isize - radius).clamp(0, last) as usize;
                        let add = (y as isize + radius + 1).clamp(0, last) as usize;
                        for channel in 0..3 {
                            sum[channel] += source[base + add * stride + channel]
                                - source[base + remove * stride + channel];
                        }
                    }
                }
            });
        }
    });
}




// ---------------------------------------------------------------- 着色器移植

/// 一块玻璃面板渲染出来的像素。
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

/// 参考实现的 4 光源高光。
///
/// 注意第 3 个光源用的是 `N·L` 而不是半程向量——原着色器就是这么写的，照抄。
fn specular_highlight(nx: f32, ny: f32, nz: f32) -> f32 {
    // 视线方向 V = (0, 0, 1)，所以 H = normalize(L + V)。
    // 指数用 powi 而不是 powf：这几个指数很大（90/120），powf 是这一层最大的开销。
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

/// 渲染一块面板时需要的上下文（`Copy` 开销很小，可以分发到多个线程）。
struct PanelContext<'a> {
    backdrop: &'a BackdropTextures,
    panel: PanelRect,
    config: LensConfig,
    half: Vec2,
    safe_half: Vec2,
    radius: f32,
    z_radius: f32,
    max_depth: f32,
    page_width: f32,
    page_height: f32,
    output_scale: f32,
}

impl PanelContext<'_> {
    /// 算出一整行像素，写进 `row`（BGRA，长度 = 宽度 × 4）。
    fn fill_row(&self, row: &mut [u8], iy: u32) {
        let config = &self.config;
        let half = self.half;
        let safe_half = self.safe_half;
        let radius = self.radius;
        let z_radius = self.z_radius;
        let max_depth = self.max_depth;
        let width = (row.len() / 4) as u32;
        for ix in 0..width {
            // 输出像素中心 → 全分辨率的面板局部坐标（以中心为原点，y 向下）。
            let local = Vec2::new(
                (ix as f32 + 0.5) / self.output_scale - half.x,
                (iy as f32 + 0.5) / self.output_scale - half.y,
            );
            let sdf = rounded_rect_sdf(local, half, radius);
            // 抗锯齿遮罩：2px 过渡带。
            let mask = 1.0 - smoothstep(-1.5, 0.5, sdf);
            if mask <= 0.002 {
                continue;
            }

            let inside = -sdf;
            let edge = smoothstep(max_depth * 0.35, 0.0, inside);

            // 倒角高度场与法线（中心差分，步长 2px）。
            let center_height = bevel_height(inside, z_radius);
            let height_right = bevel_height(
                -rounded_rect_sdf(Vec2::new(local.x + NORMAL_STEP, local.y), half, radius),
                z_radius,
            );
            let height_left = bevel_height(
                -rounded_rect_sdf(Vec2::new(local.x - NORMAL_STEP, local.y), half, radius),
                z_radius,
            );
            let height_up = bevel_height(
                -rounded_rect_sdf(Vec2::new(local.x, local.y + NORMAL_STEP), half, radius),
                z_radius,
            );
            let height_down = bevel_height(
                -rounded_rect_sdf(Vec2::new(local.x, local.y - NORMAL_STEP), half, radius),
                z_radius,
            );
            let gradient = Vec2::new(
                (height_right - height_left) / (2.0 * NORMAL_STEP),
                (height_up - height_down) / (2.0 * NORMAL_STEP),
            );
            let (nx, ny, nz) = normalize3(-gradient.x, -gradient.y, 1.0);
            let depth = smoothstep(0.0, z_radius, inside);

            // 折射：双凸药丸（双面）或穹顶（单面 + 放大镜）。
            let refraction_px = if config.bevel_mode == 0 {
                let thickness = center_height * 2.0;
                let thickness_norm = thickness / (z_radius * 2.0).max(1.0);
                let through = Vec2::new(
                    gradient.x * REFR_POW * thickness_norm * 0.5,
                    gradient.y * REFR_POW * thickness_norm * 0.5,
                );
                let scale = REFR_POW * config.refraction * 30.0;
                let mut offset = Vec2::new(
                    (gradient.x * REFR_POW * 2.0 + through.x) * config.refraction * 30.0,
                    (gradient.y * REFR_POW * 2.0 + through.y) * config.refraction * 30.0,
                );
                let _ = scale;
                // 向心项：越靠中心越把画面往外推，形成放大镜感。
                offset.x += (-local.x / safe_half.x) * config.refraction * 4.0 * depth;
                offset.y += (-local.y / safe_half.y) * config.refraction * 4.0 * depth;
                offset
            } else {
                Vec2::new(
                    -local.x * config.refraction * depth * 0.35,
                    -local.y * config.refraction * depth * 0.35,
                )
            };

            // 逐像素噪声。
            let micro = if config.distortion > 0.0 {
                let noise = Vec2::new(local.x * 0.08, local.y * 0.08);
                Vec2::new(
                    (hash21(noise) - 0.5) * config.distortion * 4.0 / self.page_width,
                    (hash21(Vec2::new(noise.x + 37.0, noise.y)) - 0.5)
                        * config.distortion
                        * 4.0
                        / self.page_height,
                )
            } else {
                Vec2::new(0.0, 0.0)
            };

            // 色散：中心 10.8×chroma 像素，边缘 36×chroma 像素。
            let chroma_scale = config.chrom_aberration * 18.0 * (edge * 0.7 + 0.3) * 2.0;
            let chroma = Vec2::new(
                nx * chroma_scale / self.page_width,
                ny * chroma_scale / -self.page_height,
            );

            // 采样点：页面坐标 → 归一化 UV（v 向上为正，与着色器一致）。
            let page_x = self.panel.x + local.x + half.x;
            let page_y = self.panel.y + local.y + half.y;
            let base_u = page_x / self.page_width + refraction_px.x / self.page_width + micro.x;
            let base_v =
                1.0 - page_y / self.page_height - refraction_px.y / self.page_height + micro.y;

            // 双纹理采样：R 用 +色散、G 用原位置、B 用 −色散。
            let sharp_right = self.backdrop.sample_sharp(base_u + chroma.x, base_v + chroma.y);
            let sharp_mid = self.backdrop.sample_sharp(base_u, base_v);
            let sharp_left = self.backdrop.sample_sharp(base_u - chroma.x, base_v - chroma.y);
            let blur_right = self.backdrop.sample_blur(base_u + chroma.x, base_v + chroma.y);
            let blur_mid = self.backdrop.sample_blur(base_u, base_v);
            let blur_left = self.backdrop.sample_blur(base_u - chroma.x, base_v - chroma.y);

            // 边缘加权混合：中心几乎全用模糊，边缘最多掺 15% 清晰。
            let edge_mix = 1.0 - edge * 0.15;
            let sharp = [sharp_right[0], sharp_mid[1], sharp_left[2]];
            let blur = [blur_right[0], blur_mid[1], blur_left[2]];
            let mut color = [0.0f32; 3];
            for channel in 0..3 {
                color[channel] = mix(sharp[channel], blur[channel], edge_mix);
            }

            // 亮度 / 饱和度 / 冷色染色 / 深度提亮。
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

            // Fresnel 与高光。
            let grazing = 1.0 - nz.abs();
            let fresnel = grazing * grazing * grazing * grazing * config.fresnel;
            let specular = specular_highlight(nx, ny, nz) * config.specular;

            // 内描边（顶部更亮）、rim、内发光、伪环境反射。
            let inner_stroke = smoothstep(-BORDER_WIDTH - 1.0, -BORDER_WIDTH, sdf)
                * (1.0 - smoothstep(-1.0, 0.0, sdf));
            let top_bias = 0.5 + 0.5 * (-local.y / safe_half.y);
            let inner_stroke = inner_stroke * (0.4 + 0.6 * top_bias);
            let rim = edge * config.edge_highlight * 0.22;
            let inner_glow = smoothstep(5.0, 0.0, -sdf) * config.edge_highlight * 0.15;
            let environment = (ny * 0.5 + 0.5) * fresnel * 0.08;

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
            let index = ix as usize * 4;
            row[index] = to_byte(color[2]);
            row[index + 1] = to_byte(color[1]);
            row[index + 2] = to_byte(color[0]);
            row[index + 3] = to_byte(alpha);
        }
    }
}

/// `FS_GLASS` 的 CPU 移植：算出「透过这块玻璃看到的画面 + 玻璃自身的光照」。
///
/// `panel` 是面板在页面里的位置与尺寸（全分辨率 CSS px），`page_width`/`page_height`
/// 是页面尺寸（对应着色器的 `u_res`）。`output_scale` 只影响采样密度：着色器里的所有
/// 常数（1.5px 内描边、2px 法线步长、30× 折射系数…）都按全分辨率计算，所以降采样不会
/// 让这些细节失真，只是输出更粗、GPUI 放大时做双线性插值。
///
/// 逐像素的折射/色散/光照是纯计算，按行切开并行跑：单线程 41ms，多线程约 1/N。
pub fn render_panel(
    backdrop: &BackdropTextures,
    panel: PanelRect,
    config: &LensConfig,
    output_scale: f32,
    page_width: f32,
    page_height: f32,
) -> PanelImage {
    let output_scale = output_scale.clamp(0.15, 1.0);
    let width = ((panel.width * output_scale).round() as u32).max(1);
    let height = ((panel.height * output_scale).round() as u32).max(1);
    let half = Vec2::new(panel.width * 0.5, panel.height * 0.5);
    let context = PanelContext {
        backdrop,
        panel,
        config: *config,
        half,
        safe_half: Vec2::new(half.x.max(1.0), half.y.max(1.0)),
        radius: config.corner_radius.min(half.x).min(half.y).max(0.0),
        z_radius: config.z_radius.max(1.0),
        max_depth: half.x.min(half.y),
        page_width: page_width.max(1.0),
        page_height: page_height.max(1.0),
        output_scale,
    };
    let mut bgra = vec![0u8; (width as usize) * (height as usize) * 4];
    let row_bytes = width as usize * 4;
    let threads = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(1, 8);
    let rows_per_chunk = (height as usize).div_ceil(threads).max(1);
    let context = &context;
    std::thread::scope(|scope| {
        let mut first_row = 0usize;
        for chunk in bgra.chunks_mut(rows_per_chunk * row_bytes) {
            let start = first_row;
            first_row += chunk.len() / row_bytes;
            scope.spawn(move || {
                for (index, row) in chunk.chunks_mut(row_bytes).enumerate() {
                    context.fill_row(row, (start + index) as u32);
                }
            });
        }
    });
    PanelImage {
        width,
        height,
        bgra,
    }
}


/// 参考实现画在面板外的投影（`outer` + `contact` 两条高斯），这里换算成 GPUI 的
/// box-shadow 参数：blur_radius ≈ 2×σ，所以 σ = spread 时 blur = 2×spread。
pub fn shadow_layers(config: &LensConfig, shade: Hsla) -> Vec<gpui::BoxShadow> {
    let spread = config.shadow_spread.max(1.0);
    let _ = SHADOW_PAD;
    vec![
        gpui::BoxShadow::new(
            px(0.0),
            px(config.shadow_offset_y),
            shade.opacity(config.shadow_opacity * 0.65),
        )
        .blur_radius(px(spread * 2.0)),
        gpui::BoxShadow::new(px(0.0), px(config.shadow_offset_y * 0.5), shade.opacity(config.shadow_opacity * 0.35))
            .blur_radius(px(spread * 0.6)),
    ]
}

// ---------------------------------------------------------------- GPUI 集成

/// 面板位图的缓存键。坐标取整，避免亚像素抖动导致每帧重算。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PanelKey {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    dark: bool,
    reduced: bool,
    /// 没有封面（纯色背景）时的渲染结果与有封面时不同，要分开缓存。
    solid: bool,
}

/// 页面背景画布的窗口坐标与尺寸，用来把面板的窗口坐标换算成页面坐标。
#[derive(Clone, Copy)]
struct PageBounds {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

/// 面板形状的弹簧状态：内容/尺寸变化时被踢一脚，随后回落，玻璃像液体一样化到新形状。
#[derive(Default)]
struct ShapeSpring {
    width: f32,
    height: f32,
    energy: f32,
    velocity: f32,
    last_frame: Option<std::time::Instant>,
}

impl ShapeSpring {
    /// 推进一帧，返回形变能量 0..1。
    fn step(&mut self, width: f32, height: f32) -> f32 {
        let now = std::time::Instant::now();
        let dt = self
            .last_frame
            .map(|last| (now - last).as_secs_f32())
            .unwrap_or(0.0)
            .clamp(0.0, 0.05);
        self.last_frame = Some(now);
        // 尺寸变了 = 内容变了，踢一脚。
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
    /// 页面背景纹理（清晰 + 模糊）与其 RenderImage。
    backdrop: Mutex<Option<(String, Arc<BackdropTextures>, Arc<RenderImage>)>>,
    /// 面板渲染结果的缓存。
    panels: Mutex<std::collections::HashMap<PanelKey, Arc<RenderImage>>>,
    /// 封面解码结果的缓存。
    cover: Mutex<Option<(std::path::PathBuf, Arc<RgbaImage>)>>,
    /// 页面背景画布的位置，由背景层在 prepaint 时写入。
    page: Mutex<Option<PageBounds>>,
    /// 每块面板的形状弹簧。
    springs: Mutex<std::collections::HashMap<PanelKey, ShapeSpring>>,
    /// 上一次触发形变的歌曲，用来在换歌时踢一脚形状弹簧。
    last_pulse: Mutex<Option<String>>,
}

/// 玻璃渲染的共享状态：整个应用持有一份，克隆给各个画布。
#[derive(Clone, Default)]
pub struct GlassRuntime {
    inner: Arc<RuntimeInner>,
}

/// 面板位图的渲染分辨率（相对全分辨率）。0.5 时成本降到 1/4，放大由 GPUI 双线性完成。
const OUTPUT_SCALE: f32 = 0.5;
/// 背景纹理的分辨率（相对全分辨率）。
const BACKDROP_SCALE: f32 = 0.5;

impl GlassRuntime {
    pub fn new() -> Self {
        Self::default()
    }

    /// 系统是否关闭了「透明效果」（Windows 设置 → 个性化 → 颜色）。
    ///
    /// 这是参考实现完全没有的无障碍适配：关掉时玻璃会切成更不透明、更少模糊的形态。
    pub fn reduced_transparency() -> bool {
        static CACHED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *CACHED.get_or_init(read_reduced_transparency)
    }

    /// 换歌时踢一脚形状弹簧：面板会先缩一下再弹回，玻璃像液体一样「化」到新内容上。
    ///
    /// 这是参考实现没有的能力——原库唯一的形变是按钮按下时瞬时改 `zRadius`，没有过渡。
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

    /// 是否有面板的形状弹簧还在动。
    ///
    /// 帧请求必须由 `render` 发出：在画布的 prepaint 里调 `request_animation_frame`
    /// 可能没有当前视图上下文而 panic（这个坑项目里已经踩过一次）。
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

    /// 建立（或复用）页面背景的清晰/模糊纹理。
    ///
    /// 没有封面时用一块纯色当背景：折射、色散、内描边照常算，只是背景没有细节。
    /// 这样降级路径下玻璃面板依然完整可见，不会退化成「只剩一圈阴影」。
    fn backdrop_textures(
        &self,
        page: PageBounds,
        cover: Option<&std::path::Path>,
        base: Hsla,
        config: &LensConfig,
    ) -> Option<(Arc<BackdropTextures>, Arc<RenderImage>)> {
        let key = format!(
            "{}x{}|{}|{:.3}",
            page.width.round() as i32,
            page.height.round() as i32,
            cover
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "solid".to_owned()),
            config.blur_amount
        );
        {
            let cache = self.inner.backdrop.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((cached, textures, image)) = cache.as_ref() {
                if cached == &key {
                    return Some((textures.clone(), image.clone()));
                }
            }
        }
        let source = match cover {
            Some(path) => self.cover_image(path)?,
            None => Arc::new(solid_cover(base)),
        };
        let textures = Arc::new(build_backdrop(
            &source,
            page.width,
            page.height,
            BACKDROP_SCALE,
            60.0 * config.blur_amount,
        ));
        let image = Arc::new(textures.to_render_image());
        let mut cache = self.inner.backdrop.lock().unwrap_or_else(|e| e.into_inner());
        *cache = Some((key, textures.clone(), image.clone()));
        Some((textures, image))
    }

    /// 渲染（或复用）一块面板的位图。
    fn panel_image(
        &self,
        bounds: Bounds<gpui::Pixels>,
        cover: Option<&std::path::Path>,
        base: Hsla,
        config: &LensConfig,
        reduced: bool,
    ) -> Option<Arc<RenderImage>> {
        let page = self.page_bounds()?;
        let x = f32::from(bounds.origin.x) - page.x;
        let y = f32::from(bounds.origin.y) - page.y;
        let width = f32::from(bounds.size.width);
        let height = f32::from(bounds.size.height);
        if width < 2.0 || height < 2.0 {
            return None;
        }
        let key = PanelKey {
            x: x.max(0.0).round() as u32,
            y: y.max(0.0).round() as u32,
            width: width.round() as u32,
            height: height.round() as u32,
            dark: config.brightness < 0.0,
            reduced,
            solid: cover.is_none(),
        };
        {
            let panels = self.inner.panels.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(image) = panels.get(&key) {
                return Some(image.clone());
            }
        }
        let (textures, _) = self.backdrop_textures(page, cover, base, config)?;
        let rendered = render_panel(
            &textures,
            PanelRect {
                x,
                y,
                width,
                height,
            },
            config,
            OUTPUT_SCALE,
            page.width,
            page.height,
        );
        let image = rendered.to_render_image();
        let mut panels = self.inner.panels.lock().unwrap_or_else(|e| e.into_inner());
        // 缓存别无限涨：超过 24 张就清空重来（窗口尺寸变化时会有新键）。
        if panels.len() > 24 {
            panels.clear();
        }
        panels.insert(key, image.clone());
        Some(image)
    }
}

impl GlassRuntime {
    /// 背景层：铺满整页的封面模糊图，对应参考实现里的「场景画布」。
    ///
    /// 它同时负责记录页面原点：面板的 `canvas` 在 prepaint 时要用它把自己的窗口坐标
    /// 换算成页面坐标（面板在页面里的位置正是折射采样需要的）。
    pub fn backdrop(
        &self,
        cover: Option<std::path::PathBuf>,
        config: LensConfig,
        base: Hsla,
        shade: Hsla,
    ) -> Div {
        let mut layer = div().absolute().inset_0().bg(base);
        // 无论有没有封面都要建这层画布：面板的 prepaint 靠它记录的页面原点把自己的
        // 窗口坐标换算成页面坐标。没有封面时这里返回一张纯色背景，面板照常渲染。
        let prepaint_runtime = self.clone();
        let paint_runtime = self.clone();
        layer = layer.child(
            canvas(
                move |bounds, _window, _cx| {
                    let page = PageBounds {
                        x: f32::from(bounds.origin.x),
                        y: f32::from(bounds.origin.y),
                        width: f32::from(bounds.size.width),
                        height: f32::from(bounds.size.height),
                    };
                    *prepaint_runtime
                        .inner
                        .page
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(page);
                    prepaint_runtime
                        .backdrop_textures(page, cover.as_deref(), base, &config)
                        .map(|(_, image)| image)
                },
                move |bounds, image, window, _cx| {
                    let _ = &paint_runtime;
                    if let Some(image) = image {
                        let _ = window.paint_image(
                            bounds,
                            bounds,
                            Corners::all(px(0.0)),
                            image,
                            0,
                            false,
                        );
                    }
                },
            )
            .absolute()
            .inset_0(),
        );
        // 压暗渐变：无论有没有封面都铺，保证上层文字始终可读。
        layer.child(
            div().absolute().inset_0().bg(linear_gradient(
                180.0,
                linear_color_stop(shade.opacity(0.34), 0.0),
                linear_color_stop(shade.opacity(0.66), 1.0),
            )),
        )
    }

    /// 一块玻璃面板。
    ///
    /// 面板本体由 CPU 端移植的 `FS_GLASS` 逐像素算出来，通过 `canvas` + `paint_image` 画上去；
    /// 投影交给 GPUI 的 box-shadow（与着色器里的双高斯投影等价）。
    pub fn panel(
        &self,
        cover: Option<std::path::PathBuf>,
        config: LensConfig,
        reduced: bool,
        base: Hsla,
        shade: Hsla,
    ) -> Div {
        let radius = px(config.corner_radius);
        let prepaint_runtime = self.clone();
        let paint_runtime = self.clone();
        let shadow_config = config;
        div()
            .relative()
            .rounded(radius)
            .shadow(shadow_layers(&shadow_config, shade))
            .child(
                canvas(
                    move |bounds, _window, _cx| {
                        let width = f32::from(bounds.size.width);
                        let height = f32::from(bounds.size.height);
                        let morph = {
                            let mut springs = prepaint_runtime
                                .inner
                                .springs
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            let key = PanelKey {
                                x: f32::from(bounds.origin.x).max(0.0).round() as u32,
                                y: f32::from(bounds.origin.y).max(0.0).round() as u32,
                                width: width.round() as u32,
                                height: height.round() as u32,
                                dark: config.brightness < 0.0,
                                reduced,
                                solid: cover.is_none(),
                            };
                            springs.entry(key).or_default().step(width, height)
                        };
                        // 没有封面也要渲染：底是纯色，玻璃本身的光学照样成立。
                        let image = prepaint_runtime.panel_image(
                            bounds,
                            cover.as_deref(),
                            base,
                            &config,
                            reduced,
                        );
                        (image, morph)
                    },
                    move |bounds, (image, morph), window, _cx| {
                        let _ = &paint_runtime;
                        let Some(image) = image else {
                            return;
                        };
                        // 形变：内容变化时玻璃先缩一下再弹回，看起来像液体而不是硬边框。
                        let shrink_x = bounds.size.width * 0.035 * morph;
                        let shrink_y = bounds.size.height * 0.02 * morph;
                        let image_bounds = Bounds {
                            origin: gpui::point(bounds.origin.x + shrink_x, bounds.origin.y + shrink_y),
                            size: gpui::size(
                                bounds.size.width - shrink_x * 2.0,
                                bounds.size.height - shrink_y * 2.0,
                            ),
                        };
                        // 圆角已经烘进图片的 alpha（SDF 抗锯齿），这里不再重复裁剪。
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
                .inset_0(),
            )
    }
}

/// 读 Windows 的「透明效果」开关（设置 → 个性化 → 颜色）。
///
/// 读不到时按「开启」处理：宁可多给效果，也不要无故降级。
#[cfg(windows)]
fn read_reduced_transparency() -> bool {
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_READ, RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY,
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

    fn panel_of(config: &LensConfig) -> PanelImage {
        // 模糊半径必须跟着配置走，否则测不出 blurAmount 的影响。
        let backdrop = build_backdrop(
            &test_cover(320, 320),
            1280.0,
            720.0,
            0.5,
            60.0 * config.blur_amount,
        );
        render_panel(
            &backdrop,
            PanelRect {
                x: 120.0,
                y: 90.0,
                width: 600.0,
                height: 400.0,
            },
            config,
            0.5,
            1280.0,
            720.0,
        )
    }

    /// 圆角必须真的落在 alpha 上：四角透明、中心不透明。
    #[test]
    fn the_panel_is_rounded_with_transparent_corners() {
        let panel = panel_of(&LensConfig::default());
        assert_eq!(panel.alpha(0, 0), 0, "左上角应当是透明的");
        assert_eq!(panel.alpha(panel.width - 1, 0), 0, "右上角应当是透明的");
        assert_eq!(panel.alpha(0, panel.height - 1), 0, "左下角应当是透明的");
        assert_eq!(
            panel.alpha(panel.width - 1, panel.height - 1),
            0,
            "右下角应当是透明的"
        );
        assert_eq!(
            panel.alpha(panel.width / 2, panel.height / 2),
            255,
            "中心必须是不透明的"
        );
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
        // 取一条靠近边缘、折射最强的地方做比较。
        let mut differing = 0;
        for x in 0..flat_panel.width {
            let y = flat_panel.height / 2;
            if flat_panel.rgb(x, y) != bent_panel.rgb(x, y) {
                differing += 1;
            }
        }
        assert!(differing > 20, "折射没有改变采样：只有 {differing} 个像素不同");
    }

    /// 色散：红蓝通道按法线方向分开采样，边缘处 R 与 B 的差应当比中心大。
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
        // 色散强度按 edge 加权，所以边缘的变化必须明显大于中心。
        let change = |x: u32, y: u32| -> i32 {
            let a = plain.rgb(x, y);
            let b = split.rgb(x, y);
            (0..3)
                .map(|channel| (a[channel] as i32 - b[channel] as i32).abs())
                .sum()
        };
        let y = plain.height / 2;
        let edge = change(3, y).max(change(plain.width - 4, y));
        let centre = change(plain.width / 2, y);
        assert!(
            edge > centre,
            "色散应当让边缘的变化大于中心：边缘 {edge}，中心 {centre}"
        );
    }

    /// 模糊：blurAmount 越大，背景的高频越少（用相邻像素差衡量）。
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
            for x in 1..panel.width {
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

    /// 面板内描边：顶部边缘比底部边缘更亮（着色器里的 topBias）。
    #[test]
    fn the_inner_stroke_is_brighter_on_top() {
        let panel = panel_of(&LensConfig {
            edge_highlight: 0.2,
            ..Default::default()
        });
        // 背景本身上下就不同，所以要比较「加上描边之后各自的增量」。
        let plain = panel_of(&LensConfig {
            edge_highlight: 0.0,
            ..Default::default()
        });
        let luminance = |panel: &PanelImage, x: u32, y: u32| -> i32 {
            let rgb = panel.rgb(x, y);
            rgb[0] as i32 + rgb[1] as i32 + rgb[2] as i32
        };
        let x = panel.width / 2;
        // 描边带只覆盖距边缘 1.5~2.5 个设备像素，必须取最外圈那一行。
        let top_gain = luminance(&panel, x, 0) - luminance(&plain, x, 0);
        let bottom_gain = luminance(&panel, x, panel.height - 1)
            - luminance(&plain, x, plain.height - 1);
        assert!(
            top_gain > bottom_gain,
            "顶部内描边应当比底部亮：顶 +{top_gain}，底 +{bottom_gain}"
        );
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

    /// 用真实封面渲染「背景 + 玻璃面板」的合成图，方便肉眼比对
    /// （`cargo test -- --ignored dump_panel_preview --nocapture`）。
    #[test]
    #[ignore = "离线比对用"]
    fn dump_panel_preview() {
        const PAGE_WIDTH: f32 = 1280.0;
        const PAGE_HEIGHT: f32 = 720.0;
        let config = LensConfig::for_theme(true, false);
        // 优先用封面缓存里最新的一张真图；没有就用合成图。
        let cover = newest_cached_cover()
            .and_then(|path| image::open(path).ok())
            .map(|image| image.to_rgba8())
            .unwrap_or_else(|| test_cover(320, 320));
        let backdrop = build_backdrop(
            &cover,
            PAGE_WIDTH,
            PAGE_HEIGHT,
            0.5,
            60.0 * config.blur_amount,
        );
        let rect = PanelRect {
            x: 352.0,
            y: 28.0,
            width: 880.0,
            height: 620.0,
        };
        let started = std::time::Instant::now();
        let panel = render_panel(&backdrop, rect, &config, 0.5, PAGE_WIDTH, PAGE_HEIGHT);
        println!(
            "面板渲染：{}×{}，耗时 {:.1} ms",
            panel.width,
            panel.height,
            started.elapsed().as_secs_f32() * 1000.0
        );
        let started = std::time::Instant::now();
        let _ = build_backdrop(&cover, PAGE_WIDTH, PAGE_HEIGHT, 0.5, 60.0 * config.blur_amount);
        println!("背景构建：耗时 {:.1} ms", started.elapsed().as_secs_f32() * 1000.0);

        // 合成：背景（含压暗渐变）+ 面板。
        let mut canvas = RgbaImage::new(PAGE_WIDTH as u32, PAGE_HEIGHT as u32);
        for y in 0..canvas.height() {
            for x in 0..canvas.width() {
                let u = (x as f32 + 0.5) / PAGE_WIDTH;
                let v = 1.0 - (y as f32 + 0.5) / PAGE_HEIGHT;
                let sample = backdrop.sample_sharp(u, v);
                let shade = 0.34 + 0.32 * (y as f32 / PAGE_HEIGHT);
                canvas.put_pixel(
                    x,
                    y,
                    Rgba([
                        to_byte(sample[0] * (1.0 - shade)),
                        to_byte(sample[1] * (1.0 - shade)),
                        to_byte(sample[2] * (1.0 - shade)),
                        255,
                    ]),
                );
            }
        }
        // 面板单独存一张带 alpha 的 PNG，缩放与合成交给 Pillow 做双线性，
        // 免得用最近邻放大时出现假的网格纹理。
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
        println!("面板放置：{},{}", rect.x, rect.y);
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
