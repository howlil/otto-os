use std::ffi::c_void;
use std::mem::size_of;

use otto_core::{GenieGeometry, PixelLimits, Rect as CoreRect, validate_bgra_buffer};
use windows::Win32::Foundation::{CloseHandle, E_INVALIDARG, HANDLE, HMODULE, HWND, WAIT_OBJECT_0};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_10_0,
    D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
    D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_SHADER_RESOURCE, D3D11_BIND_VERTEX_BUFFER,
    D3D11_BLEND_DESC, D3D11_BLEND_INV_SRC_ALPHA, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD,
    D3D11_BUFFER_DESC, D3D11_COLOR_WRITE_ENABLE_ALL, D3D11_COMPARISON_NEVER,
    D3D11_CPU_ACCESS_WRITE, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CULL_NONE, D3D11_FILL_SOLID,
    D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA,
    D3D11_MAP_WRITE_DISCARD, D3D11_MAPPED_SUBRESOURCE, D3D11_RASTERIZER_DESC,
    D3D11_RENDER_TARGET_BLEND_DESC, D3D11_SAMPLER_DESC, D3D11_SDK_VERSION,
    D3D11_SHADER_RESOURCE_VIEW_DESC, D3D11_SHADER_RESOURCE_VIEW_DESC_0, D3D11_SUBRESOURCE_DATA,
    D3D11_TEX2D_SRV, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DYNAMIC,
    D3D11_USAGE_IMMUTABLE, D3D11_VIEWPORT, D3D11CreateDevice, ID3D11BlendState, ID3D11Buffer,
    ID3D11Device, ID3D11DeviceContext, ID3D11InputLayout, ID3D11PixelShader, ID3D11RasterizerState,
    ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView, ID3D11Texture2D,
    ID3D11VertexShader,
};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_PREMULTIPLIED, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_R32_FLOAT,
    DXGI_FORMAT_R32G32_FLOAT, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_SCALING_STRETCH, DXGI_SWAP_CHAIN_DESC1,
    DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGISwapChain1,
    IDXGISwapChain2,
};
use windows::Win32::System::Threading::WaitForSingleObject;
use windows::core::{Error, Interface, Result, s};

const GENIE_VS_DXBC: &[u8] = include_bytes!("../shaders/genie_vs.dxbc");
const GENIE_PS_DXBC: &[u8] = include_bytes!("../shaders/genie_ps.dxbc");

const MAX_SURFACE_DIMENSION: i32 = 16_384;
const MAX_VERTICES: usize = 512;
const EDGE_AA_PAD_PX: f32 = 1.5;
const CLEAR_TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

#[repr(C)]
#[derive(Clone, Copy)]
struct GenieVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    strip_w: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct GenieConstants {
    viewport_origin: [f32; 2],
    viewport_size: [f32; 2],
    window_size: [f32; 2],
    corner_radius: f32,
    opacity: f32,
    clip_edge_y: f32,
    clip_sink_y: f32,
    clip_below: f32,
    clip_strength: f32,
}

struct WaitableHandle(HANDLE);

impl WaitableHandle {
    const fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    fn wait(&self, timeout_ms: u32) -> bool {
        if self.0.is_invalid() {
            return true;
        }
        unsafe { WaitForSingleObject(self.0, timeout_ms) == WAIT_OBJECT_0 }
    }

    fn close(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
            self.0 = HANDLE::default();
        }
    }
}

impl Drop for WaitableHandle {
    fn drop(&mut self) {
        self.close();
    }
}

pub struct GenieRenderer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    swap_chain: IDXGISwapChain2,
    waitable: WaitableHandle,
    rtv: Option<ID3D11RenderTargetView>,
    dcomp_device: IDCompositionDevice,
    dcomp_visual: IDCompositionVisual,
    dcomp_target: Option<IDCompositionTarget>,
    bound_hwnd: isize,
    vertex_shader: ID3D11VertexShader,
    pixel_shader: ID3D11PixelShader,
    input_layout: ID3D11InputLayout,
    vertex_buffer: ID3D11Buffer,
    constant_buffer: ID3D11Buffer,
    blend_state: ID3D11BlendState,
    rasterizer_state: ID3D11RasterizerState,
    sampler_state: ID3D11SamplerState,
    snapshot_srv: Option<ID3D11ShaderResourceView>,
    snapshot_size: Option<(u32, u32)>,
    canvas: CoreRect,
    vertices: Vec<GenieVertex>,
}

