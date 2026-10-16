// SPDX-License-Identifier: GPL-2.0
//! CORDIC tests: the ported fixed-point sine/cosine must track the real
//! `f64` functions to within the algorithm's precision. (Linux ships no
//! kunit for cordic; this checks the port against `libm`.)

use rinux_lib::math::cordic_calc_iq;

const SCALE: f64 = 65536.0; // results are scaled by 2^16
const TOL: f64 = 200.0; // ~0.3% of full scale, within 18-iteration precision

#[test]
fn matches_libm_over_the_circle() {
    for deg in -180..=180 {
        let c = cordic_calc_iq(deg);
        let rad = (deg as f64).to_radians();
        let want_i = rad.cos() * SCALE;
        let want_q = rad.sin() * SCALE;
        assert!(
            (c.i as f64 - want_i).abs() < TOL,
            "cos({deg}): got {}, want {want_i}",
            c.i
        );
        assert!(
            (c.q as f64 - want_q).abs() < TOL,
            "sin({deg}): got {}, want {want_q}",
            c.q
        );
    }
}

#[test]
fn cardinal_points() {
    let p0 = cordic_calc_iq(0);
    assert!((p0.i - 65536).abs() < 200 && p0.q.abs() < 200);
    let p90 = cordic_calc_iq(90);
    assert!(p90.i.abs() < 200 && (p90.q - 65536).abs() < 200);
    let p180 = cordic_calc_iq(180);
    assert!((p180.i + 65536).abs() < 200 && p180.q.abs() < 200);
    let pm90 = cordic_calc_iq(-90);
    assert!(pm90.i.abs() < 200 && (pm90.q + 65536).abs() < 200);
}

#[test]
fn angle_reduction_wraps() {
    // 270 reduces to -90; 540 reduces to 180.
    let a = cordic_calc_iq(270);
    let b = cordic_calc_iq(-90);
    assert!((a.i - b.i).abs() < 50 && (a.q - b.q).abs() < 50);
}
