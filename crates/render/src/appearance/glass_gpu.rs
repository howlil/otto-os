use std::cell::RefCell;
use std::rc::{Rc, Weak};

use otto_core::{GlassBackdrop, GlassBitmap, GlassParams, GlassShape, RgbBuffer};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_UNKNOWN;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
use windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_BOX,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAP_WRITE_DISCARD,
    D3D11_MAPPED_SUBRESOURCE, D3D11_RESOURCE_MISC_FLAG, D3D11_SDK_VERSION,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SUBRESOURCE_DATA, D3D11_TEX2D_SRV, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_DYNAMIC, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
    D3D11CreateDevice, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, ID3D11PixelShader,
    ID3D11RenderTargetView, ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_GPU_PREFERENCE_MINIMUM_POWER, IDXGIAdapter, IDXGIAdapter1,
    IDXGIFactory6,
};
use windows::core::Interface;

const TARGET_BUCKET: u32 = 64;

fn bucket_dim(value: u32) -> u32 {
    value.max(1).next_multiple_of(TARGET_BUCKET)
}

const VS_BYTES: &[u8] = include_bytes!("../shaders/liquid_glass_vs.dxbc");
const PS_BYTES: &[u8] = include_bytes!("../shaders/liquid_glass_ps.dxbc");