impl Drop for GenieRenderer {
    fn drop(&mut self) {
        self.snapshot_srv = None;
        self.rtv = None;
        self.dcomp_target = None;
        self.waitable.close();
    }
}

impl GenieRenderer {
    pub fn new(canvas: CoreRect) -> Result<Self> {
        validate_canvas(canvas)?;
        let (device, context) = create_d3d11_device()?;
        let dxgi_device: IDXGIDevice = device.cast()?;
        let adapter: IDXGIAdapter = unsafe { dxgi_device.GetAdapter()? };
        let factory: IDXGIFactory2 = unsafe { adapter.GetParent()? };

        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: canvas.width as u32,
            Height: canvas.height as u32,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
            Flags: DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT.0 as u32,
        };
        let swap_chain1: IDXGISwapChain1 =
            unsafe { factory.CreateSwapChainForComposition(&device, &desc, None)? };
        let swap_chain: IDXGISwapChain2 = swap_chain1.cast()?;
        unsafe {
            swap_chain.SetMaximumFrameLatency(1)?;
        }
        let waitable = WaitableHandle::new(unsafe { swap_chain.GetFrameLatencyWaitableObject() });
        let rtv = create_backbuffer_rtv(&device, &swap_chain)?;

        let dcomp_device: IDCompositionDevice = unsafe { DCompositionCreateDevice(&dxgi_device)? };
        let dcomp_visual = unsafe { dcomp_device.CreateVisual()? };
        unsafe {
            dcomp_visual.SetContent(&swap_chain)?;
        }

        let mut vertex_shader = None;
        unsafe {
            device.CreateVertexShader(GENIE_VS_DXBC, None, Some(&mut vertex_shader))?;
        }
        let vertex_shader = vertex_shader.ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;

        let mut pixel_shader = None;
        unsafe {
            device.CreatePixelShader(GENIE_PS_DXBC, None, Some(&mut pixel_shader))?;
        }
        let pixel_shader = pixel_shader.ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;

        let input_layout = create_input_layout(&device)?;
        let vertex_buffer = create_dynamic_buffer(
            &device,
            MAX_VERTICES * size_of::<GenieVertex>(),
            D3D11_BIND_VERTEX_BUFFER.0 as u32,
        )?;
        let constant_buffer = create_dynamic_buffer(
            &device,
            size_of::<GenieConstants>(),
            D3D11_BIND_CONSTANT_BUFFER.0 as u32,
        )?;
        let blend_state = create_premultiplied_blend_state(&device)?;
        let rasterizer_state = create_rasterizer_state(&device)?;
        let sampler_state = create_linear_clamp_sampler(&device)?;

