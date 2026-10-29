use crate::math::{lerp, norm, Point, Recurrence};
use std::collections::VecDeque;

pub struct Particle {
    pub history: VecDeque<Point>,
    pub age: usize,
    pub hue: f32,
}

pub struct Field {
    pub particles: Vec<Particle>,
    pub phase: f64,
    pub steps: u64,
    rng: u64,
}

impl Default for Field {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            phase: 0.0,
            steps: 0,
            rng: 42,
        }
    }
}

impl Field {
    fn random(&mut self) -> f64 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng >> 11) as f64 / ((1_u64 << 53) as f64)
    }

    fn seed(&mut self, order: usize, spread: f64) -> Point {
        let mut a = std::array::from_fn(|_| (self.random() * 2.0 - 1.0) * spread);
        if order == 2 {
            a[2] = 0.0;
        }
        a
    }

    pub fn reset(&mut self, rec: Recurrence, count: usize, spread: f64, trail: usize) {
        self.rng = 0x4d595df4d0f33173;
        self.phase = 0.0;
        self.steps = 0;
        self.particles.clear();
        for i in 0..count {
            let mut particle = Particle {
                history: VecDeque::from([self.seed(rec.order, spread)]),
                age: 0,
                hue: i as f32 / count as f32,
            };
            // Spread ages over the entire lifetime, so converging particles do not
            // all disappear into the origin and respawn in one synchronized wave.
            let warmup = (self.random() * 181.0) as usize;
            for _ in 0..warmup {
                self.advance_particle(&mut particle, rec, spread, trail);
            }
            self.particles.push(particle);
        }
    }

    fn advance_particle(&mut self, p: &mut Particle, rec: Recurrence, spread: f64, trail: usize) {
        let next = rec.step(*p.history.back().expect("particle always has a point"));
        if !next.iter().all(|x| x.is_finite())
            || norm(next) > spread * 24.0
            || norm(next) < spread * 0.002
            || p.age > 180
        {
            p.history.clear();
            p.history.push_back(self.seed(rec.order, spread));
            p.age = 0;
        } else {
            p.history.push_back(next);
            p.age += 1;
        }
        while p.history.len() > trail + 1 {
            p.history.pop_front();
        }
    }

    pub fn advance(&mut self, rec: Recurrence, dt: f64, speed: f64, spread: f64, trail: usize) {
        self.phase += dt.clamp(0.0, 0.1) * speed;
        while self.phase >= 1.0 {
            self.phase -= 1.0;
            self.steps += 1;
            let mut particles = std::mem::take(&mut self.particles);
            for p in &mut particles {
                self.advance_particle(p, rec, spread, trail);
            }
            self.particles = particles;
        }
    }

    pub fn head(&self, p: &Particle, rec: Recurrence) -> Point {
        let a = *p.history.back().expect("particle always has a point");
        lerp(a, rec.step(a), self.phase)
    }
}

/// A single mathematical orbit. Never resampled or normalized.
pub fn orbit(rec: Recurrence, initial: Point, count: usize) -> Vec<Point> {
    let mut p = initial;
    if rec.order == 2 {
        p[2] = 0.0;
    }
    let mut points = vec![p];
    for _ in 0..count {
        p = rec.step(p);
        if p.iter().any(|v| !v.is_finite() || v.abs() > 1e100) {
            break;
        }
        points.push(p);
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converging_field_keeps_young_particles_throughout_lifetime() {
        let rec = Recurrence {
            order: 2,
            c: [1.36, -0.96, 0.0],
            matrix: None,
        };
        let mut field = Field::default();
        field.reset(rec, 750, 4.0, 32);
        for _ in 0..400 {
            let young = field.particles.iter().filter(|p| p.age < 30).count();
            assert!(young > 50, "field lost its outer trajectories: {young}");
            field.advance(rec, 0.1, 10.0, 4.0, 32);
        }
    }
    #[test]
    fn extreme_coefficients_keep_field_bounded_and_finite() {
        for c in [[8.0, 8.0, 8.0], [-8.0, -8.0, -8.0], [0.0; 3]] {
            let rec = Recurrence {
                order: 3,
                c,
                matrix: None,
            };
            let mut field = Field::default();
            field.reset(rec, 64, 4.0, 20);
            for _ in 0..300 {
                field.advance(rec, 0.1, 30.0, 4.0, 20);
            }
            for p in field.particles {
                assert!(!p.history.is_empty() && p.history.len() <= 21);
                assert!(p.history.iter().flatten().all(|x| x.is_finite()));
            }
        }
    }

    #[test]
    fn single_orbit_is_not_resampled_on_divergence() {
        let points = orbit(
            Recurrence {
                matrix: None,
                order: 2,
                c: [2.0, 0.0, 0.0],
            },
            [1.0, 2.0, 99.0],
            8,
        );
        assert_eq!(points.last().unwrap(), &[256.0, 512.0, 0.0]);
    }

    #[test]
    fn reproducible_reset_and_pause() {
        let rec = Recurrence {
            matrix: None,
            order: 2,
            c: [1.0, -0.61, 0.0],
        };
        let mut field = Field::default();
        field.reset(rec, 20, 4.0, 24);
        let start = field.particles[0].history.clone();
        field.advance(rec, 0.0, 10.0, 4.0, 24);
        assert_eq!(start, field.particles[0].history);
        field.reset(rec, 20, 4.0, 24);
        assert_eq!(start, field.particles[0].history);
    }
}