#[repr(C)]
#[derive(Clone, Copy)]
struct LiquidGlassConstants {
    bounds: [f32; 4],
    shape_rect: [f32; 4],
    shape_extra: [f32; 4],
    light_and_sharp: [f32; 4],
    soft_and_tint: [f32; 4],
    tint_and_grade: [f32; 4],
    optics_a: [f32; 4],
    optics_b: [f32; 4],
    optics_c: [f32; 4],
    optics_d: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct SoftCacheKey {
    pub(crate) generation: u64,
    pub(crate) dpi: u32,
    pub(crate) theme: otto_core::ResolvedTheme,
    pub(crate) style: otto_core::AppearanceStyle,
    pub(crate) tint_key: u32,
}

struct RenderTarget {
    width: u32,
    height: u32,
    texture: ID3D11Texture2D,
    rtv: ID3D11RenderTargetView,
    staging: ID3D11Texture2D,
}

struct TextureSlot<K> {
    key: K,
    width: u32,
    height: u32,
    texture: ID3D11Texture2D,
    srv: ID3D11ShaderResourceView,
}

struct SharedGpu {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    constants: ID3D11Buffer,
}

thread_local! {
    static SHARED_GPU: RefCell<Weak<SharedGpu>> = const { RefCell::new(Weak::new()) };
}

pub fn release_shared_gpu() {
    let _ = SHARED_GPU.try_with(|cell| {
        if let Ok(mut slot) = cell.try_borrow_mut() {
            *slot = Weak::new();
        }
    });
}

fn low_power_adapter() -> Option<IDXGIAdapter> {
    let factory: IDXGIFactory6 = unsafe { CreateDXGIFactory1() }.ok()?;
    let adapter: IDXGIAdapter1 =
        unsafe { factory.EnumAdapterByGpuPreference(0, DXGI_GPU_PREFERENCE_MINIMUM_POWER) }.ok()?;
    adapter.cast().ok()
}

fn create_shared_gpu() -> Option<SharedGpu> {
    let mut device = None;
    let mut context = None;
    let levels = [D3D_FEATURE_LEVEL_11_0];
    let adapter = low_power_adapter();
    let driver = if adapter.is_some() {
        D3D_DRIVER_TYPE_UNKNOWN
    } else {
        D3D_DRIVER_TYPE_HARDWARE
    };
    let created = unsafe {
        D3D11CreateDevice(
            adapter.as_ref(),
            driver,
            windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
    };
    if created.is_err() {
        return None;
    }
    let device = device?;
    let context = context?;

    let mut vs = None;
    let mut ps = None;
    unsafe {
        device
            .CreateVertexShader(VS_BYTES, None, Some(&mut vs))
            .ok()?;
        device
            .CreatePixelShader(PS_BYTES, None, Some(&mut ps))
            .ok()?;
    }

    let cb_desc = D3D11_BUFFER_DESC {
        ByteWidth: u32::try_from(size_of::<LiquidGlassConstants>()).ok()?,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut constants = None;
    unsafe {
        device
            .CreateBuffer(&cb_desc, None, Some(&mut constants))
            .ok()?;
    }

    Some(SharedGpu {
        device,
        context,
        vs: vs?,
        ps: ps?,
        constants: constants?,
    })
}

fn shared_gpu() -> Option<Rc<SharedGpu>> {
    SHARED_GPU.with(|cell| {
        let mut slot = cell.try_borrow_mut().ok()?;
        if let Some(live) = slot.upgrade() {
            if unsafe { live.device.GetDeviceRemovedReason() }.is_ok() {
                return Some(live);
            }
            *slot = Weak::new();
        }
        let created = Rc::new(create_shared_gpu()?);
        *slot = Rc::downgrade(&created);
        Some(created)
    })
}

pub(crate) struct GpuGlassComposer {
    shared: Rc<SharedGpu>,
    sharp_cache: Option<TextureSlot<u64>>,
    soft_cache: Option<TextureSlot<SoftCacheKey>>,
    target: Option<RenderTarget>,
    upload_scratch: Vec<u8>,
}

impl GpuGlassComposer {
    pub(crate) fn new() -> Option<Self> {
        Some(Self {
            shared: shared_gpu()?,
            sharp_cache: None,
            soft_cache: None,
            target: None,
            upload_scratch: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn clear_caches(&mut self) {
        self.sharp_cache = None;
        self.soft_cache = None;
        self.target = None;
        self.upload_scratch.clear();
        self.upload_scratch.shrink_to_fit();
    }

    pub(crate) fn is_device_lost(&self) -> bool {
        unsafe { self.shared.device.GetDeviceRemovedReason() }.is_err()
    }

    pub(crate) fn compose(
        &mut self,
        generation: u64,
        soft_key: SoftCacheKey,
        params: &GlassParams,
        shape: &GlassShape,
        backdrop: &GlassBackdrop<'_>,
        light_dir: (f32, f32),
    ) -> Option<GlassBitmap> {
        let bounds = shape.pixel_bounds();
        let width = u32::try_from(bounds.width).ok()?;
        let height = u32::try_from(bounds.height).ok()?;
        if width == 0
            || height == 0
            || backdrop.sharp.width() == 0
            || backdrop.sharp.height() == 0
            || backdrop.soft.width() == 0
            || backdrop.soft.height() == 0
        {
            return None;
        }

        self.ensure_sharp_srv(generation, backdrop.sharp)?;
        self.ensure_soft_srv(soft_key, backdrop.soft)?;
        self.ensure_target(width, height)?;

        let cb_data = build_constants(&bounds, shape, params, backdrop, light_dir);
        self.upload_constants(&cb_data)?;

        let shared = Rc::clone(&self.shared);
        let sharp_srv = self.sharp_cache.as_ref()?.srv.clone();
        let soft_srv = self.soft_cache.as_ref()?.srv.clone();
        let target = self.target.as_ref()?;

        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        let src_box = D3D11_BOX {
            left: 0,
            top: 0,
            front: 0,
            right: width,
            bottom: height,
            back: 1,
        };

        unsafe {
            shared
                .context
                .IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            shared.context.IASetInputLayout(None);
            shared.context.VSSetShader(&shared.vs, None);
            shared.context.PSSetShader(&shared.ps, None);
            shared
                .context
                .PSSetConstantBuffers(0, Some(&[Some(shared.constants.clone())]));
            shared
                .context
                .PSSetShaderResources(0, Some(&[Some(sharp_srv), Some(soft_srv)]));
            shared
                .context
                .OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            shared.context.RSSetViewports(Some(&[viewport]));
            shared.context.Draw(4, 0);
            shared.context.OMSetRenderTargets(Some(&[]), None);
            shared.context.PSSetShaderResources(0, Some(&[None, None]));
            shared.context.CopySubresourceRegion(
                &target.staging,
                0,
                0,
                0,
                0,
                &target.texture,
                0,
                Some(&src_box),
            );
        }

        let pixels = self.read_staging(width, height)?;
        Some(GlassBitmap {
            x: bounds.x,
            y: bounds.y,
            width,
            height,
            pixels,
        })
    }

    fn ensure_sharp_srv(&mut self, generation: u64, buffer: &RgbBuffer) -> Option<()> {
        if self
            .sharp_cache
            .as_ref()
            .is_some_and(|slot| slot.key == generation)
        {
            return Some(());
        }
        upload_texture(
            &self.shared,
            &mut self.sharp_cache,
            &mut self.upload_scratch,
            generation,
            buffer,
        )
    }

    fn ensure_soft_srv(&mut self, key: SoftCacheKey, buffer: &RgbBuffer) -> Option<()> {
        if self.soft_cache.as_ref().is_some_and(|slot| slot.key == key) {
            return Some(());
        }
        upload_texture(
            &self.shared,
            &mut self.soft_cache,
            &mut self.upload_scratch,
            key,
            buffer,
        )
    }

    fn ensure_target(&mut self, width: u32, height: u32) -> Option<()> {
        let cap_w = bucket_dim(width);
        let cap_h = bucket_dim(height);
        if self.target.as_ref().is_some_and(|held| {
            held.width >= width
                && held.height >= height
                && held.width <= cap_w.saturating_mul(2)
                && held.height <= cap_h.saturating_mul(2)
        }) {
            return Some(());
        }
        let rt_desc = D3D11_TEXTURE2D_DESC {
            Width: cap_w,
            Height: cap_h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe {
            self.shared
                .device
                .CreateTexture2D(&rt_desc, None, Some(&mut texture))
                .ok()?;
        }
        let texture = texture?;

        let mut rtv = None;
        unsafe {
            self.shared
                .device
                .CreateRenderTargetView(&texture, None, Some(&mut rtv))
                .ok()?;
        }
        let rtv = rtv?;

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: cap_w,
            Height: cap_h,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging = None;
        unsafe {
            self.shared
                .device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging))
                .ok()?;
        }

        self.target = Some(RenderTarget {
            width: cap_w,
            height: cap_h,
            texture,
            rtv,
            staging: staging?,
        });
        Some(())
    }

    fn upload_constants(&self, data: &LiquidGlassConstants) -> Option<()> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.shared
                .context
                .Map(
                    &self.shared.constants,
                    0,
                    D3D11_MAP_WRITE_DISCARD,
                    0,
                    Some(&mut mapped),
                )
                .ok()?;
            std::ptr::copy_nonoverlapping(
                std::ptr::from_ref(data),
                mapped.pData.cast::<LiquidGlassConstants>(),
                1,
            );
            self.shared.context.Unmap(&self.shared.constants, 0);
        }
        Some(())
    }

    fn read_staging(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        let target = self.target.as_ref()?;
        let row_bytes = (width as usize).checked_mul(4)?;
        let total = row_bytes.checked_mul(height as usize)?;
        let mut pixels = Vec::with_capacity(total);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.shared
                .context
                .Map(&target.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .ok()?;
            let pitch = mapped.RowPitch as usize;
            let base = mapped.pData.cast::<u8>();
            if pitch == row_bytes {
                pixels.extend_from_slice(std::slice::from_raw_parts(base, total));
            } else {
                for row in 0..height as usize {
                    let src = std::slice::from_raw_parts(base.add(row * pitch), row_bytes);
                    pixels.extend_from_slice(src);
                }
            }
            self.shared.context.Unmap(&target.staging, 0);
        }
        Some(pixels)
    }
}

fn upload_texture<K>(
    shared: &SharedGpu,
    slot: &mut Option<TextureSlot<K>>,
    scratch: &mut Vec<u8>,
    key: K,
    buffer: &RgbBuffer,
) -> Option<()> {
    let width = u32::try_from(buffer.width()).ok()?;
    let height = u32::try_from(buffer.height()).ok()?;
    buffer.write_rgba_u8(scratch);
    let pitch = width.checked_mul(4)?;

    if let Some(held) = slot.as_mut()
        && held.width == width
        && held.height == height
    {
        unsafe {
            shared.context.UpdateSubresource(
                &held.texture,
                0,
                None,
                scratch.as_ptr().cast(),
                pitch,
                0,
            );
        }
        held.key = key;
        return Some(());
    }

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_DEFAULT,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
    };
    let subresource = D3D11_SUBRESOURCE_DATA {
        pSysMem: scratch.as_ptr().cast(),
        SysMemPitch: pitch,
        SysMemSlicePitch: 0,
    };
    let mut texture = None;
    unsafe {
        shared
            .device
            .CreateTexture2D(&desc, Some(&subresource), Some(&mut texture))
            .ok()?;
    }
    let texture = texture?;

    let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        ViewDimension: D3D11_SRV_DIMENSION_TEXTURE2D,
        Anonymous: windows::Win32::Graphics::Direct3D11::D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
            Texture2D: D3D11_TEX2D_SRV {
                MostDetailedMip: 0,
                MipLevels: 1,
            },
        },
    };
    let mut srv = None;
    unsafe {
        shared
            .device
            .CreateShaderResourceView(&texture, Some(&srv_desc), Some(&mut srv))
            .ok()?;
    }
    *slot = Some(TextureSlot {
        key,
        width,
        height,
        texture,
        srv: srv?,
    });
    Some(())
}

