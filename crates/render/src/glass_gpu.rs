use otto_core::{GlassBackdrop, GlassBitmap, GlassParams, GlassShape, RgbBuffer};
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D::D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP;
use windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
    D3D11_BUFFER_DESC, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAP_READ, D3D11_MAP_WRITE_DISCARD,
    D3D11_MAPPED_SUBRESOURCE, D3D11_RESOURCE_MISC_FLAG, D3D11_SDK_VERSION,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SUBRESOURCE_DATA, D3D11_TEX2D_SRV, D3D11_TEXTURE2D_DESC,
    D3D11_USAGE_DEFAULT, D3D11_USAGE_DYNAMIC, D3D11_USAGE_IMMUTABLE, D3D11_USAGE_STAGING,
    D3D11_VIEWPORT, D3D11CreateDevice, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext,
    ID3D11PixelShader, ID3D11RenderTargetView, ID3D11ShaderResourceView, ID3D11Texture2D,
    ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32G32B32A32_FLOAT, DXGI_SAMPLE_DESC,
};

const VS_BYTES: &[u8] = include_bytes!("shaders/liquid_glass_vs.dxbc");
const PS_BYTES: &[u8] = include_bytes!("shaders/liquid_glass_ps.dxbc");

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
}

struct RenderTarget {
    width: u32,
    height: u32,
    texture: ID3D11Texture2D,
    rtv: ID3D11RenderTargetView,
    staging: ID3D11Texture2D,
}

pub(crate) struct GpuGlassComposer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    constants: ID3D11Buffer,
    sharp_cache: Option<(u64, ID3D11ShaderResourceView)>,
    soft_cache: Option<(SoftCacheKey, ID3D11ShaderResourceView)>,
    target: Option<RenderTarget>,
}

impl GpuGlassComposer {
    pub(crate) fn new() -> Option<Self> {
        let mut device = None;
        let mut context = None;
        let levels = [D3D_FEATURE_LEVEL_11_0];
        let created = unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
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
        let vs = vs?;
        let ps = ps?;

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

        Some(Self {
            device,
            context,
            vs,
            ps,
            constants: constants?,
            sharp_cache: None,
            soft_cache: None,
            target: None,
        })
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

        let sharp_srv = self.sharp_cache.as_ref()?.1.clone();
        let soft_srv = self.soft_cache.as_ref()?.1.clone();
        let target = self.target.as_ref()?;

        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: width as f32,
            Height: height as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };

        unsafe {
            self.context
                .IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            self.context.IASetInputLayout(None);
            self.context.VSSetShader(&self.vs, None);
            self.context.PSSetShader(&self.ps, None);
            self.context
                .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
            self.context
                .PSSetShaderResources(0, Some(&[Some(sharp_srv), Some(soft_srv)]));
            self.context
                .OMSetRenderTargets(Some(&[Some(target.rtv.clone())]), None);
            self.context.RSSetViewports(Some(&[viewport]));
            self.context.Draw(4, 0);
            self.context.OMSetRenderTargets(Some(&[]), None);
            self.context.PSSetShaderResources(0, Some(&[None, None]));
            self.context.CopyResource(&target.staging, &target.texture);
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
            .is_some_and(|(cached, _)| *cached == generation)
        {
            return Some(());
        }
        let srv = create_rgba_f32_srv(&self.device, buffer)?;
        self.sharp_cache = Some((generation, srv));
        Some(())
    }

    fn ensure_soft_srv(&mut self, key: SoftCacheKey, buffer: &RgbBuffer) -> Option<()> {
        if self
            .soft_cache
            .as_ref()
            .is_some_and(|(cached, _)| *cached == key)
        {
            return Some(());
        }
        let srv = create_rgba_f32_srv(&self.device, buffer)?;
        self.soft_cache = Some((key, srv));
        Some(())
    }

    fn ensure_target(&mut self, width: u32, height: u32) -> Option<()> {
        if self
            .target
            .as_ref()
            .is_some_and(|held| held.width == width && held.height == height)
        {
            return Some(());
        }
        let rt_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
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
            self.device
                .CreateTexture2D(&rt_desc, None, Some(&mut texture))
                .ok()?;
        }
        let texture = texture?;

        let mut rtv = None;
        unsafe {
            self.device
                .CreateRenderTargetView(&texture, None, Some(&mut rtv))
                .ok()?;
        }
        let rtv = rtv?;

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
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
            self.device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging))
                .ok()?;
        }

        self.target = Some(RenderTarget {
            width,
            height,
            texture,
            rtv,
            staging: staging?,
        });
        Some(())
    }

    fn upload_constants(&self, data: &LiquidGlassConstants) -> Option<()> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(
                    &self.constants,
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
            self.context.Unmap(&self.constants, 0);
        }
        Some(())
    }

    fn read_staging(&self, width: u32, height: u32) -> Option<Vec<u8>> {
        let target = self.target.as_ref()?;
        let row_bytes = (width as usize).checked_mul(4)?;
        let total = row_bytes.checked_mul(height as usize)?;
        let mut pixels = vec![0u8; total];

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&target.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .ok()?;
            let pitch = mapped.RowPitch as usize;
            let base = mapped.pData.cast::<u8>();
            for row in 0..height as usize {
                let src = std::slice::from_raw_parts(base.add(row * pitch), row_bytes);
                let dst = &mut pixels[row * row_bytes..(row + 1) * row_bytes];
                dst.copy_from_slice(src);
            }
            self.context.Unmap(&target.staging, 0);
        }
        Some(pixels)
    }
}

fn create_rgba_f32_srv(
    device: &ID3D11Device,
    buffer: &RgbBuffer,
) -> Option<ID3D11ShaderResourceView> {
    let width = u32::try_from(buffer.width()).ok()?;
    let height = u32::try_from(buffer.height()).ok()?;
    let rgba = buffer.to_rgba_f32();
    let pitch = width.checked_mul(16)?;

    let desc = D3D11_TEXTURE2D_DESC {
        Width: width,
        Height: height,
        MipLevels: 1,
        ArraySize: 1,
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Usage: D3D11_USAGE_IMMUTABLE,
        BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
        CPUAccessFlags: 0,
        MiscFlags: D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
    };
    let subresource = D3D11_SUBRESOURCE_DATA {
        pSysMem: rgba.as_ptr().cast(),
        SysMemPitch: pitch,
        SysMemSlicePitch: 0,
    };
    let mut texture = None;
    unsafe {
        device
            .CreateTexture2D(&desc, Some(&subresource), Some(&mut texture))
            .ok()?;
    }
    let texture = texture?;

    let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
        Format: DXGI_FORMAT_R32G32B32A32_FLOAT,
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
        device
            .CreateShaderResourceView(&texture, Some(&srv_desc), Some(&mut srv))
            .ok()?;
    }
    srv
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
}
