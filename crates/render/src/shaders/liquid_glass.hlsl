cbuffer LiquidGlassParams : register(b0)
{
    float4 bounds;
    float4 shape_rect;
    float4 shape_extra;
    float4 light_and_sharp;
    float4 soft_and_tint;
    float4 tint_and_grade;
    float4 optics_a;
    float4 optics_b;
    float4 optics_c;
    float4 optics_d;
};

Texture2D<float4> sharp_tex : register(t0);
Texture2D<float4> soft_tex : register(t1);

struct PSInput
{
    float4 pos : SV_Position;
    float2 uv : TEXCOORD0;
};

PSInput VSMain(uint id : SV_VertexID)
{
    PSInput output;
    float2 uv = float2((id & 1u) ? 1.0 : 0.0, (id & 2u) ? 1.0 : 0.0);
    output.pos = float4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    output.uv = uv;
    return output;
}

float smooth_unit(float t)
{
    float c = saturate(t);
    return c * c * (3.0 - 2.0 * c);
}

float sd_rect(float x, float y)
{
    float cx = (shape_rect.x + shape_rect.z) * 0.5;
    float cy = (shape_rect.y + shape_rect.w) * 0.5;
    float rx = (shape_rect.z - shape_rect.x) * 0.5 - shape_extra.x;
    float ry = (shape_rect.w - shape_rect.y) * 0.5 - shape_extra.x;
    float ox = abs(x - cx) - rx;
    float oy = abs(y - cy) - ry;
    float px = max(ox, 0.0);
    float py = max(oy, 0.0);
    float outside = (px > 0.0 && py > 0.0) ? length(float2(px, py)) : (px + py);
    return outside + min(max(ox, oy), 0.0) - shape_extra.x;
}

float2 rect_normal(float x, float y)
{
    float gx = sd_rect(x + 1.0, y) - sd_rect(x - 1.0, y);
    float gy = sd_rect(x, y + 1.0) - sd_rect(x, y - 1.0);
    float len = length(float2(gx, gy));
    return (len < 1e-4) ? float2(0.0, 0.0) : float2(gx / len, gy / len);
}

float3 sample_hermite_sharp(float sx, float sy)
{
    int w = max((int)light_and_sharp.z, 1);
    int h = max((int)light_and_sharp.w, 1);
    float x = clamp(sx - 0.5, 0.0, (float)(w - 1));
    float y = clamp(sy - 0.5, 0.0, (float)(h - 1));
    int x0 = (int)floor(x);
    int y0 = (int)floor(y);
    int x1 = min(x0 + 1, w - 1);
    int y1 = min(y0 + 1, h - 1);
    float fx = smooth_unit(x - (float)x0);
    float fy = smooth_unit(y - (float)y0);
    float3 c00 = sharp_tex.Load(int3(x0, y0, 0)).rgb;
    float3 c10 = sharp_tex.Load(int3(x1, y0, 0)).rgb;
    float3 c01 = sharp_tex.Load(int3(x0, y1, 0)).rgb;
    float3 c11 = sharp_tex.Load(int3(x1, y1, 0)).rgb;
    return lerp(lerp(c00, c10, fx), lerp(c01, c11, fx), fy);
}

float3 sample_hermite_soft(float sx, float sy)
{
    int w = max((int)soft_and_tint.x, 1);
    int h = max((int)soft_and_tint.y, 1);
    float x = clamp(sx - 0.5, 0.0, (float)(w - 1));
    float y = clamp(sy - 0.5, 0.0, (float)(h - 1));
    int x0 = (int)floor(x);
    int y0 = (int)floor(y);
    int x1 = min(x0 + 1, w - 1);
    int y1 = min(y0 + 1, h - 1);
    float fx = smooth_unit(x - (float)x0);
    float fy = smooth_unit(y - (float)y0);
    float3 c00 = soft_tex.Load(int3(x0, y0, 0)).rgb;
    float3 c10 = soft_tex.Load(int3(x1, y0, 0)).rgb;
    float3 c01 = soft_tex.Load(int3(x0, y1, 0)).rgb;
    float3 c11 = soft_tex.Load(int3(x1, y1, 0)).rgb;
    return lerp(lerp(c00, c10, fx), lerp(c01, c11, fx), fy);
}

