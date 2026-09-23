//! Floating-point reductions shared by the interpreter and native runtime.

pub fn float_midpoint(left: f64, right: f64) -> f64 {
    if left == right {
        return left;
    }
    if !left.is_finite() || !right.is_finite() {
        return (left + right) / 2.0;
    }
    if left.is_sign_negative() == right.is_sign_negative() {
        left + (right - left) / 2.0
    } else {
        left / 2.0 + right / 2.0
    }
}

pub fn float_average(values: &[f64]) -> f64 {
    let sum = values.iter().sum::<f64>();
    // Rescaling can underflow small residuals after large values cancel.
    // Keep ordinary summation whenever it did not overflow.
    if sum.is_finite() {
        return sum / values.len() as f64;
    }
    let scale = values.iter().map(|value| value.abs()).fold(0.0, f64::max);
    if scale == 0.0 || !scale.is_finite() {
        return sum / values.len() as f64;
    }

    let scaled_sum = values.iter().map(|value| value / scale).sum::<f64>();
    scaled_sum / values.len() as f64 * scale
}