        Ok(Self {
            device,
            context,
            swap_chain,
            waitable,
            rtv: Some(rtv),
            dcomp_device,
            dcomp_visual,
            dcomp_target: None,
            bound_hwnd: 0,
            vertex_shader,
            pixel_shader,
            input_layout,
            vertex_buffer,
            constant_buffer,
            blend_state,
            rasterizer_state,
            sampler_state,
            snapshot_srv: None,
            snapshot_size: None,
            canvas,
            vertices: Vec::with_capacity(MAX_VERTICES),
        })
    }

    pub fn bind_window(&mut self, hwnd: isize) -> Result<()> {
        if hwnd == 0 || self.bound_hwnd == hwnd {
            return Ok(());
        }
        self.dcomp_target = None;
        let target = unsafe {
            self.dcomp_device
                .CreateTargetForHwnd(HWND(hwnd as *mut c_void), true)?
        };
        unsafe {
            target.SetRoot(&self.dcomp_visual)?;
            self.dcomp_device.Commit()?;
        }
        self.dcomp_target = Some(target);
        self.bound_hwnd = hwnd;
        Ok(())
    }

    pub fn retarget_canvas(&mut self, canvas: CoreRect) -> Result<()> {
        validate_canvas(canvas)?;
        if canvas.width == self.canvas.width && canvas.height == self.canvas.height {
            self.canvas = canvas;
            return Ok(());
        }
        self.rtv = None;
        let resized = unsafe {
            self.swap_chain.ResizeBuffers(
                2,
                canvas.width as u32,
                canvas.height as u32,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT,
            )
        };
        if let Err(error) = resized {
            self.rtv = create_backbuffer_rtv(&self.device, &self.swap_chain).ok();
            return Err(error);
        }
        self.rtv = Some(create_backbuffer_rtv(&self.device, &self.swap_chain)?);
        self.canvas = canvas;
        Ok(())
    }

    pub fn set_snapshot(&mut self, width: u32, height: u32, pixels: &[u8]) -> Result<()> {
        self.clear_snapshot();
        let layout =
            validate_bgra_buffer(width, height, pixels.len(), PixelLimits::WINDOW_SNAPSHOT)
                .map_err(|_| Error::from_hresult(E_INVALIDARG))?;
        let tex_desc = D3D11_TEXTURE2D_DESC {
            Width: layout.width,
            Height: layout.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_IMMUTABLE,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let initial = D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr().cast(),
            SysMemPitch: layout.stride,
            SysMemSlicePitch: 0,
        };
        let mut texture = None;
        unsafe {
            self.device
                .CreateTexture2D(&tex_desc, Some(&initial), Some(&mut texture))?;
        }
        let texture = texture.ok_or_else(|| Error::from_hresult(E_INVALIDARG))?;
        let srv_desc = D3D11_SHADER_RESOURCE_VIEW_DESC {
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            ViewDimension: windows::Win32::Graphics::Direct3D::D3D11_SRV_DIMENSION_TEXTURE2D,
            Anonymous: D3D11_SHADER_RESOURCE_VIEW_DESC_0 {
                Texture2D: D3D11_TEX2D_SRV {
                    MostDetailedMip: 0,
                    MipLevels: 1,
                },
            },
        };
        let mut srv = None;
        unsafe {
            self.device
                .CreateShaderResourceView(&texture, Some(&srv_desc), Some(&mut srv))?;
        }
        self.snapshot_srv = srv;
        self.snapshot_size = Some((layout.width, layout.height));
        Ok(())
    }

    pub fn clear_snapshot(&mut self) {
        self.snapshot_srv = None;
        self.snapshot_size = None;
    }

    #[must_use]
    pub const fn canvas(&self) -> CoreRect {
        self.canvas
    }

    #[must_use]
    pub fn wait_for_next_frame(&self, timeout_ms: u32) -> bool {
        self.waitable.wait(timeout_ms)
    }

    pub fn present_frame(
        &mut self,
        geometry: &GenieGeometry,
        window_rect: CoreRect,
        corner_radius_px: f32,
        vsync: bool,
        commit_dcomp: bool,
    ) -> Result<()> {
        {
            let Some(rtv) = self.rtv.as_ref() else {
                return Err(Error::from_hresult(E_INVALIDARG));
            };
            unsafe {
                self.context.ClearRenderTargetView(rtv, &CLEAR_TRANSPARENT);
            }
        }
        if self.snapshot_srv.is_some() && !geometry.boundaries.is_empty() {
            self.build_vertices(geometry);
            let vertex_count = self.vertices.len().min(MAX_VERTICES);
            if vertex_count >= 4 {
                self.upload_vertices(vertex_count)?;
                self.upload_constants(geometry, window_rect, corner_radius_px)?;
                if let (Some(rtv), Some(srv)) = (self.rtv.as_ref(), self.snapshot_srv.as_ref()) {
                    self.draw_mesh(rtv, srv, vertex_count as u32);
                }
            }
        }
        let sync_interval = u32::from(vsync);
        unsafe {
            self.swap_chain
                .Present(sync_interval, DXGI_PRESENT(0))
                .ok()?;
            if commit_dcomp {
                self.dcomp_device.Commit()?;
            }
        }
        Ok(())
    }

    pub fn clear_and_present(&self) -> Result<()> {
        let Some(rtv) = self.rtv.as_ref() else {
            return Ok(());
        };
        unsafe {
            self.context.ClearRenderTargetView(rtv, &CLEAR_TRANSPARENT);
            self.swap_chain.Present(0, DXGI_PRESENT(0)).ok()?;
            self.dcomp_device.Commit()?;
        }
        Ok(())
    }

    fn build_vertices(&mut self, geometry: &GenieGeometry) {
        self.vertices.clear();
        for boundary in &geometry.boundaries {
            if self.vertices.len() + 2 > MAX_VERTICES {
                break;
            }
            let geom_w = (boundary.right - boundary.left).max(1.0);
            let pad_u = EDGE_AA_PAD_PX / geom_w;
            let padded_w = geom_w + 2.0 * EDGE_AA_PAD_PX;
            self.vertices.push(GenieVertex {
                pos: [boundary.left - EDGE_AA_PAD_PX, boundary.y],
                uv: [-pad_u, boundary.v],
                strip_w: padded_w,
            });
            self.vertices.push(GenieVertex {
                pos: [boundary.right + EDGE_AA_PAD_PX, boundary.y],
                uv: [1.0 + pad_u, boundary.v],
                strip_w: padded_w,
            });
        }
    }

    fn upload_vertices(&self, count: usize) -> Result<()> {
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context.Map(
                &self.vertex_buffer,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped),
            )?;
            std::ptr::copy_nonoverlapping(
                self.vertices.as_ptr(),
                mapped.pData.cast::<GenieVertex>(),
                count,
            );
            self.context.Unmap(&self.vertex_buffer, 0);
        }
        Ok(())
    }

    fn upload_constants(
        &self,
        geometry: &GenieGeometry,
        window_rect: CoreRect,
        corner_radius_px: f32,
    ) -> Result<()> {
        let constants = GenieConstants {
            viewport_origin: [self.canvas.x as f32, self.canvas.y as f32],
            viewport_size: [
                self.canvas.width.max(1) as f32,
                self.canvas.height.max(1) as f32,
            ],
            window_size: [
                window_rect.width.max(1) as f32,
                window_rect.height.max(1) as f32,
            ],
            corner_radius: corner_radius_px.max(0.0),
            opacity: geometry.opacity.clamp(0.0, 1.0),
            clip_edge_y: geometry.clip.edge_y - self.canvas.y as f32,
            clip_sink_y: geometry.clip.sink_y - self.canvas.y as f32,
            clip_below: if geometry.clip.clip_below { 1.0 } else { -1.0 },
            clip_strength: geometry.clip.strength.clamp(0.0, 1.0),
        };
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context.Map(
                &self.constant_buffer,
                0,
                D3D11_MAP_WRITE_DISCARD,
                0,
                Some(&mut mapped),
            )?;
            std::ptr::copy_nonoverlapping(
                (&raw const constants).cast::<u8>(),
                mapped.pData.cast::<u8>(),
                size_of::<GenieConstants>(),
            );
            self.context.Unmap(&self.constant_buffer, 0);
        }
        Ok(())
    }

    fn draw_mesh(
        &self,
        rtv: &ID3D11RenderTargetView,
        srv: &ID3D11ShaderResourceView,
        vertex_count: u32,
    ) {
        let viewport = D3D11_VIEWPORT {
            TopLeftX: 0.0,
            TopLeftY: 0.0,
            Width: self.canvas.width.max(1) as f32,
            Height: self.canvas.height.max(1) as f32,
            MinDepth: 0.0,
            MaxDepth: 1.0,
        };
        let stride = size_of::<GenieVertex>() as u32;
        let offset = 0u32;
        let rtvs = [Some(rtv.clone())];
        let vbs = [Some(self.vertex_buffer.clone())];
        let cbs = [Some(self.constant_buffer.clone())];
        let srvs = [Some(srv.clone())];
        let samplers = [Some(self.sampler_state.clone())];

        unsafe {
            self.context.OMSetRenderTargets(Some(&rtvs), None);
            self.context
                .OMSetBlendState(&self.blend_state, None, u32::MAX);
            self.context.RSSetState(&self.rasterizer_state);
            self.context.RSSetViewports(Some(&[viewport]));
            self.context
                .IASetPrimitiveTopology(D3D11_PRIMITIVE_TOPOLOGY_TRIANGLESTRIP);
            self.context.IASetInputLayout(&self.input_layout);
            self.context
                .IASetVertexBuffers(0, 1, Some(vbs.as_ptr()), Some(&stride), Some(&offset));
            self.context.VSSetShader(&self.vertex_shader, None);
            self.context.VSSetConstantBuffers(0, Some(&cbs));
            self.context.PSSetShader(&self.pixel_shader, None);
            self.context.PSSetConstantBuffers(0, Some(&cbs));
            self.context.PSSetShaderResources(0, Some(&srvs));
            self.context.PSSetSamplers(0, Some(&samplers));
            self.context.Draw(vertex_count, 0);

            let empty_srvs: [Option<ID3D11ShaderResourceView>; 1] = [None];
            self.context.PSSetShaderResources(0, Some(&empty_srvs));
            self.context.OMSetRenderTargets(None, None);
        }
    }
}