fn build_constants(
    bounds: &otto_core::Rect,
    shape: &GlassShape,
    params: &GlassParams,
    backdrop: &GlassBackdrop<'_>,
    light_dir: (f32, f32),
) -> LiquidGlassConstants {
    LiquidGlassConstants {
        bounds: [
            bounds.x as f32,
            bounds.y as f32,
            bounds.width as f32,
            bounds.height as f32,
        ],
        shape_rect: [shape.left, shape.top, shape.right, shape.bottom],
        shape_extra: [
            shape.radius,
            backdrop.soft_factor as f32,
            backdrop.origin.0,
            backdrop.origin.1,
        ],
        light_and_sharp: [
            light_dir.0,
            light_dir.1,
            backdrop.sharp.width() as f32,
            backdrop.sharp.height() as f32,
        ],
        soft_and_tint: [
            backdrop.soft.width() as f32,
            backdrop.soft.height() as f32,
            params.tint[0],
            params.tint[1],
        ],
        tint_and_grade: [
            params.tint[2],
            params.tint_amount,
            params.luma_floor,
            params.luma_ceiling,
        ],
        optics_a: [
            params.saturation,
            params.sheen_alpha,
            params.rim_base,
            params.rim_lit,
        ],
        optics_b: [
            params.rim_bounce,
            params.rim_width,
            params.inner_glow,
            params.inner_glow_depth,
        ],
        optics_c: [
            params.outline_alpha,
            params.shadow_strength,
            params.shadow_depth,
            params.lens_band,
        ],
        optics_d: [
            params.lens_shift,
            params.lens_mix,
            params.lens_tint_scale,
            params.dispersion,
        ],
    }
}

