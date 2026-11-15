cbuffer GenieParams : register(b0)
{
    float2 viewport_origin;
    float2 viewport_size;
    float2 window_size;
    float corner_radius;
    float opacity;
    float clip_edge_y;
    float clip_sink_y;
    float clip_below;
    float clip_strength;
};

Texture2D snapshot_tex : register(t0);
SamplerState linear_clamp : register(s0);

struct VSInput
{
    float2 pos : POSITION;
    float2 uv : TEXCOORD0;
    float strip_w : TEXCOORD1;
};

struct PSInput
{
    float4 pos : SV_Position;
    float3 uv_q : TEXCOORD0;
    float local_y : TEXCOORD1;
};

PSInput VSMain(VSInput input)
{
    PSInput output;
    float2 local_pos = input.pos - viewport_origin;
    float2 safe_size = max(viewport_size, float2(1.0, 1.0));
    float2 ndc;
    ndc.x = (local_pos.x / safe_size.x) * 2.0 - 1.0;
    ndc.y = 1.0 - (local_pos.y / safe_size.y) * 2.0;
    output.pos = float4(ndc, 0.0, 1.0);
    float safe_w = max(input.strip_w, 1.0);
    output.uv_q = float3(input.uv.x * safe_w, input.uv.y * safe_w, safe_w);
    output.local_y = local_pos.y;
    return output;
}

float4 PSMain(PSInput input) : SV_Target
{
    float2 uv = input.uv_q.xy / max(input.uv_q.z, 1e-4);

    float fw_x = max(fwidth(uv.x), 1e-5);
    float dist_x = min(uv.x, 1.0 - uv.x) / fw_x;
    float edge_aa = smoothstep(-0.75, 0.75, dist_x);

    float corner_aa = 1.0;
    float max_r = min(window_size.x, window_size.y) * 0.5;
    float r = clamp(corner_radius, 0.0, max_r);
    if (r > 0.5)
    {
        float2 p_px = clamp(uv, 0.0, 1.0) * window_size;
        float2 half_size = window_size * 0.5;
        float2 q = abs(p_px - half_size) - (half_size - r);
        if (q.x > 0.0 && q.y > 0.0)
        {
            float sdf = length(q) - r;
            float px_scale = max(length(float2(ddx(p_px.x), ddy(p_px.y))), 1.0);
            corner_aa = 1.0 - smoothstep(-0.75 * px_scale, 0.75 * px_scale, sdf);
        }
    }

    float clip_aa = 1.0;
    if (clip_strength > 0.001)
    {
        float depth = (input.local_y - clip_edge_y) * clip_below;
        float span = max(abs(clip_sink_y - clip_edge_y), 1.0);
        float clipped = 1.0 - smoothstep(0.0, span, depth);
        clip_aa = lerp(1.0, clipped, clip_strength);
    }

    float alpha = saturate(edge_aa * corner_aa * clip_aa * opacity);
    float4 color = snapshot_tex.Sample(linear_clamp, clamp(uv, 0.0, 1.0));
    return float4(color.rgb * alpha, color.a * alpha);
}