#[must_use]
pub fn build_fallback_card_snapshot(
    window_width: i32,
    window_height: i32,
    icon_rgba: Option<(u32, u32, &[u8])>,
    dark_theme: bool,
) -> Option<(u32, u32, Vec<u8>)> {
    if window_width <= 0 || window_height <= 0 {
        return None;
    }
    let scale = (480.0 / (window_width.max(window_height) as f32)).min(1.0);
    let width = ((window_width as f32 * scale).round() as u32).clamp(96, 480);
    let height = ((window_height as f32 * scale).round() as u32).clamp(64, 480);
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    let body: [u8; 4] = if dark_theme {
        [44, 42, 42, 245]
    } else {
        [244, 244, 246, 245]
    };
    let titlebar: [u8; 4] = if dark_theme {
        [58, 55, 55, 250]
    } else {
        [232, 232, 236, 250]
    };
    let border: [u8; 4] = if dark_theme {
        [92, 90, 90, 255]
    } else {
        [204, 204, 208, 255]
    };
    let title_h = (height / 8).clamp(18, 28);

    for y in 0..height {
        for x in 0..width {
            let edge = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
            let src = if edge {
                border
            } else if y < title_h {
                titlebar
            } else {
                body
            };
            let idx = ((y * width + x) * 4) as usize;
            let a = u16::from(src[3]);
            pixels[idx] = ((u16::from(src[0]) * a + 127) / 255) as u8;
            pixels[idx + 1] = ((u16::from(src[1]) * a + 127) / 255) as u8;
            pixels[idx + 2] = ((u16::from(src[2]) * a + 127) / 255) as u8;
            pixels[idx + 3] = src[3];
        }
    }

    let dot_r = (title_h as f32 * 0.19).clamp(3.0, 5.5);
    let dot_y = title_h as f32 * 0.5;
    let dots: [(f32, [u8; 3]); 3] = [
        (title_h as f32 * 0.65, [90, 95, 255]),
        (title_h as f32 * 1.35, [46, 189, 254]),
        (title_h as f32 * 2.05, [64, 200, 40]),
    ];
    for (dot_x, bgr) in dots {
        draw_circle(&mut pixels, width, height, dot_x, dot_y, dot_r, bgr);
    }

    if let Some((iw, ih, ibuf)) = icon_rgba
        && iw > 0
        && ih > 0
        && ibuf.len() >= (iw * ih * 4) as usize
    {
        let target_side = (width.min(height) / 3).clamp(32, 96);
        let ox = (width - target_side) / 2;
        let oy = title_h + ((height - title_h).saturating_sub(target_side)) / 2;
        blit_scaled_premul(
            &mut pixels,
            width,
            height,
            ox,
            oy,
            target_side,
            iw,
            ih,
            ibuf,
        );
    }

    Some((width, height, pixels))
}