#[cfg(test)]
mod tests {
    use otto_core::{ResolvedTheme, compose_glass_lit, soften_backdrop};

    use super::*;

    struct ReleaseGpu;

    impl Drop for ReleaseGpu {
        fn drop(&mut self) {
            release_shared_gpu();
        }
    }

    fn synthetic_backdrop(width: usize, height: usize) -> RgbBuffer {
        let mut pixels = vec![0u8; width * height * 4];
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                let stripe = ((x / 12) + (y / 12)) % 2 == 0;
                if stripe {
                    pixels[idx] = 210;
                    pixels[idx + 1] = 140;
                    pixels[idx + 2] = 60;
                } else {
                    pixels[idx] = 35;
                    pixels[idx + 1] = 75;
                    pixels[idx + 2] = 165;
                }
                pixels[idx + 3] = 255;
            }
        }
        let w = u32::try_from(width).expect("synthetic width");
        let h = u32::try_from(height).expect("synthetic height");
        RgbBuffer::from_bgra(w, h, &pixels).expect("synthetic backdrop")
    }

    #[test]
    fn gpu_composer_matches_cpu_reference_within_three_levels() {
        let _release = ReleaseGpu;
        let Some(mut gpu) = GpuGlassComposer::new() else {
            return;
        };
        let params = GlassParams::for_theme(ResolvedTheme::Dark).scaled(96);
        let sharp = synthetic_backdrop(160, 80);
        let soft = soften_backdrop(&sharp, &params, 2, 3);
        let backdrop = GlassBackdrop {
            sharp: &sharp,
            soft: &soft,
            soft_factor: 2,
            origin: (0.0, 0.0),
        };
        let shape = GlassShape {
            left: 16.0,
            top: 12.0,
            right: 144.0,
            bottom: 68.0,
            radius: 18.0,
        };
        let light = (0.35_f32, -0.94_f32);
        let cpu = compose_glass_lit(&params, &shape, &backdrop, light);
        let gpu_bitmap = gpu
            .compose(
                1,
                SoftCacheKey {
                    generation: 1,
                    dpi: 96,
                    theme: ResolvedTheme::Dark,
                    style: otto_core::AppearanceStyle::LiquidGlass,
                    tint_key: params.tint_key(),
                },
                &params,
                &shape,
                &backdrop,
                light,
            )
            .expect("gpu compose should succeed");

        assert_eq!(gpu_bitmap.width, cpu.width);
        assert_eq!(gpu_bitmap.height, cpu.height);
        let max_diff = cpu
            .pixels
            .iter()
            .zip(&gpu_bitmap.pixels)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(max_diff <= 3, "max channel diff {max_diff} exceeded 3");
    }

    #[test]
    fn composers_share_one_device_and_reuse_backdrop_textures() {
        let _release = ReleaseGpu;
        let (Some(mut first), Some(second)) = (GpuGlassComposer::new(), GpuGlassComposer::new())
        else {
            return;
        };
        assert!(Rc::ptr_eq(&first.shared, &second.shared));

        let params = GlassParams::for_theme(ResolvedTheme::Dark).scaled(96);
        let sharp = synthetic_backdrop(160, 80);
        let soft = soften_backdrop(&sharp, &params, 2, 3);
        let backdrop = GlassBackdrop {
            sharp: &sharp,
            soft: &soft,
            soft_factor: 2,
            origin: (0.0, 0.0),
        };
        let shape = GlassShape {
            left: 16.0,
            top: 12.0,
            right: 144.0,
            bottom: 68.0,
            radius: 18.0,
        };
        let light = (0.35_f32, -0.94_f32);
        let key = |generation| SoftCacheKey {
            generation,
            dpi: 96,
            theme: ResolvedTheme::Dark,
            style: otto_core::AppearanceStyle::LiquidGlass,
            tint_key: params.tint_key(),
        };
        first
            .compose(1, key(1), &params, &shape, &backdrop, light)
            .expect("first generation");
        let texture = first
            .sharp_cache
            .as_ref()
            .expect("sharp slot")
            .texture
            .as_raw();
        let again = first
            .compose(2, key(2), &params, &shape, &backdrop, light)
            .expect("second generation");
        let reused = first.sharp_cache.as_ref().expect("sharp slot");
        assert_eq!(reused.key, 2);
        assert_eq!(reused.texture.as_raw(), texture);
        let cpu = compose_glass_lit(&params, &shape, &backdrop, light);
        let max_diff = cpu
            .pixels
            .iter()
            .zip(&again.pixels)
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0);
        assert!(max_diff <= 3, "max channel diff {max_diff} exceeded 3");
    }

    #[test]
    fn gpu_composer_reuses_bucketed_render_target_across_grows() {
        let _release = ReleaseGpu;
        let Some(mut gpu) = GpuGlassComposer::new() else {
            return;
        };
        let params = GlassParams::for_theme(ResolvedTheme::Dark).scaled(96);
        let sharp = synthetic_backdrop(200, 100);
        let soft = soften_backdrop(&sharp, &params, 2, 3);
        let backdrop = GlassBackdrop {
            sharp: &sharp,
            soft: &soft,
            soft_factor: 2,
            origin: (0.0, 0.0),
        };
        let light = (0.35_f32, -0.94_f32);
        let key = SoftCacheKey {
            generation: 1,
            dpi: 96,
            theme: ResolvedTheme::Dark,
            style: otto_core::AppearanceStyle::LiquidGlass,
            tint_key: params.tint_key(),
        };

        let shapes = [
            (
                130_u32,
                56_u32,
                GlassShape {
                    left: 0.0,
                    top: 0.0,
                    right: 130.0,
                    bottom: 56.0,
                    radius: 16.0,
                },
            ),
            (
                132_u32,
                58_u32,
                GlassShape {
                    left: 0.0,
                    top: 0.0,
                    right: 132.0,
                    bottom: 58.0,
                    radius: 16.0,
                },
            ),
            (
                140_u32,
                60_u32,
                GlassShape {
                    left: 0.0,
                    top: 0.0,
                    right: 140.0,
                    bottom: 60.0,
                    radius: 16.0,
                },
            ),
        ];

        let mut initial_ptr = None;
        for (req_w, req_h, shape) in shapes {
            let bounds = shape.pixel_bounds();
            let expected_w = u32::try_from(bounds.width).expect("bounds width");
            let expected_h = u32::try_from(bounds.height).expect("bounds height");
            assert!(expected_w >= req_w && expected_h >= req_h);

            let bitmap = gpu
                .compose(1, key, &params, &shape, &backdrop, light)
                .expect("gpu compose should succeed");
            assert_eq!(bitmap.width, expected_w);
            assert_eq!(bitmap.height, expected_h);
            assert_eq!(
                bitmap.pixels.len(),
                (expected_w as usize) * (expected_h as usize) * 4
            );

            let target = gpu.target.as_ref().expect("render target allocated");
            assert_eq!(target.width, 192);
            assert_eq!(target.height, 64);
            let tex_ptr = target.texture.as_raw();
            if let Some(first_ptr) = initial_ptr {
                assert_eq!(tex_ptr, first_ptr);
            } else {
                initial_ptr = Some(tex_ptr);
            }
        }
        gpu.clear_caches();
        assert!(gpu.sharp_cache.is_none());
        assert!(gpu.soft_cache.is_none());
        assert!(gpu.target.is_none());
        assert!(gpu.upload_scratch.is_empty());
    }
}
