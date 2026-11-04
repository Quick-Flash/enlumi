struct View { matrix: mat4x4<f32> }
struct Tile { position: vec4<f32>, dem: vec4<f32>, settings: vec4<f32>, unused: vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var imagery: texture_2d<f32>;
@group(1) @binding(1) var imagery_sampler: sampler;
@group(1) @binding(2) var heights: texture_2d<f32>;
@group(1) @binding(3) var<uniform> tile: Tile;
struct Output { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) shade: f32 }
fn elevation(uv:vec2<f32>)->f32 {
    let dimensions=vec2<i32>(textureDimensions(heights));
    let point=clamp(uv*vec2<f32>(dimensions)-vec2(0.5),vec2(0.),vec2<f32>(dimensions-vec2(1)));
    let a=vec2<i32>(floor(point));let b=min(a+vec2(1),dimensions-vec2(1));let f=fract(point);
    return mix(mix(textureLoad(heights,a,0).r,textureLoad(heights,vec2(b.x,a.y),0).r,f.x),
        mix(textureLoad(heights,vec2(a.x,b.y),0).r,textureLoad(heights,b,0).r,f.x),f.y);
}
@vertex fn vertex(@builtin(vertex_index) index:u32)->Output {
    let grid=48u;let side=grid+1u;let surface_count=side*side;
    var uv=vec2<f32>(0.);var skirt=0.;
    if index<surface_count {
        uv=vec2<f32>(f32(index%side),f32(index/side))/f32(grid);
    } else {
        let offset=index-surface_count;let edge=offset/(side*2u);
        let along=f32((offset/2u)%side)/f32(grid);skirt=f32(offset%2u)*tile.settings.z;
        switch edge {case 0u:{uv=vec2(along,0.);}case 1u:{uv=vec2(1.,along);}case 2u:{uv=vec2(along,1.);}default:{uv=vec2(0.,along);}}
    }
    let dem_uv=tile.dem.xy+uv*tile.dem.z;let step=1./vec2<f32>(textureDimensions(heights));
    let h=elevation(dem_uv);
    let dx=elevation(dem_uv+vec2(step.x,0.))-elevation(dem_uv-vec2(step.x,0.));
    let dz=elevation(dem_uv+vec2(0.,step.y))-elevation(dem_uv-vec2(0.,step.y));
    let normal=normalize(vec3(-dx*tile.settings.x,2.*tile.dem.w,-dz*tile.settings.x));
    let position=vec3(tile.position.x+uv.x*tile.position.z,(h-tile.position.w)*tile.settings.x-skirt,tile.position.y+uv.y*tile.position.z);
    var output:Output;output.position=view.matrix*vec4(position,1.);output.uv=tile.unused.xy+uv*tile.unused.z;
    output.shade=0.45+0.55*max(dot(normal,normalize(vec3(-0.4,0.85,-0.3))),0.);return output;
}
@fragment fn fragment(input:Output)->@location(0) vec4<f32> {
    return vec4(textureSample(imagery,imagery_sampler,input.uv).rgb*input.shade,1.);
}