fn draw_circle(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    bgr: [u8; 3],
) {
    let min_x = (cx - radius - 1.0).floor().max(0.0) as u32;
    let max_x = (cx + radius + 1.0).ceil().min((width - 1) as f32) as u32;
    let min_y = (cy - radius - 1.0).floor().max(0.0) as u32;
    let max_y = (cy + radius + 1.0).ceil().min((height - 1) as f32) as u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let cov = (radius + 0.5 - dist).clamp(0.0, 1.0);
            if cov > 0.0 {
                let idx = ((y * width + x) * 4) as usize;
                let inv = 1.0 - cov;
                for c in 0..3 {
                    pixels[idx + c] =
                        (f32::from(pixels[idx + c]) * inv + f32::from(bgr[c]) * cov).round() as u8;
                }
                pixels[idx + 3] = (f32::from(pixels[idx + 3]) * inv + 255.0 * cov)
                    .round()
                    .min(255.0) as u8;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_scaled_premul(
    dst: &mut [u8],
    dst_w: u32,
    dst_h: u32,
    ox: u32,
    oy: u32,
    side: u32,
    src_w: u32,
    src_h: u32,
    src: &[u8],
) {
    for dy in 0..side {
        let py = oy + dy;
        if py >= dst_h {
            break;
        }
        let sy = ((u64::from(dy) * u64::from(src_h)) / u64::from(side.max(1)))
            .min(u64::from(src_h - 1)) as u32;
        for dx in 0..side {
            let px = ox + dx;
            if px >= dst_w {
                break;
            }
            let sx = ((u64::from(dx) * u64::from(src_w)) / u64::from(side.max(1)))
                .min(u64::from(src_w - 1)) as u32;
            let s_idx = ((sy * src_w + sx) * 4) as usize;
            let d_idx = ((py * dst_w + px) * 4) as usize;
            let sa = f32::from(src[s_idx + 3]) / 255.0;
            let inv = 1.0 - sa;
            for c in 0..3 {
                dst[d_idx + c] =
                    (f32::from(dst[d_idx + c]) * inv + f32::from(src[s_idx + c])).min(255.0) as u8;
            }
            dst[d_idx + 3] =
                (f32::from(dst[d_idx + 3]) * inv + f32::from(src[s_idx + 3])).min(255.0) as u8;
        }
    }
}

fn validate_canvas(canvas: CoreRect) -> Result<()> {
    if canvas.width <= 0
        || canvas.height <= 0
        || canvas.width > MAX_SURFACE_DIMENSION
        || canvas.height > MAX_SURFACE_DIMENSION
    {
        return Err(Error::from_hresult(E_INVALIDARG));
    }
    Ok(())
}

fn create_d3d11_device() -> Result<(ID3D11Device, ID3D11DeviceContext)> {
    let levels = [
        D3D_FEATURE_LEVEL_11_1,
        D3D_FEATURE_LEVEL_11_0,
        D3D_FEATURE_LEVEL_10_1,
        D3D_FEATURE_LEVEL_10_0,
    ];
    for driver in [D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP] {
        let mut device = None;
        let mut context = None;
        let mut feature = D3D_FEATURE_LEVEL(0);
        let hr = unsafe {
            D3D11CreateDevice(
                None,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature),
                Some(&mut context),
            )
        };
        if hr.is_ok()
            && let (Some(dev), Some(ctx)) = (device, context)
        {
            return Ok((dev, ctx));
        }
    }
    Err(Error::from_hresult(E_INVALIDARG))
}

fn create_backbuffer_rtv(
    device: &ID3D11Device,
    swap_chain: &IDXGISwapChain2,
) -> Result<ID3D11RenderTargetView> {
    let backbuffer: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0)? };
    let mut rtv = None;
    unsafe {
        device.CreateRenderTargetView(&backbuffer, None, Some(&mut rtv))?;
    }
    rtv.ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

