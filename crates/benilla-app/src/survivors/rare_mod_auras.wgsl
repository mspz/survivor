// Paladin ground textures remain shared and untouched. Only this cosmetic material
// samples them through a different animated motif for each of the 34 rare mods.
#import bevy_pbr::forward_io::VertexOutput
struct AuraUniform { color: vec4<f32>, accent: vec4<f32>, animation: vec4<f32> };
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> aura: AuraUniform;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var paladin: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var paladin_sampler: sampler;
const TAU: f32 = 6.28318530718;
const PI: f32 = 3.14159265359;
fn turn(p: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle); let s = sin(angle);
    return vec2<f32>(c*p.x-s*p.y,s*p.x+c*p.y);
}
fn line(d: f32, width: f32) -> f32 {
    return 1.0-smoothstep(width,width+clamp(fwidth(d)*1.2,0.003,0.012),abs(d));
}
fn ring(p: vec2<f32>, radius: f32, width: f32) -> f32 { return line(length(p)-radius,width); }
fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, width: f32) -> f32 {
    let ab = b-a; let q = p-a;
    return line(length(q-ab*clamp(dot(q,ab)/max(dot(ab,ab),0.0001),0.0,1.0)),width);
}
fn polygon(p: vec2<f32>, sides: f32, radius: f32, width: f32) -> f32 {
    let angle = atan2(p.y,p.x)+PI;
    let step_angle = TAU/sides;
    let edge = cos(floor(0.5+angle/step_angle)*step_angle-angle)*length(p);
    return line(edge-radius,width);
}
fn cross(p: vec2<f32>, extent: f32, width: f32) -> f32 {
    return max(segment(p,vec2<f32>(-extent,0.0),vec2<f32>(extent,0.0),width),
        segment(p,vec2<f32>(0.0,-extent),vec2<f32>(0.0,extent),width));
}
fn drop(p: vec2<f32>, scale: f32) -> f32 {
    let q = p/scale;
    let body = length(q-vec2<f32>(0.0,-0.12))-0.2;
    let tip = max(abs(q.x)*0.8+q.y*0.45-0.18,-q.y-0.1);
    return line(min(body,tip)*scale,0.014);
}
fn shield(p: vec2<f32>, scale: f32) -> f32 {
    let q = p/scale;
    let a = segment(q,vec2<f32>(-0.38,0.30),vec2<f32>(0.38,0.30),0.025);
    let b = segment(q,vec2<f32>(0.38,0.30),vec2<f32>(0.29,-0.15),0.025);
    let c = segment(q,vec2<f32>(0.29,-0.15),vec2<f32>(0.0,-0.44),0.025);
    let d = segment(q,vec2<f32>(0.0,-0.44),vec2<f32>(-0.29,-0.15),0.025);
    let e = segment(q,vec2<f32>(-0.29,-0.15),vec2<f32>(-0.38,0.30),0.025);
    return max(max(a,b),max(max(c,d),e));
}
fn chevron(p: vec2<f32>, y: f32, size: f32) -> f32 {
    return max(segment(p,vec2<f32>(-size,y-size*0.5),vec2<f32>(0.0,y),0.014),
        segment(p,vec2<f32>(0.0,y),vec2<f32>(size,y-size*0.5),0.014));
}
fn motif(p0: vec2<f32>, id: u32, t: f32) -> f32 {
    let p = turn(p0,t*0.08);
    let r = length(p); let a = atan2(p.y,p.x);
    var value = 0.0;
    switch id {
        // Explosive: an expanding cracked sun, twelve jagged rays.
        case 0u: { value = ring(p,0.51+0.035*sin(t*2.0),0.015)
            + line(r-(0.62+0.12*cos(a*12.0)),0.012)*smoothstep(0.38,0.65,r); }
        // Healer: a clean healing cross, four petals.
        case 1u: { value = cross(p,0.35,0.044)+line(r-(0.60+0.065*cos(a*4.0)),0.012); }
        // Protector: shield on a firm octagonal perimeter.
        case 2u: { value = shield(p,1.0)+polygon(p,8.0,0.64,0.013)+cross(p,0.13,0.014); }
        // Banner: the standard's diamond and four banners.
        case 3u: { value = polygon(p,4.0,0.38,0.015)+segment(p,vec2<f32>(0.0,-0.65),vec2<f32>(0.0,0.65),0.018)
            + chevron(p,0.34,0.27)+chevron(-p,0.34,0.27); }
        // Rally: three sound waves and horn rays.
        case 4u: { value = (ring(p,0.32,0.015)+ring(p,0.50,0.012)+ring(p,0.69,0.010))
            *smoothstep(0.35,0.7,abs(sin(a)))+chevron(p,-0.08,0.21); }
        // Death Avenger: skull, eye sockets and teeth.
        case 5u: { value = ring(p-vec2<f32>(0.0,0.10),0.31,0.023)
            +ring(p-vec2<f32>(-0.12,0.12),0.055,0.018)+ring(p-vec2<f32>(0.12,0.12),0.055,0.018)
            +segment(p,vec2<f32>(-0.18,-0.17),vec2<f32>(0.18,-0.17),0.018)
            +line(abs(p.x)-0.09,0.012)*(1.0-smoothstep(0.09,0.14,abs(p.y+0.25))); }
        // Guardian: six inward shields orbit a small central star.
        case 6u: { value = polygon(p,6.0,0.18,0.018);
            for (var n=0u;n<6u;n+=1u) { let q=turn(p,TAU*f32(n)/6.0)-vec2<f32>(0.0,0.56); value+=shield(q,0.38); } }
        // Blood Link: paired blood drops connected by a pulsing figure eight.
        case 7u: { value = drop(turn(p-vec2<f32>(-0.30,0.0),-0.6),0.85)
            +drop(turn(p-vec2<f32>(0.30,0.0),0.6),0.85)
            +line(p.y-0.10*sin(p.x*9.0+t),0.014)*(1.0-smoothstep(0.28,0.43,abs(p.x))); }
        // Electrostatic: six angular bolts, never fire's smooth tongues.
        case 8u: { for (var n=0u;n<6u;n+=1u) { let q=turn(p,TAU*f32(n)/6.0+t*0.06);
            value+=segment(q,vec2<f32>(0.11,0.73),vec2<f32>(-0.05,0.45),0.017)
                +segment(q,vec2<f32>(-0.05,0.45),vec2<f32>(0.08,0.46),0.017)
                +segment(q,vec2<f32>(0.08,0.46),vec2<f32>(-0.08,0.18),0.017); } }
        // Magnetic: opposing horseshoe poles and two field lines.
        case 9u: { let q=turn(p,t*0.18); value=(ring(q,0.49,0.025)+ring(q,0.65,0.010))
            *smoothstep(0.20,0.28,abs(q.y))+segment(q,vec2<f32>(-0.49,-0.25),vec2<f32>(-0.49,0.25),0.024)
            +segment(q,vec2<f32>(0.49,-0.25),vec2<f32>(0.49,0.25),0.024); }
        // Charger: three aligned forward chevrons.
        case 10u: { value=chevron(p,0.54,0.38)+chevron(p,0.18,0.32)+chevron(p,-0.18,0.25); }
        // Burning Wake: a trailing, eight-tongued spiral.
        case 11u: { value=line(r-(0.51+0.12*sin(a*8.0-r*5.0-t)),0.016)
            +line(r-(0.72+0.035*sin(a*8.0-t)),0.010); }
        // Elusive: three fading crescent tracks.
        case 12u: { for (var n=0u;n<3u;n+=1u) { let q=turn(p,TAU*f32(n)/3.0+t*0.26);
            value+=ring(q-vec2<f32>(0.13,0.0),0.49,0.023)*smoothstep(0.0,0.45,q.x)*(0.65+0.35*sin(t+f32(n))); } }
        // Second Skin: double hexagonal scales, three nested layers.
        case 13u: { value=polygon(p,6.0,0.64,0.015)+polygon(turn(p,PI/6.0),6.0,0.43,0.012)+polygon(p,6.0,0.24,0.010); }
        // Avenger: a banked-energy spiral with four barbed arms.
        case 14u: { value=line(sin(a*2.0+r*10.0-t)*0.10,0.012)*smoothstep(0.22,0.32,r)*(1.0-smoothstep(0.69,0.74,r))
            +polygon(turn(p,t*0.16),4.0,0.54,0.012); }
        // Riposte: crossed blades and curved guard.
        case 15u: { let q=turn(p,PI/4.0); value=cross(q,0.58,0.019)
            +segment(q,vec2<f32>(-0.13,-0.29),vec2<f32>(0.13,-0.29),0.028)
            +segment(q,vec2<f32>(-0.29,-0.13),vec2<f32>(-0.29,0.13),0.028)
            +ring(p,0.68,0.010)*smoothstep(0.1,0.4,-p.y); }
        // Adaptive: a six-school prism; hue changes in the fragment stage.
        case 16u: { value=polygon(turn(p,t*0.13),6.0,0.60,0.018)
            +polygon(turn(p,-t*0.21),3.0,0.35,0.014)+ring(p,0.14,0.018); }
        // Aegis: a shield in a four-petalled luminous shell.
        case 17u: { value=shield(p,0.72)+line(r-(0.58+0.10*cos(a*4.0)),0.018)+ring(p,0.79,0.008); }
        // Bastion: square battlements and a heavy inner fort.
        case 18u: { value=polygon(turn(p,PI/4.0),4.0,0.52,0.021)
            +polygon(turn(p,PI/4.0),4.0,0.31,0.014)
            +line(max(abs(p.x),abs(p.y))-(0.62+0.045*sign(cos(a*12.0))),0.014); }
        // Predator: three curved claw slashes and a feline eye.
        case 19u: { for (var n=0u;n<3u;n+=1u) { let x=(f32(n)-1.0)*0.20;
            value+=line(p.x-x-0.22*p.y*p.y,0.019)*(1.0-smoothstep(0.39,0.55,abs(p.y))); }
            value+=line(length(vec2<f32>(p.x*0.65,p.y*2.2))-0.39,0.012); }
        // Plagued: three biohazard lobes around a contaminated core.
        case 20u: { value=ring(p,0.14,0.026);
            for (var n=0u;n<3u;n+=1u) { let q=turn(p,TAU*f32(n)/3.0)-vec2<f32>(0.0,0.39);
                value+=ring(q,0.29,0.025)*smoothstep(-0.12,0.06,q.y); } }
        // Hemorrhagic: three falling drops on a serrated blood ring.
        case 21u: { value=drop(p-vec2<f32>(0.0,0.12),1.25)
            +drop(p-vec2<f32>(-0.38,-0.14),0.7)+drop(p-vec2<f32>(0.38,-0.14),0.7)
            +line(r-(0.74+0.025*cos(a*17.0)),0.010); }
        // Incendiary: five standing flames, distinct from the wake's spiral.
        case 22u: { for (var n=0u;n<5u;n+=1u) { let q=turn(p,TAU*f32(n)/5.0)-vec2<f32>(0.0,0.50);
            value+=drop(vec2<f32>(q.x+0.035*sin(q.y*17.0-t*2.0),q.y),0.85); } value+=ring(p,0.24,0.016); }
        // Greater Artillery: four separate targeting reticles.
        case 23u: { for (var n=0u;n<4u;n+=1u) { let q=turn(p,TAU*f32(n)/4.0)-vec2<f32>(0.0,0.51);
            value+=ring(q,0.15,0.013)+cross(q,0.22,0.009); } }
        // Echo: three delayed wavefronts carrying double orbiting points.
        case 24u: { for (var n=0u;n<3u;n+=1u) { let wave=fract(t*0.16+f32(n)/3.0);
            value+=ring(p,0.20+wave*0.55,0.015)*(1.0-wave*0.7); }
            let q=turn(p,t*0.4); value+=ring(q-vec2<f32>(0.55,0.0),0.06,0.019)+ring(q+vec2<f32>(0.55,0.0),0.06,0.019); }
        // Bombardier: a locked target and four aiming marks.
        case 25u: { value=ring(p,0.42,0.018)+cross(p,0.65,0.014)+ring(p,0.16,0.014); }
        // Remanence: offset aftershock rings.
        case 26u: { value=ring(p,0.58,0.018)+ring(p-vec2<f32>(0.15,0.0),0.38,0.014)+ring(p+vec2<f32>(0.15,0.0),0.22,0.014); }
        // Flusher: contracting brackets.
        case 27u: { value=polygon(p,4.0,0.55+0.06*sin(t),0.02)+cross(p,0.22,0.015); }
        // Mage Hunter: an eye and leaping chevrons.
        case 28u: { value=ring(vec2<f32>(p.x,p.y*2.0),0.40,0.018)+chevron(p,0.65,0.25)+chevron(p,-0.3,0.25); }
        // Concentration Breaker: a slashed magical seal.
        case 29u: { value=polygon(p,6.0,0.50,0.016)+segment(p,vec2<f32>(-0.55,-0.55),vec2<f32>(0.55,0.55),0.025); }
        // Beater: three separated rays.
        case 30u: { for(var n=0u;n<3u;n+=1u) { let q=turn(p,(f32(n)-1.0)*0.5); value+=segment(q,vec2<f32>(0.0,-0.4),vec2<f32>(0.0,0.65),0.018); } }
        // Besieger: concentric square walls.
        case 31u: { value=polygon(p,4.0,0.58,0.018)+polygon(p,4.0,0.35,0.014)+ring(p,0.17,0.014); }
        // Siege Volley: three aligned impact reticles.
        case 32u: { for(var n=0u;n<3u;n+=1u) { let q=p-vec2<f32>((f32(n)-1.0)*0.4,0.0); value+=ring(q,0.16,0.016)+cross(q,0.21,0.01); } }
        // Desecrator: a spreading, seven-lobed corruption.
        default: { value=line(r-(0.5+0.12*sin(a*7.0+t)),0.02)+ring(p,0.75,0.012)+polygon(p,3.0,0.22,0.018); }
    }
    return clamp(value,0.0,1.8);
}
@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let id=u32(aura.animation.y+0.5);
    let t=aura.animation.x*aura.animation.z+in.color.r*TAU;
    let p=in.uv*2.0-1.0;
    let r=length(p);
    let angle=atan2(p.y,p.x);
    let rim=ring(p,0.90,0.008)*(0.35+0.65*smoothstep(-0.5,0.5,sin(angle*f32(5u+id%7u)+t*0.24)));
    let uv=turn(p,t*0.035)*0.5+0.5;
    let art=textureSample(paladin,paladin_sampler,uv);
    let paladin_glow=dot(art.rgb,vec3<f32>(0.333333))*art.a*aura.animation.w;
    let design=motif(p,id,t);
    let mask=smoothstep(0.11,0.20,r)*(1.0-smoothstep(0.93,0.99,r));
    let strength=(design+rim*0.65+paladin_glow*0.17)*mask*(0.82+0.12*sin(t*1.4));
    // Evaluate derivatives and texture sampling before any non-uniform discard.
    if (r>0.99 || strength<0.009) { discard; }
    var primary=aura.color.rgb;
    if (id==16u) { primary=0.55+0.45*cos(vec3<f32>(0.0,2.0944,4.1888)+t*0.6+angle*0.25); }
    let color=mix(primary,aura.accent.rgb,clamp(design*0.45+0.15*sin(angle*3.0-t),0.0,0.75));
    return vec4<f32>(color*1.35,clamp(strength*0.58,0.0,0.78));
}
