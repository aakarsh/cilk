extern crate cilk;
mod codegen;
mod parser;

fn main() {
    // let input = r#"
    // function main(): i32 {
    //     var a: i32; a = 10000;
    //     var c: i32; c = 8400;
    //     var b: i32;
    //     var d: i32;
    //     var e: i32;
    //     var g: i32;
    //     var f: [8401] i32;
    //
    //     b = 0;
    //     while b < c {
    //         f[b] = a / 5;
    //         b = b + 1;
    //     }
    //
    //     e = 0;
    //     c = 8400;
    //     while 0 < c {
    //         d = 0;
    //         b = c - 1;
    //         while 0 < b {
    //             g = b * 2- 1;
    //             d = d * b + f[b] * a;
    //             f[b] = d % g;
    //             d = d / g;
    //             b = b - 1;
    //         }
    //
    //         println_i32(e + d / a);
    //
    //         e = d % a;
    //         c = c - 14;
    //     }
    //
    //     return 0;
    // }
    // "#;
    // let input = r#"
    // struct A {
    //     first: [8][8] i32,
    //     second: i32
    // }
    // function f(): i32 {
    //     println_i32(2);
    //     return  0;
    // }
    // function main(): i32 {
    //     f();
    //     return 0;
    // }
    // "#;

    // let input = r#"
    // function m(c_x: f64, c_y: f64, n: i32): i32 {
    //     var x_n: f64; x_n = 0.0;
    //     var y_n: f64; y_n = 0.0;
    //     var x_n_1: f64; var y_n_1: f64;
    //     var i: i32;
    //     i = 0;
    //     while i < n {
    //         x_n_1 = x_n*x_n - y_n*y_n + c_x;
    //         y_n_1 = x_n * y_n * 2.0 + c_y;
    //         if 4.0 < x_n_1*x_n_1 + y_n_1*y_n_1 {
    //             return n;
    //         } else {
    //             x_n = x_n_1;
    //             y_n = y_n_1;
    //         }
    //         i = i + 1;
    //     }
    //     return 0;
    // }
    // function main(): i32 {
    //     var x_max: f64; x_max = 1.0;
    //     var x_min: f64; x_min = 0.0 - 2.0;
    //     var y_max: f64; y_max = 1.0;
    //     var y_min: f64; y_min = 0.0 - 1.0;
    //     var dx: f64; dx = 0.05;
    //     var dy: f64; dy = 0.05;
    //     var y: f64; var x: f64;
    //     y = y_max;
    //     while y_min < y {
    //         x = x_min;
    //         while x < x_max {
    //             if m(x, y, 300) == 0 {
    //                 printch_i32(65);
    //             } else {
    //                 printch_i32(32);
    //             }
    //             x = x + dx;
    //         }
    //         printch_i32(10);
    //         y = y - dy;
    //     }
    //     return 0;
    // }
    //     "#;

    // let input = "
    // function main(): i32 {
    //     println_f64( sin(3.14 / 2.0) );
    //     println_f64( cos(3.14 / 2.0) );
    //     println_f64( sqrt(2.0) );
    //     return 0;
    // }
    // ";

    // let input = "
    // struct Vec {
    //     x: f64,
    //     y: f64,
    //     z: f64
    // }
    // function main(): i32 {
    //     var v: * struct Vec;
    //     v = malloc(24);
    //     (*v).x = 2.3;
    //     return 0;
    // }
    // ";

    // let input = r#"
    // function main(): i32 {
    //     var i: i32;
    //     var k: i32;
    //     i = 0; while i < 10 {
    //         k = 0; while k < 10 {
    //             k = k + 1;
    //         }
    //         i = i + 1;
    //     }
    //     return 0;
    // }
    // "#;

    let input = r#"
    struct Vec {
      x: f64,
      y: f64,
      z: f64
    }

    struct Isect {
      hit: i32,
      hit_point: * struct Vec,
      normal: * struct Vec,
      color: * struct Vec,
      distance: f64,
      ray_dir: * struct Vec
    }

    struct Ray {
      origin: * struct Vec,
      dir: * struct Vec
    }

    struct Sphere {
      radius: f64,
      position: * struct Vec,
      color: * struct Vec
    }

    struct Plane {
      position: * struct Vec,
      normal  : * struct Vec,
      color   : * struct Vec
    }

    struct Env {
      light     : *struct Vec,
      sphere1: *struct Sphere,
      sphere2: *struct Sphere,
      sphere3: *struct Sphere,
      plane   : *struct Plane
    }

    function clamp(t: f64, min: f64, max: f64): f64 {
      if t < min { return min; }
      if max < t { return max; }
      return t;
    }

    function Vec_new(x: f64, y: f64, z: f64): *struct Vec {
      var vec: * struct Vec;
      vec = malloc(128);
      (*vec).x = x;
      (*vec).y = y;
      (*vec).z = z;
      return vec;
    }

    function Vec_add(a: *struct Vec, b: *struct Vec): *struct Vec {
      return Vec_new((*a).x + (*b).x, (*a).y + (*b).y, (*a).z + (*b).z);
    }

    function Vec_sub(a: *struct Vec, b: *struct Vec): *struct Vec {
      return Vec_new((*a).x - (*b).x, (*a).y - (*b).y, (*a).z - (*b).z);
    }

    function Vec_mul(a: *struct Vec, t: f64): *struct Vec {
      return Vec_new((*a).x * t, (*a).y * t, (*a).z * t);
    }

    function Vec_multi(a: *struct Vec, b: *struct Vec): *struct Vec {
      return Vec_new((*a).x * (*b).x, (*a).y * (*b).y, (*a).z * (*b).z);
    }

    function Vec_dot(a: *struct Vec, b: *struct Vec): f64 {
      return (*a).x * (*b).x + (*a).y * (*b).y + (*a).z * (*b).z;
    }

    function Vec_reflect(self: *struct Vec, normal: *struct Vec): *struct Vec {
      return Vec_add(self, Vec_mul(normal, (0.0-2.0)*Vec_dot(normal, self)));
    }

    function Vec_length(v: *struct Vec): f64 {
      return sqrt((*v).x*(*v).x + (*v).y*(*v).y + (*v).z*(*v).z);
    }

    function Vec_normalize(v: *struct Vec): *struct Vec {
      var len: f64;
      var r_len: f64;
      len = Vec_length(v);
      if 0.00000001 < len {
        r_len = 1.0 / len;
        (*v).x = (*v).x * r_len;
        (*v).y = (*v).y * r_len;
        (*v).z = (*v).z * r_len;
      }
      return v;
    }

    function Ray_new(origin: *struct Vec, dir: *struct Vec): *struct Ray {
      var ray: *struct Ray;
      ray = malloc(128);
      (*ray).origin = origin;
      (*ray).dir = dir;
      return ray;
    }

    function Isect_new(
      hit: i32,
      hit_point: *struct Vec,
      normal: *struct Vec,
      color: *struct Vec,
      distance: f64,
      ray_dir: *struct Vec): *struct Isect {
      var i: *struct Isect;
      i = malloc(128);
      (*i).hit       = hit      ;
      (*i).hit_point = hit_point;
      (*i).normal    = normal   ;
      (*i).color     = color    ;
      (*i).distance  = distance ;
      (*i).ray_dir   = ray_dir ;
      return i;
    }

    function Sphere_new(radius: f64, position: *struct Vec, color: *struct Vec): *struct Sphere {
      var s: *struct Sphere;
      s = malloc(128);
      (*s).radius   = radius;
      (*s).position = position;
      (*s).color    = color;
      return s;
    }

    function Sphere_intersect(s: *struct Sphere, light: *struct Vec, ray: *struct Ray, isect: *struct Isect): i32 {
      var rs: *struct Vec;
      var b: f64; var c: f64; var d: f64; var t: f64;
      rs = Vec_sub((*ray).origin, (*s).position);
      b = Vec_dot(rs, (*ray).dir);
      c = Vec_dot(rs, rs) - (*s).radius * (*s).radius;
      d = b * b - c;
      t = 0.0 - b - sqrt(d);
      if d <= 0.0 { return 0; }
      if t <= 0.0001 { return 0; }
      if (*isect).distance <= t { return 0; }
      (*isect).hit_point = Vec_add((*ray).origin, Vec_mul((*ray).dir, t));
      (*isect).normal = Vec_normalize(Vec_sub((*isect).hit_point, (*s).position));
      (*isect).color = Vec_mul((*s).color, clamp(Vec_dot(light, (*isect).normal), 0.1, 1.0));
      (*isect).distance = t;
      (*isect).hit = (*isect).hit + 1;
      (*isect).ray_dir = (*ray).dir;
      return 0;
    }

    function Plane_new(position: *struct Vec, normal: *struct Vec, color: *struct Vec): *struct Plane {
      var p: *struct Plane;
      p = malloc(128);
      (*p).position = position;
      (*p).normal = normal;
      (*p).color = color;
      return p;
    }

    function Plane_intersect(p: *struct Plane, light: *struct Vec, ray: *struct Ray, isect: *struct Isect): i32 {
      var d: f64;
      var v: f64;
      var t: f64;
      var d2: f64;
      var m: f64;
      var n: f64;
      var d3: f64;
      var abs_: f64;
      var f: f64;
      d = 0.0 - Vec_dot((*p).position, (*p).normal);
      v = Vec_dot((*ray).dir, (*p).normal);
      t = 0.0 - (Vec_dot((*ray).origin, (*p).normal) + d) / v;
      if t <= 0.0001 { return 0; }
      if (*isect).distance <= t { return 0; }
      (*isect).hit_point = Vec_add((*ray).origin, Vec_mul((*ray).dir, t));
      (*isect).normal = (*p).normal;
      d2 = clamp(Vec_dot(light, (*isect).normal), 0.1, 1.0);
      m = (*(*isect).hit_point).x - 2.0*floor((*(*isect).hit_point).x / 2.0);
      n = (*(*isect).hit_point).z - 2.0*floor((*(*isect).hit_point).z / 2.0);
      d3 = d2;
      if 1.0 < m { if 1.0 < n { d3 = d3 * 0.5; } }
      else { if m < 1.0 { if n < 1.0 { d3 = d3 * 0.5; } } }
      abs_ = fabs((*(*isect).hit_point).z);
      f = 0.0;
      if abs_ < 25.0 { f = 1.0 - abs_*0.04; }
      (*isect).color = Vec_mul((*p).color, d3 * f);
      (*isect).distance = t;
      (*isect).hit = (*isect).hit + 1;
      (*isect).ray_dir = (*ray).dir;
      return 0;
    }

    function Env_intersect(env: *struct Env, ray: *struct Ray, i: *struct Isect): i32 {
      Sphere_intersect((*env).sphere1, (*env).light, ray, i);
      Sphere_intersect((*env).sphere2, (*env).light, ray, i);
      Sphere_intersect((*env).sphere3, (*env).light, ray, i);
      Plane_intersect((*env).plane, (*env).light, ray, i);
      return 0;
    }

    function Env_new(): *struct Env {
      var env: *struct Env;
      env = malloc(128);
      (*env).light = Vec_new(0.577, 0.577, 0.577);
      (*env).sphere1 = Sphere_new(0.5, Vec_new( 0.0, 0.0-0.5, 0.0), Vec_new(1.0, 0.0, 0.0));
      (*env).sphere2 = Sphere_new(1.0, Vec_new( 2.0,  0.0, cos(10.0 * 0.666)), Vec_new(0.0, 1.0, 0.0));
      (*env).sphere3 = Sphere_new(1.5, Vec_new(0.0-2.0,  0.5, cos(10.0 * 0.333)), Vec_new(0.0, 0.0, 1.0));
      (*env).plane = Plane_new(Vec_new(0.0, 0.0-1.0, 0.0), Vec_new(0.0, 1.0, 0.0), Vec_new(1.0, 1.0, 1.0));
      return env;
    }

    function color_of(t: f64): i32 {
      var ret: i32;
      ret = f64_to_i32((i32_to_f64(256) * clamp(t, 0.0, 1.0)));
      if ret == 256 { return 256 - 1; }
      return ret;
    }

    function print_col(c: *struct Vec): i32 {
      print_i32(color_of((*c).x)); printch_i32(32);
      print_i32(color_of((*c).y)); printch_i32(32);
      print_i32(color_of((*c).z)); printch_i32(10);
      return 0;
    }

    function main(): i32 {
      var env: *struct Env;
      var row: i32; var col: i32;
      var x: f64; var y: f64;
      var ray: *struct Ray;
      var i: *struct Isect;
      var dest_col: *struct Vec;
      var temp_col: *struct Vec;
      var j: i32;
      var q: *struct Ray;

      env = Env_new();

      row = 0; while row < 300 {
        col = 0; while col < 300 {
          x = i32_to_f64(col) / (300.0 / 2.0) - 1.0;
          y = i32_to_f64(300 - row) / (300.0 / 2.0) - 1.0;

          ray = Ray_new( Vec_new(0.0, 2.0, 6.0), Vec_normalize(Vec_new(x, y, 0.0 - 1.0)) );
          i = Isect_new(0, Vec_new(0.0, 0.0, 0.0), Vec_new(0.0, 0.0, 0.0), Vec_new(0.0, 0.0, 0.0),
                        10000000.0, Vec_new(0.0, 0.0, 0.0));
              Env_intersect(env, ray, i);

          if 0 < (*i).hit {
            dest_col = (*i).color;
            temp_col = Vec_multi(Vec_new(1.0, 1.0, 1.0), (*i).color);
            j = 1; while j < 4 {
              q = Ray_new(Vec_add((*i).hit_point, Vec_mul((*i).normal, 0.0001)),
                                      Vec_reflect((*i).ray_dir, (*i).normal));
              Env_intersect(env, q, i);
              if j < (*i).hit {
                dest_col = Vec_add(dest_col, Vec_multi(temp_col, (*i).color));
                temp_col = Vec_multi(temp_col, (*i).color);
              }

              j = j + 1;
            }
            print_col(dest_col);
          } else {
            print_col(Vec_new((*(*ray).dir).y, (*(*ray).dir).y, (*(*ray).dir).y));
          }
          col = col + 1;
        }
        row = row + 1;
      }

      return 0;
    }
                    "#;
    let mut codegen = codegen::CodeGenerator::new();
    codegen.run(input);

    let mut jit = cilk::codegen::x64::exec::jit::JITExecutor::new(&codegen.module);
    let func = jit.find_function_by_name("main").unwrap();
    println!("Result: {:?}", jit.run(func, vec![]));
}

