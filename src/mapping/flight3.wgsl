struct View { matrix: mat4x4<f32> }
// offset: translation (xyz) and horizontal scale (w) from the camera the
// vertices were built for to the current one.
struct Viewport { size:vec4<f32>, offset:vec4<f32> }
@group(0) @binding(0) var<uniform> view: View;
@group(1) @binding(0) var<uniform> viewport: Viewport;
struct Output {
    @builtin(position) position:vec4<f32>,
    @location(0) color:vec4<f32>,
    @location(1) @interpolate(linear) distance:vec2<f32>,
    @location(2) @interpolate(flat) radius:f32,
    @location(3) @interpolate(flat) span:f32,
    @location(4) @interpolate(flat) shape:f32,
    @location(5) @interpolate(flat) pattern:vec2<f32>,
}
@vertex fn vertex(@builtin(vertex_index) vertex:u32,@location(0) start:vec3<f32>,@location(1) end:vec3<f32>,@location(2) stroke:vec4<f32>,@location(3) stroke_width:f32,@location(4) point:f32,@location(5) pattern:vec2<f32>,@location(6) stroke_end:vec4<f32>)->Output {
    // Ground strokes are stored once as their halo; vertices 6..12 draw the
    // dark core from the same instance in a second, later draw call.
    let index=vertex%6u;let core=vertex>=6u && point< -0.5;
    var color=stroke;var end_color=stroke_end;var width=stroke_width;
    if core {color=vec4(0.,0.,0.,220./255.);end_color=color;width=1.5;}
    let o=viewport.offset;
    let p0=vec3(start.x*o.w,start.y,start.z*o.w)+o.xyz;let p1=vec3(end.x*o.w,end.y,end.z*o.w)+o.xyz;
    var a=view.matrix*vec4(p0,1.);var b=view.matrix*vec4(p1,1.);
    let da=a.w-a.z;let db=b.w-b.z;
    var output:Output;output.color=color;output.radius=max(width*0.5,0.);
    output.shape=point;output.span=0.;output.distance=vec2(0.);output.position=vec4(2.,2.,0.,1.);
    output.pattern=pattern;
    if (da<=0. && db<=0.) || width<=0. {return output;}
    // Clip crossing segments to the near plane before dividing by W.
    if da<0. {a=mix(a,b,da/(da-db));} else if db<0. {b=mix(a,b,da/(da-db));}
    if a.w<=0. || b.w<=0. {return output;}
    let corners=array<vec2<f32>,6>(vec2(0.,-1.),vec2(0.,1.),vec2(1.,-1.),vec2(1.,-1.),vec2(0.,1.),vec2(1.,1.));
    let corner=corners[index];let delta=(b.xy/b.w-a.xy/a.w)*viewport.size.xy*0.5;
    output.color=mix(color,end_color,corner.x);
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
// Distance to nearest repeated ink interval; round caps match solid strokes.
fn ink_distance(x:f32,period:f32,start:f32,end:f32)->f32 {
    let center=(start+end)*0.5;
    let wrapped=x-center-period*floor((x-center)/period+0.5);
    return max(abs(wrapped)-(end-start)*0.5,0.);
}
struct Fragment { @location(0) color:vec4<f32>, @builtin(frag_depth) depth:f32 }
@fragment fn fragment(input:Output)->Fragment {
    let depth_slope=fwidth(input.position.z);
    let p=abs(input.distance);
    var along=max(max(-input.distance.x,input.distance.x-input.span),0.);
    if input.pattern.y>0.5 {
        let unit=max(input.radius*2.,0.5);
        let x=(clamp(input.distance.x,0.,input.span)+input.pattern.x)/unit;
        var gap=ink_distance(x,9.,0.,6.);
        if input.pattern.y>1.5 && input.pattern.y<2.5 {gap=ink_distance(x,3.05,0.,0.05);}
        if input.pattern.y>2.5 {gap=min(ink_distance(x,12.05,0.,6.),ink_distance(x,12.05,9.,9.05));}
        along=max(along,gap*unit);
    }
    var distance=length(vec2(along,input.distance.y));
    if input.shape>1.5 && input.shape<2.5 {distance=max(p.x,p.y);}
    if input.shape>2.5 && input.shape<3.5 {
        distance=max(p.x*0.8660254+input.distance.y*0.5,-input.distance.y)*2.;
    }
    if input.shape>3.5 && input.shape<4.5 {distance=p.x+p.y;}
    if input.shape>4.5 && input.shape<5.5 {distance=max(max(p.x,p.y),min(p.x,p.y)*6.);}
    let coverage=1.-smoothstep(max(input.radius-0.5,0.),input.radius+0.5,distance);
    if coverage<=0. {discard;}
    var color=input.color.rgb;
    if input.shape>5.5 {
        let border=smoothstep(input.radius-2.,input.radius-1.,distance);
        color=mix(color,vec3(1.),border);
    }
    var output:Fragment;
    output.color=vec4(color,input.color.a*coverage);
    output.depth=input.position.z;
    // Ground annotations expand in screen space; bias depth across their width
    // so adjacent uphill terrain cannot shave off stroke edges and joints.
    if input.shape<0. {
        output.depth=min(1.,output.depth+depth_slope*(input.radius+1.)+max(0.0000001,output.depth*0.002));
    }
    return output;
}