float3 grade_color(float3 rgb, float tint_scale)
{
    float luma = dot(rgb, float3(0.2126, 0.7152, 0.0722));
    float3 diff = abs(rgb - luma);
    float chroma = max(diff.r, max(diff.g, diff.b));
    float luma_floor = tint_and_grade.z;
    float luma_ceiling = tint_and_grade.w;
    float compressed_luma = (luma_ceiling < 0.999)
        ? (luma / (1.0 + luma / max(luma_ceiling, 1e-3)))
        : luma;
    float target_luma = luma_floor + (1.0 - luma_floor) * compressed_luma;
    float luma_scale = (luma > 1e-4) ? (target_luma / luma) : 1.0;
    float chroma_relief = 1.0 - 0.30 * smooth_unit(chroma * 2.0);
    float amount = saturate(tint_and_grade.y * tint_scale * chroma_relief);
    float3 tint = float3(soft_and_tint.z, soft_and_tint.w, tint_and_grade.x);
    float3 base = (luma > 1e-4)
        ? (target_luma + (rgb - luma) * optics_a.x * luma_scale)
        : float3(target_luma, target_luma, target_luma);
    return saturate(saturate(base) * (1.0 - amount) + tint * amount);
}

float4 PSMain(PSInput input) : SV_Target
{
    float x = bounds.x + input.uv.x * bounds.z;
    float y = bounds.y + input.uv.y * bounds.w;
    float dist = sd_rect(x, y);
    float coverage = saturate(0.5 - dist);
    if (coverage <= 0.0)
    {
        return float4(0.0, 0.0, 0.0, 0.0);
    }

    float depth = max(-dist, 0.0);
    float factor = max(shape_extra.y, 1.0);
    float local_x = x - shape_extra.z;
    float local_y = y - shape_extra.w;

    float lens_band = optics_c.w;
    float lens_mix = optics_d.y;
    float lens_raw = (lens_mix <= 0.0 || lens_band <= 0.0) ? 0.0 : saturate(1.0 - depth / lens_band);
    float edge = smooth_unit(lens_raw);

    float rim_width = optics_b.y;
    float shadow_depth = optics_c.z;
    float inner_glow_depth = optics_b.w;
    bool need_normal = (edge > 0.0) || (depth < rim_width + max(shadow_depth, inner_glow_depth));
    float2 normal = need_normal ? rect_normal(x, y) : float2(0.0, 0.0);

    float shift = (edge > 0.0) ? (optics_d.x * edge * edge) : 0.0;
    float2 sample_pos = float2(local_x, local_y) - normal * shift;

    float3 color = sample_hermite_soft(sample_pos.x / factor, sample_pos.y / factor);

    if (edge > 0.0)
    {
        float disp = optics_d.w * edge * edge;
        float3 refracted;
        if (disp > 1e-3)
        {
            refracted.r = sample_hermite_sharp(sample_pos.x - normal.x * disp, sample_pos.y - normal.y * disp).r;
            refracted.g = sample_hermite_sharp(sample_pos.x, sample_pos.y).g;
            refracted.b = sample_hermite_sharp(sample_pos.x + normal.x * disp, sample_pos.y + normal.y * disp).b;
        }
        else
        {
            refracted = sample_hermite_sharp(sample_pos.x, sample_pos.y);
        }
        float3 graded = grade_color(refracted, optics_d.z);
        color = lerp(color, graded, lens_mix * edge);
    }

    float span = max(shape_rect.w - shape_rect.y, 1.0);
    float upper = saturate(1.0 - (y - shape_rect.y) / span);
    color = lerp(color, float3(1.0, 1.0, 1.0), optics_a.y * upper * upper);

    float lit = dot(normal, light_and_sharp.xy);
    float inner_band = smooth_unit(saturate(1.0 - depth / max(inner_glow_depth, 1e-3)));
    float inner_sheen = saturate(inner_band * optics_b.z * (0.35 + 0.65 * max(lit, 0.0)));
    color = lerp(color, float3(1.0, 1.0, 1.0), inner_sheen);

    float shadow_reach = saturate((depth - rim_width) / max(shadow_depth, 1e-3));
    float shadow_t = 1.0 - shadow_reach;
    float shadow = (shadow_t * shadow_t) * max(-lit, 0.0) * optics_c.y;
    color *= (1.0 - shadow);

    float rim_band = smooth_unit(saturate(1.0 - depth / max(rim_width, 1e-3)));
    float primary = max(lit, 0.0);
    float bounce = max(-lit, 0.0);
    float rim = saturate(rim_band * (optics_a.z + optics_a.w * primary * primary + optics_b.x * bounce * bounce));
    color = lerp(color, float3(1.0, 1.0, 1.0), rim);

    float outline = smooth_unit(saturate(1.0 - depth)) * optics_c.x;
    color = lerp(color, float3(0.0, 0.0, 0.0), outline);

    return float4(color * coverage, coverage);
}
