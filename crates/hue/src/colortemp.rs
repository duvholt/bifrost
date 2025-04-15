use crate::xy::XY;

// compute point on 3rd degree polynomial
fn power3_approx(input: f64, q: [f64; 4]) -> f64 {
    q[0].mul_add(input, q[1])
        .mul_add(input, q[2])
        .mul_add(input, q[3])
}

/// Convert an input CCT value (Corrected Color Temperature) to XY color coordinates
///
/// Inspired by this implementation:
///
///   <https://github.com/colour-science/colour/blob/develop/colour/temperature/kang2002.py>
///
/// Algorithm by Kang et. al
///
///   `Kang2002a`: Kang, B., Moon, O., Hong, C., Lee, H., Cho, B., & Kim,
///    Y. (2002). Design of advanced color: Temperature control system for HDTV
///    applications. Journal of the Korean Physical Society, 41(6), 865-871.
///
#[rustfmt::skip]
#[must_use]
pub fn cct_to_xy(cct: f64) -> XY {
    const X_OVER_ZERO: [f64; 4] = [-0.266_123_90, -0.234_358_90, 0.877_695_60,  0.179_910_00];
    const X_OVER_4000: [f64; 4] = [-3.025_846_90,  2.107_037_90, 0.222_634_70,  0.240_390_00];
    const Y_OVER_ZERO: [f64; 4] = [-1.106_381_40, -1.348_110_20, 2.185_558_32, -0.202_196_83];
    const Y_OVER_2222: [f64; 4] = [-0.954_947_60, -1.374_185_93, 2.091_370_15, -0.167_488_67];
    const Y_OVER_4000: [f64; 4] = [ 3.081_758_00, -5.873_386_70, 3.751_129_97, -0.370_014_83];

    let mk = 1000.0 / cct;

    let x = if cct <= 4000.0 {
        power3_approx(mk, X_OVER_ZERO)
    } else {
        power3_approx(mk, X_OVER_4000)
    };

    let y = if cct <= 2222.0 {
        power3_approx(x, Y_OVER_ZERO)
    } else if cct <= 4000.0 {
        power3_approx(x, Y_OVER_2222)
    } else {
        power3_approx(x, Y_OVER_4000)
    };

    XY::new(x, y)
}

#[cfg(test)]
mod tests {
    use crate::colortemp::cct_to_xy;
    use crate::xy::XY;

    macro_rules! compare {
        ($expr:expr, $value:expr) => {
            let a = $expr;
            let b = $value;
            eprintln!("{a} vs {b:.4}");
            assert!((a - b).abs() < 1e-4);
        };
    }

    macro_rules! compare_xy {
        ($expr:expr, $value:expr) => {
            let a = $expr;
            let b = $value;
            compare!(a.x, b.x);
            compare!(a.y, b.y);
        };
    }

    // Regression tests, sanity checked against kelvin-to-blackbody raditation color
    // data found here:
    //
    //   <http://www.vendian.org/mncharity/dir3/blackbody/UnstableURLs/bbr_color.html>
    //
    // The values match to 2-3 decimals, which is about what can be expected
    // from the approximation used.

    #[test]
    fn test2000k() {
        let a = cct_to_xy(2000.0);
        let b = XY::new(0.5269, 0.4132);

        compare_xy!(a, b);
    }

    #[test]
    fn test3500k() {
        let a = cct_to_xy(3500.0);
        let b = XY::new(0.4053, 0.3908);

        compare_xy!(a, b);
    }

    #[test]
    fn test4200k() {
        let a = cct_to_xy(4200.0);
        let b = XY::new(0.3720, 0.3713);

        compare_xy!(a, b);
    }

    #[test]
    fn test6500k() {
        let a = cct_to_xy(6500.0);
        let b = XY::new(0.3134, 0.3236);

        compare_xy!(a, b);
    }
}