fn create_input_layout(device: &ID3D11Device) -> Result<ID3D11InputLayout> {
    let elements = [
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("POSITION"),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 0,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("TEXCOORD"),
            SemanticIndex: 0,
            Format: DXGI_FORMAT_R32G32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 8,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
        D3D11_INPUT_ELEMENT_DESC {
            SemanticName: s!("TEXCOORD"),
            SemanticIndex: 1,
            Format: DXGI_FORMAT_R32_FLOAT,
            InputSlot: 0,
            AlignedByteOffset: 16,
            InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
            InstanceDataStepRate: 0,
        },
    ];
    let mut layout = None;
    unsafe {
        device.CreateInputLayout(&elements, GENIE_VS_DXBC, Some(&mut layout))?;
    }
    layout.ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

fn create_dynamic_buffer(
    device: &ID3D11Device,
    byte_width: usize,
    bind_flags: u32,
) -> Result<ID3D11Buffer> {
    let aligned = byte_width.div_ceil(16) * 16;
    let desc = D3D11_BUFFER_DESC {
        ByteWidth: aligned as u32,
        Usage: D3D11_USAGE_DYNAMIC,
        BindFlags: bind_flags,
        CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
        MiscFlags: 0,
        StructureByteStride: 0,
    };
    let mut buffer = None;
    unsafe {
        device.CreateBuffer(&desc, None, Some(&mut buffer))?;
    }
    buffer.ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

fn create_premultiplied_blend_state(device: &ID3D11Device) -> Result<ID3D11BlendState> {
    let rt_blend = D3D11_RENDER_TARGET_BLEND_DESC {
        BlendEnable: true.into(),
        SrcBlend: D3D11_BLEND_ONE,
        DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOp: D3D11_BLEND_OP_ADD,
        SrcBlendAlpha: D3D11_BLEND_ONE,
        DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
        BlendOpAlpha: D3D11_BLEND_OP_ADD,
        RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8,
    };
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0] = rt_blend;
    let mut state = None;
    unsafe {
        device.CreateBlendState(&desc, Some(&mut state))?;
    }
    state.ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

fn create_rasterizer_state(device: &ID3D11Device) -> Result<ID3D11RasterizerState> {
    let desc = D3D11_RASTERIZER_DESC {
        FillMode: D3D11_FILL_SOLID,
        CullMode: D3D11_CULL_NONE,
        FrontCounterClockwise: false.into(),
        DepthBias: 0,
        DepthBiasClamp: 0.0,
        SlopeScaledDepthBias: 0.0,
        DepthClipEnable: false.into(),
        ScissorEnable: false.into(),
        MultisampleEnable: false.into(),
        AntialiasedLineEnable: false.into(),
    };
    let mut state = None;
    unsafe {
        device.CreateRasterizerState(&desc, Some(&mut state))?;
    }
    state.ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

fn create_linear_clamp_sampler(device: &ID3D11Device) -> Result<ID3D11SamplerState> {
    let desc = D3D11_SAMPLER_DESC {
        Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
        AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
        AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
        MipLODBias: 0.0,
        MaxAnisotropy: 1,
        ComparisonFunc: D3D11_COMPARISON_NEVER,
        BorderColor: [0.0; 4],
        MinLOD: 0.0,
        MaxLOD: f32::MAX,
    };
    let mut sampler = None;
    unsafe {
        device.CreateSamplerState(&desc, Some(&mut sampler))?;
    }
    sampler.ok_or_else(|| Error::from_hresult(E_INVALIDARG))
}

#[cfg(test)]
mod tests {
    use super::*;
    use otto_core::{GenieDirection, compute_genie_geometry};

    #[test]
    fn gpu_genie_renderer_draws_and_retargets_without_error() {
        let canvas = CoreRect {
            x: 0,
            y: 0,
            width: 640,
            height: 480,
        };
        let mut renderer = GenieRenderer::new(canvas).expect("D3D11/WARP renderer");
        let pixels = vec![180u8; 160 * 120 * 4];
        renderer
            .set_snapshot(160, 120, &pixels)
            .expect("snapshot upload");
        let window = CoreRect {
            x: 60,
            y: 40,
            width: 300,
            height: 220,
        };
        let dock = CoreRect {
            x: 280,
            y: 400,
            width: 48,
            height: 48,
        };
        let geometry = compute_genie_geometry(window, dock, 0.5, GenieDirection::Minimize);
        renderer
            .present_frame(&geometry, window, 12.0, false, false)
            .expect("frame draw");
        renderer
            .retarget_canvas(CoreRect {
                x: 10,
                y: 20,
                width: 800,
                height: 600,
            })
            .expect("retarget resize");
        renderer
            .present_frame(&geometry, window, 12.0, false, false)
            .expect("frame draw after resize");
    }

    #[test]
    fn fallback_card_snapshot_renders_valid_premultiplied_buffer() {
        let icon = vec![255u8; 32 * 32 * 4];
        let (w, h, buf) =
            build_fallback_card_snapshot(1200, 800, Some((32, 32, &icon)), true).unwrap();
        assert!(w <= 480 && h <= 480);
        assert_eq!(buf.len(), (w * h * 4) as usize);
        assert!(buf.iter().any(|&v| v > 0));
    }

    #[test]
    fn waitable_handle_drop_closes_duplicated_handle() {
        use windows::Win32::Foundation::{
            DUPLICATE_SAME_ACCESS, DuplicateHandle, GetHandleInformation,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;

        let process = unsafe { GetCurrentProcess() };
        let mut dup = HANDLE::default();
        unsafe {
            DuplicateHandle(
                process,
                process,
                process,
                &raw mut dup,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        }
        .expect("duplicate current process handle");
        let mut flags = 0u32;
        assert!(unsafe { GetHandleInformation(dup, &raw mut flags) }.is_ok());
        {
            let waitable = WaitableHandle::new(dup);
            assert!(!waitable.wait(0));
        }
        assert!(unsafe { GetHandleInformation(dup, &raw mut flags) }.is_err());
    }
}
