struct View { origin_scale: vec4<f32>, limits: vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;

@vertex fn vertex(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>,3>(vec2(-1.,-1.), vec2(3.,-1.), vec2(-1.,3.));
    return vec4(positions[index],0.,1.);
}

@fragment fn fragment(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let css = position.xy / view.origin_scale.w;
    if css.y < view.limits.x || css.y >= view.limits.y { return vec4(18./255.,24./255.,30./255.,1.); }
    let point = view.origin_scale.xy + css / view.origin_scale.z;
    let square = floor(point / 32.);
    let odd = (u32(square.x) + u32(square.y)) % 2u;
    let color = select(vec3(34.,58.,78.),vec3(49.,86.,102.),odd==1u);
    return vec4(color/255.,1.);
}
