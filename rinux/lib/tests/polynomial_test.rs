// SPDX-License-Identifier: GPL-2.0-or-later
//! Host tests for `polynomial_calc`. Linux has no KUnit file for it; these
//! use the temperature conversion from the C file's comment, checked against
//! the real-valued formula.

use rinux_lib::math::{Polynomial, PolynomialTerm, polynomial_calc};

const fn term(deg: u32, coef: i64, divider: i64, divider_leftover: i64) -> PolynomialTerm {
    PolynomialTerm {
        deg,
        coef,
        divider,
        divider_leftover,
    }
}

/// N = 1.8322e-8*T^4 + ... with T in millidegrees, from the C comment.
const TEMP_TO_N: Polynomial<'static> = Polynomial {
    total_divider: 10000,
    terms: &[
        term(4, 18322, 10000, 10000),
        term(3, 2343, 10000, 10),
        term(2, 87018, 10000, 10),
        term(1, 39269, 1000, 1),
        term(0, 1720400, 1, 1),
    ],
};

/// T = -1.6743e-11*N^4 + ... giving millidegrees from the PVT code N.
const N_TO_TEMP: Polynomial<'static> = Polynomial {
    total_divider: 1,
    terms: &[
        term(4, -16743, 1000, 1),
        term(3, 81542, 1000, 1),
        term(2, -182010, 1000, 1),
        term(1, 310200, 1000, 1),
        term(0, -48380, 1, 1),
    ],
};

#[test]
fn integer_polynomial_evaluates() {
    // 2x^2 + 3x + 5 at x = 4: 32 + 12 + 5 = 49.
    let p = Polynomial {
        total_divider: 1,
        terms: &[term(2, 2, 1, 1), term(1, 3, 1, 1), term(0, 5, 1, 1)],
    };
    assert_eq!(polynomial_calc(&p, 4), 49);
}

#[test]
fn zero_total_divider_means_one() {
    let p = Polynomial {
        total_divider: 0,
        terms: &[term(0, 7, 1, 1)],
    };
    assert_eq!(polynomial_calc(&p, 123), 7);
}

#[test]
fn temperature_to_pvt_code_matches_the_real_formula() {
    for t in (-48_380..=147_438).step_by(997) {
        let x = t as f64 / 1000.0;
        let real = 1.8322e-8 * x.powi(4)
            + 2.343e-5 * x.powi(3)
            + 8.7018e-3 * x.powi(2)
            + 3.9269 * x
            + 172.04;
        let got = polynomial_calc(&TEMP_TO_N, t);
        assert!(
            (got as f64 - real).abs() <= 1.0,
            "T={t}: got {got}, real {real:.3}"
        );
    }
}

#[test]
fn pvt_code_to_temperature_matches_the_real_formula() {
    for n in 0..=1023 {
        let x = n as f64;
        let real = -1.6743e-11 * x.powi(4) + 8.1542e-8 * x.powi(3) - 1.8201e-4 * x.powi(2)
            + 0.31020 * x
            - 48.38;
        // Results are in millidegrees. Each power truncates, so the integer form
        // drifts up to about 3.5 mC from the real value near N = 800; 5 mC bounds it.
        let got = polynomial_calc(&N_TO_TEMP, n) as f64;
        assert!(
            (got - real * 1000.0).abs() <= 5.0,
            "N={n}: got {got}, real {}",
            real * 1000.0
        );
    }
}
