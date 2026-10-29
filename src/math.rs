//! Real linear recurrences and the roots of their characteristic polynomials.
use serde::{Deserialize, Serialize};

pub type Point = [f64; 3];

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recurrence {
    pub order: usize,
    /// Newest term first: a[n+2] = c[0] a[n+1] + c[1] a[n].
    pub c: [f64; 3],
    #[serde(default)]
    pub matrix: Option<[[f64; 2]; 2]>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Root {
    pub re: f64,
    pub im: f64,
}

impl Root {
    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

impl Recurrence {
    pub fn linear(matrix: [[f64; 2]; 2]) -> Self {
        let [[a, b], [c, d]] = matrix;
        Self {
            order: 2,
            c: [a + d, b * c - a * d, 0.0],
            matrix: Some(matrix),
        }
    }

    pub fn step(self, a: Point) -> Point {
        if let Some(m) = self.matrix {
            return [
                m[0][0] * a[0] + m[0][1] * a[1],
                m[1][0] * a[0] + m[1][1] * a[1],
                0.0,
            ];
        }
        if self.order == 2 {
            [a[1], self.c[0] * a[1] + self.c[1] * a[0], 0.0]
        } else {
            [
                a[1],
                a[2],
                self.c[0] * a[2] + self.c[1] * a[1] + self.c[2] * a[0],
            ]
        }
    }

    pub fn roots(self) -> Vec<Root> {
        let [p, q, r] = self.matrix.map(Self::linear).unwrap_or(self).c;
        let mut roots = if self.order == 2 {
            let d = p * p + 4.0 * q;
            if d >= 0.0 {
                vec![
                    Root {
                        re: (p + d.sqrt()) / 2.0,
                        im: 0.0,
                    },
                    Root {
                        re: (p - d.sqrt()) / 2.0,
                        im: 0.0,
                    },
                ]
            } else {
                vec![
                    Root {
                        re: p / 2.0,
                        im: (-d).sqrt() / 2.0,
                    },
                    Root {
                        re: p / 2.0,
                        im: -(-d).sqrt() / 2.0,
                    },
                ]
            }
        } else {
            // Depress x^3 - p*x^2 - q*x - r with x = t + p/3.
            let a = -q - p * p / 3.0;
            let b = -2.0 * p.powi(3) / 27.0 - p * q / 3.0 - r;
            let d = b * b / 4.0 + a.powi(3) / 27.0;
            let shift = p / 3.0;
            if d > 1e-14 {
                let u = (-b / 2.0 + d.sqrt()).cbrt();
                let v = (-b / 2.0 - d.sqrt()).cbrt();
                vec![
                    Root {
                        re: u + v + shift,
                        im: 0.0,
                    },
                    Root {
                        re: -(u + v) / 2.0 + shift,
                        im: (u - v) * 3.0_f64.sqrt() / 2.0,
                    },
                    Root {
                        re: -(u + v) / 2.0 + shift,
                        im: -(u - v) * 3.0_f64.sqrt() / 2.0,
                    },
                ]
            } else if a.abs() < 1e-12 {
                vec![Root { re: shift, im: 0.0 }; 3]
            } else {
                let radius = 2.0 * (-a / 3.0).max(0.0).sqrt();
                let theta = ((3.0 * b / (2.0 * a)) * (-3.0 / a).sqrt())
                    .clamp(-1.0, 1.0)
                    .acos()
                    / 3.0;
                (0..3)
                    .map(|k| Root {
                        re: radius * (theta - k as f64 * std::f64::consts::TAU / 3.0).cos() + shift,
                        im: 0.0,
                    })
                    .collect()
            }
        };
        roots.sort_by(|a, b| b.im.total_cmp(&a.im).then(a.re.total_cmp(&b.re)));
        roots
    }

    /// Vieta's formulas; the caller supplies real roots or conjugate pairs.
    pub fn from_roots(roots: &[Root]) -> Self {
        let mut polynomial = vec![(1.0, 0.0)];
        for root in roots {
            let mut next = vec![(0.0, 0.0); polynomial.len() + 1];
            for (i, &(re, im)) in polynomial.iter().enumerate() {
                next[i].0 += re;
                next[i].1 += im;
                next[i + 1].0 -= re * root.re - im * root.im;
                next[i + 1].1 -= re * root.im + im * root.re;
            }
            polynomial = next;
        }
        let mut c = [0.0; 3];
        for i in 0..roots.len() {
            c[i] = -polynomial[i + 1].0;
        }
        Self {
            order: roots.len(),
            c,
            matrix: None,
        }
    }

    pub fn spectral_radius(self) -> f64 {
        self.roots().iter().map(|r| r.abs()).fold(0.0, f64::max)
    }

    pub fn behavior(self) -> &'static str {
        let rho = self.spectral_radius();
        if rho < 1.0 - 1e-6 {
            "収束"
        } else if rho > 1.0 + 1e-6 {
            "発散モードあり"
        } else {
            "境界 / 重根に注意"
        }
    }
}

pub fn norm(p: Point) -> f64 {
    p.iter().map(|x| x * x).sum::<f64>().sqrt()
}
pub fn lerp(a: Point, b: Point, t: f64) -> Point {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_system_radial_and_rotation() {
        let radial = Recurrence::linear([[1.08, 0.0], [0.0, 1.08]]);
        assert_eq!(radial.step([2.0, -3.0, 0.0]), [2.16, -3.24, 0.0]);
        assert!((radial.spectral_radius() - 1.08).abs() < 1e-8);
        let rotation = Recurrence::linear([[0.0, -1.0], [1.0, 0.0]]);
        let mut p = [1.0, 0.0, 0.0];
        for _ in 0..4 {
            p = rotation.step(p);
        }
        assert_eq!(p, [1.0, 0.0, 0.0]);
        assert!(rotation
            .roots()
            .iter()
            .all(|r| (r.abs() - 1.0).abs() < 1e-10));
    }

    fn residual(rec: Recurrence, z: Root) -> f64 {
        let (mut re, mut im) = (1.0, 0.0);
        for c in &rec.c[..rec.order] {
            (re, im) = (re * z.re - im * z.im - c, re * z.im + im * z.re);
        }
        re.hypot(im)
    }

    #[test]
    fn fibonacci_and_third_order() {
        let f = Recurrence {
            matrix: None,
            order: 2,
            c: [1.0, 1.0, 0.0],
        };
        let mut s = [0.0, 1.0, 0.0];
        for expected in [1.0, 2.0, 3.0, 5.0, 8.0] {
            s = f.step(s);
            assert_eq!(s[1], expected);
        }
        assert_eq!(
            Recurrence {
                matrix: None,
                order: 3,
                c: [2.0, 3.0, 4.0]
            }
            .step([1.0, 2.0, 3.0]),
            [2.0, 3.0, 16.0]
        );
    }

    #[test]
    fn roots_satisfy_polynomial_across_parameter_space() {
        for order in [2, 3] {
            for p in [-8.0, -2.1, -0.5, 0.0, 1.0, 2.0, 8.0] {
                for q in [-8.0, -1.0, 0.0, 0.4, 2.0, 8.0] {
                    for r in [-8.0, -0.4, 0.0, 0.5, 8.0] {
                        let rec = Recurrence {
                            matrix: None,
                            order,
                            c: [p, q, r],
                        };
                        for root in rec.roots() {
                            assert!(residual(rec, root) < 1e-7, "{rec:?} {root:?}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn repeated_roots_and_conjugate_roundtrip() {
        for roots in [
            vec![Root { re: 1.0, im: 0.0 }; 3],
            vec![
                Root { re: 0.7, im: 0.6 },
                Root { re: 0.7, im: -0.6 },
                Root { re: 0.3, im: 0.0 },
            ],
        ] {
            let rec = Recurrence::from_roots(&roots);
            for r in &roots {
                assert!(residual(rec, *r) < 1e-10);
            }
            let restored = Recurrence::from_roots(&rec.roots());
            for i in 0..3 {
                assert!((rec.c[i] - restored.c[i]).abs() < 1e-8);
            }
        }
    }

    #[test]
    fn stability_checks_all_roots() {
        assert_eq!(
            Recurrence {
                matrix: None,
                order: 2,
                c: [1.0, -0.61, 0.0]
            }
            .behavior(),
            "収束"
        );
        assert_eq!(
            Recurrence {
                matrix: None,
                order: 2,
                c: [1.0, 1.0, 0.0]
            }
            .behavior(),
            "発散モードあり"
        );
        assert_eq!(
            Recurrence {
                matrix: None,
                order: 2,
                c: [2.0, -1.0, 0.0]
            }
            .behavior(),
            "境界 / 重根に注意"
        );
    }
}
