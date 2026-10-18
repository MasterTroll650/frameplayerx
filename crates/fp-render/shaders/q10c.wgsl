// Copies one band of a Q10C plane, read through a TP10 texture (see
// src/q10c.rs), into a P010 plane.

struct Band {
    // Output row of the band's first row.
    row0: u32,
}
var<immediate> band: Band;

@group(0) @binding(0) var tp10: texture_2d<f32>;
@group(0) @binding(1) var luma_out: texture_storage_2d<r16unorm, write>;
@group(0) @binding(2) var chroma_out: texture_storage_2d<rg16unorm, write>;

// P010 keeps the 10 bits at the top of 16: v * 1023 * 64 / 65535.
const P010: f32 = 65472.0 / 65535.0;

@compute @workgroup_size(8, 8)
fn luma(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(tp10))) {
        return;
    }
    let y = textureLoad(tp10, vec2<i32>(id.xy), 0).r;
    textureStore(luma_out, vec2<u32>(id.x, band.row0 + id.y), vec4<f32>(y * P010, 0.0, 0.0, 1.0));
}

@compute @workgroup_size(8, 8)
fn chroma(@builtin(global_invocation_id) id: vec3<u32>) {
    if (any(id.xy >= textureDimensions(tp10))) {
        return;
    }
    let uv = textureLoad(tp10, vec2<i32>(id.xy), 0).rg;
    textureStore(chroma_out, vec2<u32>(id.x, band.row0 + id.y), vec4<f32>(uv * P010, 0.0, 1.0));
}
