struct View { matrix: mat4x4<f32> }
struct Viewport { size:vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<uniform> viewport: Viewport;
struct Output {
    @builtin(position) position:vec4<f32>,
    @location(0) color:vec4<f32>,
    @location(1) @interpolate(linear) distance:vec2<f32>,
    @location(2) @interpolate(flat) radius:f32,
    @location(3) @interpolate(flat) span:f32,
    @location(4) @interpolate(flat) shape:f32,
}
@vertex fn vertex(@builtin(vertex_index) index:u32,@location(0) start:vec3<f32>,@location(1) end:vec3<f32>,@location(2) color:vec4<f32>,@location(3) width:f32,@location(4) point:f32)->Output {
    var a=view.matrix*vec4(start,1.);var b=view.matrix*vec4(end,1.);
    let da=a.w-a.z;let db=b.w-b.z;
    var output:Output;output.color=color;output.radius=max(width*0.5,0.);
    output.shape=point;output.span=0.;output.distance=vec2(0.);output.position=vec4(2.,2.,0.,1.);
    if (da<=0. && db<=0.) || width<=0. {return output;}
    // Clip crossing segments to the near plane before dividing by W.
    if da<0. {a=mix(a,b,da/(da-db));} else if db<0. {b=mix(a,b,da/(da-db));}
    if a.w<=0. || b.w<=0. {return output;}
    let corners=array<vec2<f32>,6>(vec2(0.,-1.),vec2(0.,1.),vec2(1.,-1.),vec2(1.,-1.),vec2(0.,1.),vec2(1.,1.));
    let corner=corners[index];let delta=(b.xy/b.w-a.xy/a.w)*viewport.size.xy*0.5;
    let length=length(delta);let extent=output.radius+1.;
    var offset=vec2(0.);
    if point>0.5 {
        // Marker shape and radius stay in screen pixels.
        offset=vec2(corner.x*2.-1.,corner.y)*extent;
        output.distance=offset;output.position=a;
    } else {
        if length<0.00001 {return output;}
        let tangent=delta/length;let normal=vec2(-tangent.y,tangent.x);
        let cap=(corner.x*2.-1.)*extent;
        offset=normal*corner.y*extent+tangent*cap;
        output.span=length;
        output.distance=vec2(corner.x*length+cap,corner.y*extent);
        output.position=mix(a,b,corner.x);
    }
    output.position=vec4(output.position.xy+offset*2./viewport.size.xy*output.position.w,output.position.zw);
    return output;
}
@fragment fn fragment(input:Output)->@location(0) vec4<f32>{
    let p=abs(input.distance);
    var distance=length(vec2(max(max(-input.distance.x,input.distance.x-input.span),0.),input.distance.y));
    if input.shape>1.5 && input.shape<2.5 {distance=max(p.x,p.y);}
    if input.shape>2.5 && input.shape<3.5 {
        distance=max(p.x*0.8660254+input.distance.y*0.5,-input.distance.y)*2.;
    }
    if input.shape>3.5 && input.shape<4.5 {distance=p.x+p.y;}
    if input.shape>4.5 {distance=max(max(p.x,p.y),min(p.x,p.y)*6.);}
    let coverage=1.-smoothstep(max(input.radius-0.5,0.),input.radius+0.5,distance);
    if coverage<=0. {discard;}
    return vec4(input.color.rgb,input.color.a*coverage);
}