#[test]
fn pi() {
    let input = r#"
    function main(output: * [600] i32): i32 {
        var a: i32; a = 10000;
        var c: i32; c = 8400;
        var b: i32;
        var d: i32;
        var e: i32;
        var g: i32;
        var f: [8401] i32;
        var i: i32;

        b = 0;
        while b < c {
            f[b] = a / 5;
            b = b + 1;
        }

        e = 0;
        c = 8400;
        i = 0;
        while 0 < c {
            d = 0;
            b = c - 1;
            while 0 < b {
                g = b * 2- 1;
                d = d * b + f[b] * a;
                f[b] = d % g;
                d = d / g;
                b = b - 1;
            }

            println_i32(e + d / a);
            output[i] = e + d / a;

            e = d % a;
            c = c - 14;
            i = i + 1;
        }

        return 0;
    }"#;

    let mut codegen = codegen::CodeGenerator::new();
    codegen.run(input);

    let mut jit = cilk::codegen::x64::exec::jit::JITExecutor::new(&codegen.module);
    let func = jit.find_function_by_name("main").unwrap();
    let output: [i32; 600] = [0; 600];
    let answer: [i32; 600] = [
        3141, 5926, 5358, 9793, 2384, 6264, 3383, 2795, 288, 4197, 1693, 9937, 5105, 8209, 7494,
        4592, 3078, 1640, 6286, 2089, 9862, 8034, 8253, 4211, 7067, 9821, 4808, 6513, 2823, 664,
        7093, 8446, 955, 582, 2317, 2535, 9408, 1284, 8111, 7450, 2841, 270, 1938, 5211, 555, 9644,
        6229, 4895, 4930, 3819, 6442, 8810, 9756, 6593, 3446, 1284, 7564, 8233, 7867, 8316, 5271,
        2019, 914, 5648, 5669, 2346, 348, 6104, 5432, 6648, 2133, 9360, 7260, 2491, 4127, 3724,
        5870, 660, 6315, 5881, 7488, 1520, 9209, 6282, 9254, 917, 1536, 4367, 8925, 9036, 11, 3305,
        3054, 8820, 4665, 2138, 4146, 9519, 4151, 1609, 4330, 5727, 365, 7595, 9195, 3092, 1861,
        1738, 1932, 6117, 9310, 5118, 5480, 7446, 2379, 9627, 4956, 7351, 8857, 5272, 4891, 2279,
        3818, 3011, 9491, 2983, 3673, 3624, 4065, 6643, 860, 2139, 4946, 3952, 2473, 7190, 7021,
        7986, 943, 7027, 7053, 9217, 1762, 9317, 6752, 3846, 7481, 8467, 6694, 513, 2000, 5681,
        2714, 5263, 5608, 2778, 5771, 3427, 5778, 9609, 1736, 3717, 8721, 4684, 4090, 1224, 9534,
        3014, 6549, 5853, 7105, 792, 2796, 8925, 8923, 5420, 1995, 6112, 1290, 2196, 864, 344,
        1815, 9813, 6297, 7477, 1309, 9605, 1870, 7211, 3499, 9999, 8372, 9780, 4995, 1059, 7317,
        3281, 6096, 3185, 9502, 4459, 4553, 4690, 8302, 6425, 2230, 8253, 3446, 8503, 5261, 9311,
        8817, 1010, 31, 3783, 8752, 8865, 8753, 3208, 3814, 2061, 7177, 6691, 4730, 3598, 2534,
        9042, 8755, 4687, 3115, 9562, 8638, 8235, 3787, 5937, 5195, 7781, 8577, 8053, 2171, 2268,
        661, 3001, 9278, 7661, 1195, 9092, 1642, 198, 9380, 9525, 7201, 654, 8586, 3278, 8659,
        3615, 3381, 8279, 6823, 301, 9520, 3530, 1852, 9689, 9577, 3622, 5994, 1389, 1249, 7217,
        7528, 3479, 1315, 1557, 4857, 2424, 5415, 695, 9508, 2953, 3116, 8617, 2785, 5889, 750,
        9838, 1754, 6374, 6493, 9319, 2550, 6040, 927, 7016, 7113, 9009, 8488, 2401, 2858, 3616,
        356, 3707, 6601, 471, 181, 9429, 5559, 6198, 9467, 6783, 7449, 4482, 5537, 9774, 7268,
        4710, 4047, 5346, 4620, 8046, 6842, 5906, 9491, 2933, 1367, 7028, 9891, 5210, 4752, 1620,
        5696, 6024, 580, 3815, 193, 5112, 5338, 2430, 355, 8764, 247, 4964, 7326, 3914, 1992, 7260,
        4269, 9227, 9678, 2354, 7816, 3600, 9341, 7216, 4121, 9924, 5863, 1503, 286, 1829, 7455,
        5706, 7498, 3850, 5494, 5885, 8692, 6995, 6909, 2721, 797, 5093, 295, 5321, 1653, 4498,
        7202, 7559, 6023, 6480, 6654, 9911, 9881, 8347, 9775, 3566, 3698, 742, 6542, 5278, 6255,
        1818, 4175, 7467, 2890, 9777, 7279, 3800, 816, 4706, 16, 1452, 4919, 2173, 2172, 1477,
        2350, 1414, 4197, 3568, 5481, 6136, 1157, 3525, 5213, 3475, 7418, 4946, 8438, 5233, 2390,
        7394, 1433, 3454, 7762, 4168, 6251, 8983, 5694, 8556, 2099, 2192, 2218, 4272, 5502, 5425,
        6887, 6717, 9049, 4601, 6534, 6680, 4988, 6272, 3279, 1786, 857, 8438, 3827, 9679, 7668,
        1454, 1009, 5388, 3786, 3609, 5068, 64, 2251, 2520, 5117, 3929, 8489, 6084, 1284, 8862,
        6945, 6042, 4196, 5285, 222, 1066, 1186, 3067, 4427, 8622, 391, 9494, 5047, 1237, 1378,
        6960, 9563, 6437, 1917, 2874, 6776, 4657, 5739, 6241, 3890, 8658, 3264, 5995, 8133, 9047,
        8027, 5900, 9946, 5764, 789, 5126, 9468, 3983, 5259, 5709, 8258, 2262, 522, 4894, 772,
        6719, 4782, 6848, 2601, 4769, 9090, 2640, 1363, 9443, 7455, 3050, 6820, 3496, 2524, 5174,
        9399, 6514, 3142, 9809, 1906, 5925, 937, 2216, 9646, 1515, 7098, 5838, 7410, 5978, 8595,
        9772, 9754, 9893, 161, 7539, 2846, 8138, 2686, 8386, 8942, 7741, 5599, 1855, 9252, 4595,
        3959, 4310, 4997, 2524, 6808, 4598, 7273, 6446, 9584, 8653, 8367, 3622, 2626, 991, 2460,
        8051, 2438, 8439, 451, 2441, 3654, 9762, 7807, 9771, 5691, 4359, 9770, 129, 6160, 8944,
        1694, 8685, 5584, 8406, 3534, 2207, 2225, 8284, 8864, 8158, 4560, 2850,
    ];
    jit.run(
        func,
        vec![cilk::codegen::x64::exec::jit::GenericValue::Address(
            output.as_ptr() as *mut u8,
        )],
    );
    for (a, b) in output.iter().zip(answer.iter()) {
        assert_eq!(a, b);
    }
}
