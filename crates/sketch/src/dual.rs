//! Forward-mode automatic differentiation used to obtain exact (analytic)
//! Jacobians of the constraint residual kernels.
//!
//! Every residual kernel is written once, generically over [`Num`], and is
//! evaluated either with plain `f64` (residual only) or with [`Dual<N>`]
//! (residual + exact gradient w.r.t. its `N` local inputs).

#![allow(clippy::needless_range_loop)]

use std::ops::{Add, Div, Mul, Neg, Sub};

pub(crate) trait Num:
    Copy
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Div<Output = Self>
    + Neg<Output = Self>
{
    fn cst(v: f64) -> Self;
    fn val(self) -> f64;
    fn sqrt(self) -> Self;
    fn sin(self) -> Self;
    fn cos(self) -> Self;
    fn atan2(self, x: Self) -> Self;
    #[inline]
    fn scale(self, k: f64) -> Self {
        self * Self::cst(k)
    }
}

impl Num for f64 {
    #[inline]
    fn cst(v: f64) -> Self {
        v
    }
    #[inline]
    fn val(self) -> f64 {
        self
    }
    #[inline]
    fn sqrt(self) -> Self {
        f64::sqrt(self)
    }
    #[inline]
    fn sin(self) -> Self {
        f64::sin(self)
    }
    #[inline]
    fn cos(self) -> Self {
        f64::cos(self)
    }
    #[inline]
    fn atan2(self, x: Self) -> Self {
        f64::atan2(self, x)
    }
}

/// Dual number with an `N`-dimensional gradient.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dual<const N: usize> {
    pub v: f64,
    pub g: [f64; N],
}

impl<const N: usize> Dual<N> {
    #[inline]
    pub fn var(v: f64, i: usize) -> Self {
        let mut g = [0.0; N];
        g[i] = 1.0;
        Dual { v, g }
    }
    #[inline]
    fn map_g(self, k: f64) -> [f64; N] {
        let mut g = self.g;
        for x in g.iter_mut() {
            *x *= k;
        }
        g
    }
}

impl<const N: usize> Add for Dual<N> {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        let mut g = self.g;
        for i in 0..N {
            g[i] += o.g[i];
        }
        Dual { v: self.v + o.v, g }
    }
}

impl<const N: usize> Sub for Dual<N> {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        let mut g = self.g;
        for i in 0..N {
            g[i] -= o.g[i];
        }
        Dual { v: self.v - o.v, g }
    }
}

impl<const N: usize> Mul for Dual<N> {
    type Output = Self;
    #[inline]
    fn mul(self, o: Self) -> Self {
        let mut g = [0.0; N];
        for i in 0..N {
            g[i] = self.g[i] * o.v + o.g[i] * self.v;
        }
        Dual { v: self.v * o.v, g }
    }
}

impl<const N: usize> Div for Dual<N> {
    type Output = Self;
    #[inline]
    fn div(self, o: Self) -> Self {
        let inv = 1.0 / o.v;
        let v = self.v * inv;
        let mut g = [0.0; N];
        for i in 0..N {
            g[i] = (self.g[i] - v * o.g[i]) * inv;
        }
        Dual { v, g }
    }
}

impl<const N: usize> Neg for Dual<N> {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Dual { v: -self.v, g: self.map_g(-1.0) }
    }
}

impl<const N: usize> Num for Dual<N> {
    #[inline]
    fn cst(v: f64) -> Self {
        Dual { v, g: [0.0; N] }
    }
    #[inline]
    fn val(self) -> f64 {
        self.v
    }
    #[inline]
    fn sqrt(self) -> Self {
        let s = self.v.max(0.0).sqrt();
        let k = if s > 1e-300 { 0.5 / s } else { 0.0 };
        Dual { v: s, g: self.map_g(k) }
    }
    #[inline]
    fn sin(self) -> Self {
        Dual { v: self.v.sin(), g: self.map_g(self.v.cos()) }
    }
    #[inline]
    fn cos(self) -> Self {
        Dual { v: self.v.cos(), g: self.map_g(-self.v.sin()) }
    }
    #[inline]
    fn atan2(self, x: Self) -> Self {
        let d = x.v * x.v + self.v * self.v;
        let mut g = [0.0; N];
        if d > 1e-300 {
            for i in 0..N {
                g[i] = (x.v * self.g[i] - self.v * x.g[i]) / d;
            }
        }
        Dual { v: self.v.atan2(x.v), g }
    }
    #[inline]
    fn scale(self, k: f64) -> Self {
        Dual { v: self.v * k, g: self.map_g(k) }
    }
}

/// Small generic 2D vector used by the kernels.
#[derive(Clone, Copy, Debug)]
pub(crate) struct V2<T> {
    pub x: T,
    pub y: T,
}

impl<T: Num> V2<T> {
    #[inline]
    pub fn new(x: T, y: T) -> Self {
        V2 { x, y }
    }
    #[inline]
    pub fn at(s: &[T], i: usize) -> Self {
        V2 { x: s[i], y: s[i + 1] }
    }
    #[inline]
    pub fn sub(self, o: Self) -> Self {
        V2::new(self.x - o.x, self.y - o.y)
    }
    #[inline]
    pub fn add(self, o: Self) -> Self {
        V2::new(self.x + o.x, self.y + o.y)
    }
    #[inline]
    pub fn mul(self, k: T) -> Self {
        V2::new(self.x * k, self.y * k)
    }
    #[inline]
    pub fn scale(self, k: f64) -> Self {
        V2::new(self.x.scale(k), self.y.scale(k))
    }
    #[inline]
    pub fn dot(self, o: Self) -> T {
        self.x * o.x + self.y * o.y
    }
    #[inline]
    pub fn cross(self, o: Self) -> T {
        self.x * o.y - self.y * o.x
    }
    #[inline]
    pub fn norm(self) -> T {
        self.dot(self).sqrt()
    }
    #[inline]
    pub fn perp(self) -> Self {
        V2::new(-self.y, self.x)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dual_derivatives() {
        let x = Dual::<2>::var(0.7, 0);
        let y = Dual::<2>::var(-1.3, 1);
        let f = (x * x + y * y).sqrt() + y.atan2(x) * x.sin() / (y + Dual::cst(3.0));
        let h = 1e-6;
        let fx = |a: f64, b: f64| (a * a + b * b).sqrt() + b.atan2(a) * a.sin() / (b + 3.0);
        let gx = (fx(0.7 + h, -1.3) - fx(0.7 - h, -1.3)) / (2.0 * h);
        let gy = (fx(0.7, -1.3 + h) - fx(0.7, -1.3 - h)) / (2.0 * h);
        assert!((f.g[0] - gx).abs() < 1e-8);
        assert!((f.g[1] - gy).abs() < 1e-8);
    }
}
